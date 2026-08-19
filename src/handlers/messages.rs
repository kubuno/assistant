use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{BoxStream, StreamExt};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use axum::extract::Json as AxumJson;
use axum::http::StatusCode;

use crate::{
    errors::{AssistantError, AssistantResult},
    handlers::agent_access,
    middleware::AssistantUser,
    models::{FeedbackDto, Message, SendMessageDto, SseEvent},
    services::{agentic::AgenticProvider, run_agentic, AgenticEvent, LlmMessage, McpClient, ToolCatalogItem},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct ConvInfo {
    #[allow(dead_code)]
    id:       Uuid,
    agent_id: Option<Uuid>,
    model_id: String,
    provider: String,
}

pub async fn send_message(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(conv_id): Path<Uuid>,
    axum::Json(dto): axum::Json<SendMessageDto>,
) -> AssistantResult<Sse<BoxStream<'static, Result<Event, Infallible>>>> {
    let policy = st.instance();

    if !dto.regenerate && dto.content.trim().is_empty() {
        return Err(AssistantError::Validation("Le message ne peut pas être vide".into()));
    }
    if dto.content.chars().count() > policy.max_message_chars {
        return Err(AssistantError::Validation(format!(
            "Message trop long : {} caractères au maximum sur cette instance.",
            policy.max_message_chars
        )));
    }

    let conv = sqlx::query_as::<_, ConvInfo>(
        "SELECT id, agent_id, model_id, provider FROM assistant.conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(conv_id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| AssistantError::NotFound("conversation introuvable".into()))?;

    let model = dto.model.unwrap_or(conv.model_id);
    // Instance policy: only the models the administrator listed may be used.
    if !policy.model_allowed(&model) {
        return Err(AssistantError::Validation(format!(
            "Modèle non autorisé sur cette instance : {model}"
        )));
    }

    // Get agent system prompt + the tools it is allowed to use.
    //
    // The lookup is bound to the caller: owning the conversation is NOT enough to
    // reach whatever agent id it points at. An agent the user cannot reach — one
    // that was deleted, or one belonging to another account, which rows written
    // before this check may still reference — contributes nothing, so the answer
    // is generated with no agent prompt instead of borrowing (and, through the
    // model, disclosing) somebody else's `system_prompt`.
    let (system_prompt, enabled_tools): (String, Vec<String>) = match conv.agent_id {
        Some(agent_id) => agent_access::load_prompt(&st.db, agent_id, user.id)
            .await?
            .map(|a| (a.system_prompt, a.enabled_tools))
            .unwrap_or_default(),
        None => (String::new(), Vec::new()),
    };

    // Ancre temporelle : sans elle le modèle invente l'année (ex. 2022) en
    // résolvant « demain ». On l'ajoute toujours, y compris à l'agent par défaut.
    let system_prompt = format!(
        "{system_prompt}\n\nDate et heure actuelles : {} (UTC). Utilise-les pour résoudre « aujourd'hui », « demain », etc., et émets toujours les dates/heures au format ISO 8601 (AAAA-MM-JJTHH:MM:SS).",
        chrono::Utc::now().format("%Y-%m-%d %H:%M, %A")
    );

    if dto.regenerate {
        // Régénération : supprime la dernière réponse assistant (et ses éventuels
        // messages outils qui suivent le dernier message utilisateur), puis relance
        // depuis l'historique qui se termine alors par le dernier message utilisateur.
        sqlx::query(
            r#"DELETE FROM assistant.messages
               WHERE conversation_id = $1
                 AND created_at > COALESCE(
                     (SELECT MAX(created_at) FROM assistant.messages WHERE conversation_id = $1 AND role = 'user'),
                     '-infinity'::timestamptz)"#,
        )
        .bind(conv_id)
        .execute(&st.db)
        .await?;
    } else {
        // Persist user message
        sqlx::query(
            "INSERT INTO assistant.messages (conversation_id, role, content) VALUES ($1, 'user', $2)",
        )
        .bind(conv_id)
        .bind(dto.content.trim())
        .execute(&st.db)
        .await?;
    }

    // Build history. The instance may cap how far back the model is fed: the
    // newest N messages are selected, then put back in chronological order, so a
    // long conversation stops re-billing its whole past on every turn.
    // `0` keeps the previous behaviour — replay everything.
    let history = if policy.history_window_messages > 0 {
        let mut recent = sqlx::query_as::<_, Message>(
            r#"SELECT id, conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens, feedback, created_at
               FROM assistant.messages WHERE conversation_id = $1
               ORDER BY created_at DESC LIMIT $2"#,
        )
        .bind(conv_id)
        .bind(policy.history_window_messages)
        .fetch_all(&st.db)
        .await?;
        recent.reverse();
        recent
    } else {
        sqlx::query_as::<_, Message>(
            r#"SELECT id, conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens, feedback, created_at
               FROM assistant.messages WHERE conversation_id = $1 ORDER BY created_at ASC"#,
        )
        .bind(conv_id)
        .fetch_all(&st.db)
        .await?
    };

    // ── Agentic path: Anthropic + MCP tools ─────────────────────────────────
    // When the provider is Anthropic and tools are available, run the agentic
    // tool-calling loop (discovers tools via the core gateway, lets the model
    // call them, feeds results back). Other providers keep the plain stream.
    // Pick a tool-capable provider (Anthropic, or Ollama as the local default).
    let providers = st.providers();
    let agentic_provider: Option<Arc<dyn AgenticProvider>> = match conv.provider.as_str() {
        "anthropic" => providers.anthropic.clone().map(|a| a as Arc<dyn AgenticProvider>),
        "openai"    => providers.openai.clone().map(|a| a as Arc<dyn AgenticProvider>),
        "google"    => None, // tool-calling for Google is a follow-up
        _           => Some(providers.ollama.clone() as Arc<dyn AgenticProvider>),
    };
    // Instance policy: tools can be switched off for the whole instance. Skipping
    // the catalogue lookup means no tool is even offered to the model.
    if let Some(provider) = agentic_provider.filter(|_| policy.enable_tools) {
        {
            let mcp = McpClient::new(&st.settings.core.url, &st.settings.core.internal_secret);
            let catalog = mcp.list_tools(user.id).await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, "catalogue MCP indisponible");
                Vec::new()
            });
            // Scope: if the agent declares enabled_tools, restrict to those;
            // otherwise offer the full catalog (works out of the box).
            let tools: Vec<ToolCatalogItem> = if enabled_tools.is_empty() {
                catalog
            } else {
                catalog.into_iter()
                    .filter(|t| enabled_tools.iter().any(|e| e == &t.name))
                    .collect()
            };

            if !tools.is_empty() {
                let turns: Vec<LlmMessage> = history.iter()
                    .map(|m| LlmMessage { role: m.role.clone(), content: m.content.clone() })
                    .collect();
                let mut rx = run_agentic(
                    provider, mcp, user.id, model, system_prompt, turns, tools,
                    policy.max_tool_rounds,
                );
                let db = st.db.clone();
                let sse_stream = async_stream::stream! {
                    let mut full_content = String::new();
                    let mut tool_calls: Vec<serde_json::Value> = Vec::new();
                    let mut prompt_tokens     = 0i32;
                    let mut completion_tokens = 0i32;

                    while let Some(ev) = rx.recv().await {
                        match ev {
                            AgenticEvent::Delta(t) => {
                                if !t.is_empty() {
                                    full_content.push_str(&t);
                                    let data = serde_json::to_string(&SseEvent::Delta { content: t }).unwrap_or_default();
                                    yield Ok::<Event, Infallible>(Event::default().data(data));
                                }
                            }
                            AgenticEvent::ToolCall(call) => {
                                tool_calls.push(call.clone());
                                let data = serde_json::to_string(&SseEvent::ToolCall { call }).unwrap_or_default();
                                yield Ok(Event::default().data(data));
                            }
                            AgenticEvent::Done { input_tokens, output_tokens } => {
                                prompt_tokens     = input_tokens;
                                completion_tokens = output_tokens;
                            }
                            AgenticEvent::Error(msg) => {
                                tracing::error!(error = %msg, "boucle agentique");
                                let data = serde_json::to_string(&SseEvent::Error { message: msg }).unwrap_or_default();
                                yield Ok(Event::default().data(data));
                            }
                        }
                    }

                    let tc = serde_json::Value::Array(tool_calls);
                    match sqlx::query_scalar::<_, Uuid>(
                        r#"INSERT INTO assistant.messages
                               (conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens)
                           VALUES ($1, 'assistant', $2, $3, $4, $5)
                           RETURNING id"#,
                    )
                    .bind(conv_id)
                    .bind(&full_content)
                    .bind(&tc)
                    .bind(prompt_tokens)
                    .bind(completion_tokens)
                    .fetch_one(&db)
                    .await
                    {
                        Ok(msg_id) => {
                            let ev   = SseEvent::Done { message_id: msg_id, prompt_tokens, completion_tokens };
                            let data = serde_json::to_string(&ev).unwrap_or_default();
                            yield Ok(Event::default().data(data));
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "persistence message assistant (agentique)");
                            let data = serde_json::to_string(&SseEvent::Error { message: "Erreur sauvegarde".into() }).unwrap_or_default();
                            yield Ok(Event::default().data(data));
                        }
                    }

                    yield Ok(Event::default().data("[DONE]"));
                };
                return Ok(Sse::new(sse_stream.boxed()).keep_alive(KeepAlive::default()));
            }
        }
    }

    let mut messages: Vec<LlmMessage> = Vec::with_capacity(history.len() + 1);
    if !system_prompt.is_empty() {
        messages.push(LlmMessage { role: "system".into(), content: system_prompt });
    }
    for m in &history {
        messages.push(LlmMessage { role: m.role.clone(), content: m.content.clone() });
    }

    // Dispatch to the right provider
    let mut rx = match conv.provider.as_str() {
        "openai" => {
            let svc = providers.openai.as_ref()
                .ok_or_else(|| AssistantError::Validation("OpenAI non configuré. Vérifiez la configuration Assistant.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "OpenAI stream error"); AssistantError::OllamaUnavailable(e.to_string()) })?
        }
        "anthropic" => {
            let svc = providers.anthropic.as_ref()
                .ok_or_else(|| AssistantError::Validation("Anthropic non configuré. Vérifiez la configuration Assistant.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Anthropic stream error"); AssistantError::OllamaUnavailable(e.to_string()) })?
        }
        "google" => {
            let svc = providers.google.as_ref()
                .ok_or_else(|| AssistantError::Validation("Google AI non configuré. Vérifiez la configuration Assistant.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Google AI stream error"); AssistantError::OllamaUnavailable(e.to_string()) })?
        }
        _ => {
            // Default: Ollama
            providers.ollama.chat_stream_unified(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Ollama indisponible"); AssistantError::OllamaUnavailable(e.to_string()) })?
        }
    };

    let db = st.db.clone();

    let sse_stream = async_stream::stream! {
        let mut full_content      = String::new();
        let mut prompt_tokens     = 0i32;
        let mut completion_tokens = 0i32;

        while let Some(chunk) = rx.recv().await {
            if let Some(delta) = chunk.delta {
                if !delta.is_empty() {
                    full_content.push_str(&delta);
                    let ev   = SseEvent::Delta { content: delta };
                    let data = serde_json::to_string(&ev).unwrap_or_default();
                    yield Ok::<Event, Infallible>(Event::default().data(data));
                }
            }
            if chunk.done {
                prompt_tokens     = chunk.prompt_tokens;
                completion_tokens = chunk.completion_tokens;
                break;
            }
        }

        match sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO assistant.messages
                   (conversation_id, role, content, prompt_tokens, completion_tokens)
               VALUES ($1, 'assistant', $2, $3, $4)
               RETURNING id"#,
        )
        .bind(conv_id)
        .bind(&full_content)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .fetch_one(&db)
        .await
        {
            Ok(msg_id) => {
                let ev   = SseEvent::Done { message_id: msg_id, prompt_tokens, completion_tokens };
                let data = serde_json::to_string(&ev).unwrap_or_default();
                yield Ok(Event::default().data(data));
            }
            Err(e) => {
                tracing::error!(error = %e, "erreur persistence message assistant");
                let ev   = SseEvent::Error { message: "Erreur sauvegarde".into() };
                let data = serde_json::to_string(&ev).unwrap_or_default();
                yield Ok(Event::default().data(data));
            }
        }

        yield Ok(Event::default().data("[DONE]"));
    };

    Ok(Sse::new(sse_stream.boxed()).keep_alive(KeepAlive::default()))
}

/// Enregistre un retour 👍/👎 sur un message (ou le retire avec `null`).
pub async fn set_feedback(
    State(st): State<AppState>,
    user: AssistantUser,
    Path((conv_id, msg_id)): Path<(Uuid, Uuid)>,
    AxumJson(dto): AxumJson<FeedbackDto>,
) -> AssistantResult<StatusCode> {
    // N'autorise que "like"/"dislike"/null.
    if let Some(f) = &dto.feedback {
        if f != "like" && f != "dislike" {
            return Err(AssistantError::Validation("retour invalide".into()));
        }
    }
    let affected = sqlx::query(
        r#"UPDATE assistant.messages m SET feedback = $1
           FROM assistant.conversations c
           WHERE m.id = $2 AND m.conversation_id = $3 AND c.id = m.conversation_id AND c.owner_id = $4"#,
    )
    .bind(dto.feedback)
    .bind(msg_id)
    .bind(conv_id)
    .bind(user.id)
    .execute(&st.db)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AssistantError::NotFound("message introuvable".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un message (vérifie l'appartenance via la conversation).
pub async fn delete_message(
    State(st): State<AppState>,
    user: AssistantUser,
    Path((conv_id, msg_id)): Path<(Uuid, Uuid)>,
) -> AssistantResult<StatusCode> {
    let affected = sqlx::query(
        r#"DELETE FROM assistant.messages m
           USING assistant.conversations c
           WHERE m.id = $1 AND m.conversation_id = $2 AND c.id = m.conversation_id AND c.owner_id = $3"#,
    )
    .bind(msg_id)
    .bind(conv_id)
    .bind(user.id)
    .execute(&st.db)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AssistantError::NotFound("message introuvable".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

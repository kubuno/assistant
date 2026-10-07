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
use kubuno_db::{params, DbPool};

use crate::{
    errors::{AssistantError, AssistantResult},
    handlers::agent_access,
    middleware::AssistantUser,
    models::{FeedbackDto, Message, SendMessageDto, SseEvent},
    services::{agentic::AgenticProvider, run_agentic, AgenticEvent, LlmMessage, McpClient, ToolCatalogItem},
    state::AppState,
    sync,
};

#[derive(sqlx::FromRow)]
struct ConvInfo {
    #[allow(dead_code)]
    id:       Uuid,
    agent_id: Option<Uuid>,
    model_id: String,
    provider: String,
}

/// Inserts a message and bumps its parent conversation's stats and delta
/// sequence, all in one transaction.
///
/// This replaces two PostgreSQL triggers the port retired: the stats trigger
/// (`AFTER INSERT ON messages` → `message_count`/`total_tokens`) and the delta
/// bump. Neither survives MySQL/SQLite, so both are done here from Rust. The
/// primary key is generated in the process (MySQL has no `RETURNING`) and
/// returned. Matching the old triggers, `message_count`/`total_tokens` only ever
/// grow — a later delete does not decrement them.
async fn persist_message(
    db:                &DbPool,
    conversation_id:   Uuid,
    role:              &str,
    content:           &str,
    tool_calls:        Option<&serde_json::Value>,
    prompt_tokens:     i32,
    completion_tokens: i32,
) -> Result<Uuid, sqlx::Error> {
    let id = kubuno_db::new_id();
    let now = chrono::Utc::now();
    let mut tx = db.begin().await?;
    let seq = sync::next_conv_seq(&mut tx).await?;
    match tool_calls {
        Some(tc) => {
            tx.execute(
                "INSERT INTO assistant.messages \
                     (id, conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                params![id, conversation_id, role, content, tc.clone(), prompt_tokens, completion_tokens, now],
            )
            .await?;
        }
        None => {
            tx.execute(
                "INSERT INTO assistant.messages \
                     (id, conversation_id, role, content, prompt_tokens, completion_tokens, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                params![id, conversation_id, role, content, prompt_tokens, completion_tokens, now],
            )
            .await?;
        }
    }
    // Stats + delta bump on the parent, folded into a single UPDATE (the old
    // stats trigger + change_seq bump). `updated_at` is refreshed too, matching
    // the trigger that counted a conversation's age from its last activity.
    tx.execute(
        "UPDATE assistant.conversations SET \
             message_count = message_count + 1, \
             total_tokens  = total_tokens + $1 + $2, \
             updated_at    = $3, \
             change_seq    = $4 \
         WHERE id = $5",
        params![prompt_tokens, completion_tokens, now, seq, conversation_id],
    )
    .await?;
    tx.commit().await?;
    Ok(id)
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

    let conv = st.db.fetch_optional_as::<ConvInfo>(
        "SELECT id, agent_id, model_id, provider FROM assistant.conversations WHERE id = $1 AND owner_id = $2",
        params![conv_id, user.id],
    )
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
        //
        // The last user message's timestamp is read in Rust (PostgreSQL's
        // `'-infinity'::timestamptz` and the reused placeholder are not
        // portable); everything strictly after it is removed. The delete then
        // bumps the conversation once — replacing the per-row `AFTER DELETE`
        // trigger — so a client sees the truncation.
        let last_user: Option<chrono::DateTime<chrono::Utc>> = st.db.fetch_optional_scalar(
            "SELECT MAX(created_at) FROM assistant.messages WHERE conversation_id = $1 AND role = 'user'",
            params![conv_id],
        )
        .await?;
        let mut tx = st.db.begin().await?;
        match last_user {
            Some(ts) => {
                tx.execute(
                    "DELETE FROM assistant.messages WHERE conversation_id = $1 AND created_at > $2",
                    params![conv_id, ts],
                )
                .await?;
            }
            None => {
                tx.execute(
                    "DELETE FROM assistant.messages WHERE conversation_id = $1",
                    params![conv_id],
                )
                .await?;
            }
        }
        sync::touch_conversation(&mut tx, conv_id).await?;
        tx.commit().await?;
    } else {
        // Persist user message (bumps the conversation's stats + delta sequence).
        persist_message(&st.db, conv_id, "user", dto.content.trim(), None, 0, 0).await?;
    }

    // Build history. The instance may cap how far back the model is fed: the
    // newest N messages are selected, then put back in chronological order, so a
    // long conversation stops re-billing its whole past on every turn.
    // `0` keeps the previous behaviour — replay everything.
    let history = if policy.history_window_messages > 0 {
        let mut recent = st.db.fetch_all_as::<Message>(
            r#"SELECT id, conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens, feedback, created_at
               FROM assistant.messages WHERE conversation_id = $1
               ORDER BY created_at DESC LIMIT $2"#,
            params![conv_id, policy.history_window_messages],
        )
        .await?;
        recent.reverse();
        recent
    } else {
        st.db.fetch_all_as::<Message>(
            r#"SELECT id, conversation_id, role, content, tool_calls, prompt_tokens, completion_tokens, feedback, created_at
               FROM assistant.messages WHERE conversation_id = $1 ORDER BY created_at ASC"#,
            params![conv_id],
        )
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
                    match persist_message(
                        &db, conv_id, "assistant", &full_content, Some(&tc),
                        prompt_tokens, completion_tokens,
                    )
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

        match persist_message(
            &db, conv_id, "assistant", &full_content, None,
            prompt_tokens, completion_tokens,
        )
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
    // Ownership is verified with a portable JOIN (PostgreSQL's `UPDATE ... FROM`
    // has no cross-engine form). The conversation id it returns is then used to
    // bump the conversation's delta sequence — the old `AFTER UPDATE` message
    // trigger — and lets us tell "not found" from "no change" without leaning on
    // `rows_affected` (which MySQL reports as 0 for an unchanged UPDATE).
    let owns = st.db.fetch_optional_scalar::<Uuid>(
        "SELECT m.conversation_id FROM assistant.messages m \
         JOIN assistant.conversations c ON c.id = m.conversation_id \
         WHERE m.id = $1 AND m.conversation_id = $2 AND c.owner_id = $3",
        params![msg_id, conv_id, user.id],
    )
    .await?;
    let Some(cid) = owns else {
        return Err(AssistantError::NotFound("message introuvable".into()));
    };

    let mut tx = st.db.begin().await?;
    tx.execute(
        "UPDATE assistant.messages SET feedback = $1 WHERE id = $2",
        params![dto.feedback, msg_id],
    )
    .await?;
    sync::touch_conversation(&mut tx, cid).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un message (vérifie l'appartenance via la conversation).
pub async fn delete_message(
    State(st): State<AppState>,
    user: AssistantUser,
    Path((conv_id, msg_id)): Path<(Uuid, Uuid)>,
) -> AssistantResult<StatusCode> {
    // Ownership via a portable JOIN (PostgreSQL's `DELETE ... USING` has no
    // cross-engine form); the conversation id it returns is then bumped so a
    // client learns the message is gone — replacing the `AFTER DELETE` trigger.
    let owns = st.db.fetch_optional_scalar::<Uuid>(
        "SELECT m.conversation_id FROM assistant.messages m \
         JOIN assistant.conversations c ON c.id = m.conversation_id \
         WHERE m.id = $1 AND m.conversation_id = $2 AND c.owner_id = $3",
        params![msg_id, conv_id, user.id],
    )
    .await?;
    let Some(cid) = owns else {
        return Err(AssistantError::NotFound("message introuvable".into()));
    };

    let mut tx = st.db.begin().await?;
    tx.execute(
        "DELETE FROM assistant.messages WHERE id = $1",
        params![msg_id],
    )
    .await?;
    sync::touch_conversation(&mut tx, cid).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

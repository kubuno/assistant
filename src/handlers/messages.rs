use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use std::convert::Infallible;
use uuid::Uuid;

use crate::{
    errors::{JarvisError, JarvisResult},
    middleware::JarvisUser,
    models::{Message, SendMessageDto, SseEvent},
    services::LlmMessage,
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
    user: JarvisUser,
    Path(conv_id): Path<Uuid>,
    axum::Json(dto): axum::Json<SendMessageDto>,
) -> JarvisResult<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>> {
    if dto.content.trim().is_empty() {
        return Err(JarvisError::Validation("Le message ne peut pas être vide".into()));
    }

    let conv = sqlx::query_as::<_, ConvInfo>(
        "SELECT id, agent_id, model_id, provider FROM jarvis.conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(conv_id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("conversation introuvable".into()))?;

    let model = dto.model.unwrap_or(conv.model_id);

    // Get agent system prompt
    let system_prompt: Option<String> = if let Some(agent_id) = conv.agent_id {
        sqlx::query_scalar("SELECT system_prompt FROM jarvis.agents WHERE id = $1")
            .bind(agent_id)
            .fetch_optional(&st.db)
            .await?
    } else {
        None
    };
    let system_prompt = system_prompt.unwrap_or_default();

    // Persist user message
    sqlx::query(
        "INSERT INTO jarvis.messages (conversation_id, role, content) VALUES ($1, 'user', $2)",
    )
    .bind(conv_id)
    .bind(dto.content.trim())
    .execute(&st.db)
    .await?;

    // Build history
    let history = sqlx::query_as::<_, Message>(
        r#"SELECT id, conversation_id, role, content, prompt_tokens, completion_tokens, created_at
           FROM jarvis.messages WHERE conversation_id = $1 ORDER BY created_at ASC"#,
    )
    .bind(conv_id)
    .fetch_all(&st.db)
    .await?;

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
            let svc = st.openai.as_ref()
                .ok_or_else(|| JarvisError::Validation("OpenAI non configuré. Vérifiez la configuration Jarvis.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "OpenAI stream error"); JarvisError::OllamaUnavailable(e.to_string()) })?
        }
        "anthropic" => {
            let svc = st.anthropic.as_ref()
                .ok_or_else(|| JarvisError::Validation("Anthropic non configuré. Vérifiez la configuration Jarvis.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Anthropic stream error"); JarvisError::OllamaUnavailable(e.to_string()) })?
        }
        "google" => {
            let svc = st.google.as_ref()
                .ok_or_else(|| JarvisError::Validation("Google AI non configuré. Vérifiez la configuration Jarvis.".into()))?;
            svc.chat_stream(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Google AI stream error"); JarvisError::OllamaUnavailable(e.to_string()) })?
        }
        _ => {
            // Default: Ollama
            st.ollama.chat_stream_unified(&model, messages).await
                .map_err(|e| { tracing::error!(error = %e, "Ollama indisponible"); JarvisError::OllamaUnavailable(e.to_string()) })?
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
            r#"INSERT INTO jarvis.messages
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

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

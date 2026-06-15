use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    errors::{JarvisError, JarvisResult},
    middleware::JarvisUser,
    models::{Conversation, ConversationSummary, CreateConversationDto, Message, UpdateConversationDto},
    state::AppState,
};

pub async fn list_conversations(
    State(st): State<AppState>,
    user: JarvisUser,
) -> JarvisResult<Json<Vec<ConversationSummary>>> {
    let rows = sqlx::query_as::<_, Conversation>(
        r#"
        SELECT id, owner_id, agent_id, title, model_id, message_count, total_tokens,
               is_pinned, is_archived, created_at, updated_at
        FROM jarvis.conversations
        WHERE owner_id = $1 AND is_archived = false AND is_trashed = false
        ORDER BY is_pinned DESC, updated_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for conv in rows {
        let last_message: Option<String> = sqlx::query_scalar(
            "SELECT content FROM jarvis.messages WHERE conversation_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(conv.id)
        .fetch_optional(&st.db)
        .await?;

        summaries.push(ConversationSummary { conversation: conv, last_message });
    }

    Ok(Json(summaries))
}

pub async fn get_conversation(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<Json<Conversation>> {
    let conv = sqlx::query_as::<_, Conversation>(
        r#"SELECT id, owner_id, agent_id, title, model_id, message_count, total_tokens,
                  is_pinned, is_archived, created_at, updated_at
           FROM jarvis.conversations WHERE id = $1 AND owner_id = $2"#,
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("conversation introuvable".into()))?;

    Ok(Json(conv))
}

pub async fn create_conversation(
    State(st): State<AppState>,
    user: JarvisUser,
    Json(dto): Json<CreateConversationDto>,
) -> JarvisResult<(StatusCode, Json<Conversation>)> {
    let default_model = st.ollama.default_model().to_string();
    let model_id  = dto.model.unwrap_or(default_model);
    let valid_providers = ["ollama", "openai", "anthropic", "google"];
    let provider = dto.provider
        .filter(|p| valid_providers.contains(&p.as_str()))
        .unwrap_or_else(|| "ollama".to_string());

    let agent_id = match dto.agent_id {
        Some(id) => Some(id),
        None => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM jarvis.agents WHERE is_system = true ORDER BY created_at LIMIT 1",
            )
            .fetch_optional(&st.db)
            .await?
        }
    };

    let title = dto.title.as_deref().map(ToOwned::to_owned);

    let conv = sqlx::query_as::<_, Conversation>(
        r#"INSERT INTO jarvis.conversations (owner_id, agent_id, title, model_id, provider)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, owner_id, agent_id, title, model_id, message_count, total_tokens,
                     is_pinned, is_archived, created_at, updated_at"#,
    )
    .bind(user.id)
    .bind(agent_id)
    .bind(title)
    .bind(&model_id)
    .bind(&provider)
    .fetch_one(&st.db)
    .await?;

    Ok((StatusCode::CREATED, Json(conv)))
}

pub async fn update_conversation(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateConversationDto>,
) -> JarvisResult<Json<Conversation>> {
    let conv = sqlx::query_as::<_, Conversation>(
        r#"UPDATE jarvis.conversations SET
               title       = COALESCE($3, title),
               is_pinned   = COALESCE($4, is_pinned),
               is_archived = COALESCE($5, is_archived),
               model_id    = COALESCE($6, model_id)
           WHERE id = $1 AND owner_id = $2
           RETURNING id, owner_id, agent_id, title, model_id, message_count, total_tokens,
                     is_pinned, is_archived, created_at, updated_at"#,
    )
    .bind(id)
    .bind(user.id)
    .bind(dto.title.as_deref())
    .bind(dto.is_pinned)
    .bind(dto.is_archived)
    .bind(dto.model.as_deref())
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("conversation introuvable".into()))?;

    Ok(Json(conv))
}

pub async fn delete_conversation(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<StatusCode> {
    let rows = sqlx::query(
        "DELETE FROM jarvis.conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .execute(&st.db)
    .await?
    .rows_affected();

    if rows == 0 {
        return Err(JarvisError::NotFound("conversation introuvable".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_messages(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<Json<Vec<Message>>> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM jarvis.conversations WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;

    if exists == 0 {
        return Err(JarvisError::NotFound("conversation introuvable".into()));
    }

    let messages = sqlx::query_as::<_, Message>(
        r#"SELECT id, conversation_id, role, content, prompt_tokens, completion_tokens, created_at
           FROM jarvis.messages
           WHERE conversation_id = $1
           ORDER BY created_at ASC"#,
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;

    Ok(Json(messages))
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use kubuno_db::{dialect::SqlType, params};
use uuid::Uuid;

use crate::{
    errors::{AssistantError, AssistantResult},
    handlers::agent_access,
    middleware::AssistantUser,
    models::{Conversation, ConversationSummary, CreateConversationDto, Message, UpdateConversationDto},
    state::AppState,
    sync,
};

/// Columns of a conversation as the API returns it. A macro so it expands to a
/// string *literal*: the queries stay compile-time `&'static str`.
macro_rules! cols {
    () => {
        "id, owner_id, agent_id, title, model_id, message_count, total_tokens, \
         is_pinned, is_archived, folder_id, position, created_at, updated_at"
    };
}

/// Columns of a message as the API returns it.
macro_rules! msg_cols {
    () => {
        "id, conversation_id, role, content, tool_calls, prompt_tokens, \
         completion_tokens, feedback, created_at"
    };
}

pub async fn list_conversations(
    State(st): State<AppState>,
    user: AssistantUser,
) -> AssistantResult<Json<Vec<ConversationSummary>>> {
    let rows = st.db.fetch_all_as::<Conversation>(
        concat!(
            "SELECT ", cols!(),
            " FROM assistant.conversations \
              WHERE owner_id = $1 AND is_archived = false AND is_trashed = false \
              ORDER BY is_pinned DESC, position ASC, updated_at DESC",
        ),
        params![user.id],
    )
    .await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for conv in rows {
        let last_message: Option<String> = st.db.fetch_optional_scalar::<String>(
            "SELECT content FROM assistant.messages WHERE conversation_id = $1 ORDER BY created_at DESC LIMIT 1",
            params![conv.id],
        )
        .await?;

        summaries.push(ConversationSummary { conversation: conv, last_message });
    }

    Ok(Json(summaries))
}

pub async fn get_conversation(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<Json<Conversation>> {
    let conv = st.db.fetch_optional_as::<Conversation>(
        concat!(
            "SELECT ", cols!(),
            " FROM assistant.conversations WHERE id = $1 AND owner_id = $2",
        ),
        params![id, user.id],
    )
    .await?
    .ok_or_else(|| AssistantError::NotFound("conversation introuvable".into()))?;

    Ok(Json(conv))
}

pub async fn create_conversation(
    State(st): State<AppState>,
    user: AssistantUser,
    Json(dto): Json<CreateConversationDto>,
) -> AssistantResult<(StatusCode, Json<Conversation>)> {
    let policy = st.instance();
    let default_model = st.providers().ollama.default_model().to_string();
    let model_id  = dto.model.unwrap_or(default_model);
    // Instance policy: only the listed models may be used.
    if !policy.model_allowed(&model_id) {
        return Err(AssistantError::Validation(format!(
            "Modèle non autorisé sur cette instance : {model_id}"
        )));
    }
    let valid_providers = ["ollama", "openai", "anthropic", "google"];
    let provider = dto.provider
        .filter(|p| valid_providers.contains(&p.as_str()))
        .unwrap_or_else(|| "ollama".to_string());
    // Instance policy: an instance pinned to what it hosts refuses a remote one
    // outright, rather than letting the conversation fail on its first message.
    if provider != "ollama" && !policy.allow_cloud_providers {
        return Err(AssistantError::Validation(
            "Cette instance n'autorise que le moteur qu'elle héberge.".into(),
        ));
    }

    // Pinning a conversation to an agent IS an access to that agent: the id must
    // be one the user may reach, otherwise anybody could bind a conversation of
    // their own to a stranger's agent and have the model run — and reveal — its
    // system prompt. An id that is unknown and one that belongs to somebody else
    // get the very same answer, so the route cannot be used to probe ids.
    let agent_id = match dto.agent_id {
        Some(id) => {
            if agent_access::reachable(&st.db, id, user.id).await?.is_none() {
                return Err(agent_access::not_found());
            }
            Some(id)
        }
        None => {
            st.db.fetch_optional_scalar::<Uuid>(
                "SELECT id FROM assistant.agents WHERE is_system = true ORDER BY created_at LIMIT 1",
                params![],
            )
            .await?
        }
    };

    let title = dto.title.as_deref().map(ToOwned::to_owned);
    let id = dto.id.unwrap_or_else(kubuno_db::new_id);

    let mut tx = st.db.begin().await?;
    let seq = sync::next_conv_seq(&mut tx).await?;
    tx.execute(
        "INSERT INTO assistant.conversations \
             (id, owner_id, agent_id, title, model_id, provider, change_seq) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        params![id, user.id, agent_id, title, model_id, provider, seq],
    )
    .await?;
    tx.commit().await?;

    let conv = st.db.fetch_one_as::<Conversation>(
        concat!("SELECT ", cols!(), " FROM assistant.conversations WHERE id = $1"),
        params![id],
    )
    .await?;

    Ok((StatusCode::CREATED, Json(conv)))
}

pub async fn update_conversation(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateConversationDto>,
) -> AssistantResult<Json<Conversation>> {
    // folder_id: Option<Option<Uuid>> distinguishes "leave as is" from "move".
    let (set_folder, folder_val) = match dto.folder_id { Some(v) => (true, v), None => (false, None) };

    // When moving into a folder, verify it belongs to the caller BEFORE writing.
    // (The old single UPDATE folded this into an EXISTS guard; a guarded UPDATE
    // is not portable, so the check is lifted into Rust.) An unreachable folder
    // yields the same "not found" the whole route gives, disclosing nothing.
    if let Some(fid) = folder_val {
        // A bare `1` is int4 on PostgreSQL and will not decode into i64; cast it
        // so the existence probe yields a BIGINT on every engine.
        let owns = st.db.fetch_optional_scalar::<i64>(
            &format!(
                "SELECT {} FROM assistant.folders WHERE id = $1 AND owner_id = $2",
                st.db.backend().cast("1", SqlType::BigInt)
            ),
            params![fid, user.id],
        )
        .await?;
        if owns.is_none() {
            return Err(AssistantError::NotFound("conversation introuvable".into()));
        }
    }

    let mut tx = st.db.begin().await?;
    let seq = sync::next_conv_seq(&mut tx).await?;
    let affected = tx.execute(
        "UPDATE assistant.conversations SET \
             title       = COALESCE($1, title), \
             is_pinned   = COALESCE($2, is_pinned), \
             is_archived = COALESCE($3, is_archived), \
             model_id    = COALESCE($4, model_id), \
             folder_id   = CASE WHEN $5 THEN $6 ELSE folder_id END, \
             position    = COALESCE($7, position), \
             change_seq  = $8 \
         WHERE id = $9 AND owner_id = $10",
        params![
            dto.title.as_deref(),
            dto.is_pinned,
            dto.is_archived,
            dto.model.as_deref(),
            set_folder,
            folder_val,
            dto.position,
            seq,
            id,
            user.id
        ],
    )
    .await?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(AssistantError::NotFound("conversation introuvable".into()));
    }
    tx.commit().await?;

    let conv = st.db.fetch_one_as::<Conversation>(
        concat!("SELECT ", cols!(), " FROM assistant.conversations WHERE id = $1"),
        params![id],
    )
    .await?;

    Ok(Json(conv))
}

pub async fn delete_conversation(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<StatusCode> {
    // The conversation's messages follow through the foreign-key cascade; they
    // carry no delta feed of their own, so only the conversation gets a
    // tombstone, written in the same transaction as the delete.
    let mut tx = st.db.begin().await?;
    let seq = sync::next_conv_seq(&mut tx).await?;
    let affected = tx.execute(
        "DELETE FROM assistant.conversations WHERE id = $1 AND owner_id = $2",
        params![id, user.id],
    )
    .await?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(AssistantError::NotFound("conversation introuvable".into()));
    }
    kubuno_db::journal::record_tombstone(
        &mut tx,
        sync::CONV_TOMBSTONES,
        id,
        user.id,
        seq,
    )
    .await?;
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_messages(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<Json<Vec<Message>>> {
    // A bare `1` is int4 on PostgreSQL and will not decode into i64; cast it so
    // the existence probe yields a BIGINT on every engine.
    let exists = st.db.fetch_optional_scalar::<i64>(
        &format!(
            "SELECT {} FROM assistant.conversations WHERE id = $1 AND owner_id = $2",
            st.db.backend().cast("1", SqlType::BigInt)
        ),
        params![id, user.id],
    )
    .await?;

    if exists.is_none() {
        return Err(AssistantError::NotFound("conversation introuvable".into()));
    }

    let messages = st.db.fetch_all_as::<Message>(
        concat!(
            "SELECT ", msg_cols!(),
            " FROM assistant.messages WHERE conversation_id = $1 ORDER BY created_at ASC",
        ),
        params![id],
    )
    .await?;

    Ok(Json(messages))
}

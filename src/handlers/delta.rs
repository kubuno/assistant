//! Sync deltas for the local-first pull (conversations / folders / agents) — same
//! contract as the office sub-modules: owner-scoped changes past `cursor`
//! (monotonic change_seq), live rows + tombstones, ordered, paginated.
//! `kind ∈ modified | deleted`. Conversation changes carry their messages inline;
//! agent changes include the shared system agents (owner_id NULL).

use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{errors::AssistantResult, middleware::AssistantUser, state::AppState};

#[derive(serde::Deserialize)]
pub struct DeltaQuery {
    #[serde(default)]
    cursor: i64,
    limit: Option<i64>,
}

/// GET /conversations/delta — conversations (all, incl. archived) + tombstones,
/// each modified change inlining its full message list.
pub async fn conversations_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        r#"SELECT id, change_seq, 'live' AS src FROM assistant.conversations WHERE owner_id=$1 AND change_seq>$2
           UNION ALL
           SELECT id, change_seq, 'tomb' AS src FROM assistant.conv_tombstones WHERE owner_id=$1 AND change_seq>$2
           ORDER BY change_seq LIMIT $3"#,
    )
    .bind(user.id)
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&st.db)
    .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.1).unwrap_or(q.cursor);

    let mut changes = Vec::with_capacity(rows.len());
    for (id, seq, src) in &rows {
        if src == "tomb" {
            changes.push(json!({ "uuid": id, "kind": "deleted", "change_seq": seq }));
            continue;
        }
        let conv: Option<Value> = sqlx::query_scalar(
            r#"SELECT to_jsonb(c) FROM (
                   SELECT id, owner_id AS user_id, agent_id, title, model_id AS model, message_count,
                          total_tokens, is_pinned, is_archived, is_trashed, folder_id, position,
                          created_at, updated_at
                   FROM assistant.conversations WHERE id=$1) c"#,
        )
        .bind(id)
        .fetch_optional(&st.db)
        .await?;
        let Some(conv) = conv else { continue };
        let messages: Vec<Value> = sqlx::query_scalar(
            r#"SELECT to_jsonb(m) FROM (
                   SELECT id, conversation_id, role, content, tool_calls, prompt_tokens,
                          completion_tokens, feedback, created_at
                   FROM assistant.messages WHERE conversation_id=$1 ORDER BY created_at, id) m"#,
        )
        .bind(id)
        .fetch_all(&st.db)
        .await?;
        changes.push(json!({
            "uuid": id, "kind": "modified", "change_seq": seq,
            "conversation": conv, "messages": messages,
        }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}

/// GET /folders/delta
pub async fn folders_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        r#"SELECT id, change_seq, 'live' AS src FROM assistant.folders WHERE owner_id=$1 AND change_seq>$2
           UNION ALL
           SELECT id, change_seq, 'tomb' AS src FROM assistant.folder_tombstones WHERE owner_id=$1 AND change_seq>$2
           ORDER BY change_seq LIMIT $3"#,
    )
    .bind(user.id)
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&st.db)
    .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.1).unwrap_or(q.cursor);
    let mut changes = Vec::with_capacity(rows.len());
    for (id, seq, src) in &rows {
        if src == "tomb" {
            changes.push(json!({ "uuid": id, "kind": "deleted", "change_seq": seq }));
            continue;
        }
        let folder: Option<Value> = sqlx::query_scalar(
            r#"SELECT to_jsonb(f) FROM (
                   SELECT id, owner_id, name, color, position, created_at, updated_at
                   FROM assistant.folders WHERE id=$1) f"#,
        )
        .bind(id)
        .fetch_optional(&st.db)
        .await?;
        let Some(folder) = folder else { continue };
        changes.push(json!({ "uuid": id, "kind": "modified", "change_seq": seq, "folder": folder }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}

/// GET /agents/delta — the owner's agents ∪ shared system agents (owner_id NULL).
pub async fn agents_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        r#"SELECT id, change_seq, 'live' AS src FROM assistant.agents
               WHERE (owner_id=$1 OR is_system=true) AND change_seq>$2
           UNION ALL
           SELECT id, change_seq, 'tomb' AS src FROM assistant.agent_tombstones
               WHERE owner_id=$1 AND change_seq>$2
           ORDER BY change_seq LIMIT $3"#,
    )
    .bind(user.id)
    .bind(q.cursor)
    .bind(limit)
    .fetch_all(&st.db)
    .await?;
    let has_more = rows.len() as i64 == limit;
    let new_cursor = rows.last().map(|r| r.1).unwrap_or(q.cursor);
    let mut changes = Vec::with_capacity(rows.len());
    for (id, seq, src) in &rows {
        if src == "tomb" {
            changes.push(json!({ "uuid": id, "kind": "deleted", "change_seq": seq }));
            continue;
        }
        let agent: Option<Value> = sqlx::query_scalar(
            r#"SELECT to_jsonb(a) FROM (
                   SELECT id, name, description, system_prompt, preferred_model AS default_model,
                          avatar_emoji, avatar_color, prompt_suggestions, is_system,
                          owner_id AS created_by, created_at, updated_at
                   FROM assistant.agents WHERE id=$1) a"#,
        )
        .bind(id)
        .fetch_optional(&st.db)
        .await?;
        let Some(agent) = agent else { continue };
        changes.push(json!({ "uuid": id, "kind": "modified", "change_seq": seq, "agent": agent }));
    }
    Ok(Json(json!({ "changes": changes, "cursor": new_cursor, "has_more": has_more })))
}

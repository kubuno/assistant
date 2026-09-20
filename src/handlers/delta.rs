//! Sync deltas for the local-first pull (conversations / folders / agents):
//! owner-scoped changes past `cursor` (monotonic change_seq), live rows +
//! tombstones, ordered, paginated. `kind ∈ modified | deleted`. Conversation
//! changes carry their messages inline; agent changes include the shared system
//! agents (owner_id NULL).
//!
//! The change feed comes from `kubuno_db::journal::changes_since` (the portable
//! `live UNION ALL tombstones` the module used to build by hand); the live rows
//! are then fetched by id with `DbQueryBuilder::push_in`, which renders the
//! `IN (...)` list — and `IN (NULL)` for an empty page — on every engine. Agents
//! need one extra half in their feed (the shared system agents, owner NULL), so
//! their union is spelled out here rather than taken from `changes_since`.

use axum::{
    extract::{Query, State},
    Json,
};
use kubuno_db::{
    dialect::SqlType,
    journal::{changes_since, Change},
    params, DbQueryBuilder,
};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    errors::AssistantResult,
    handlers::agent_access::visible_sql,
    middleware::AssistantUser,
    models::{Agent, Folder, Message},
    state::AppState,
    sync,
};

#[derive(serde::Deserialize)]
pub struct DeltaQuery {
    #[serde(default)]
    cursor: i64,
    limit: Option<i64>,
}

/// `SELECT <select> WHERE <key> IN (<ids>) [<order>]`, built so the `IN` list
/// (or `IN (NULL)` when empty) is spelled for the pool's engine.
async fn select_in<T: kubuno_db::FromAnyRow>(
    state: &AppState,
    select: &str,
    key: &str,
    ids: &[Uuid],
    order: &'static str,
) -> Result<Vec<T>, sqlx::Error> {
    let mut qb = DbQueryBuilder::new(state.db.backend(), select);
    qb.push(" WHERE ").push(key).push_in(ids.iter().copied());
    if !order.is_empty() {
        qb.push(order);
    }
    qb.fetch_all_as::<T>(&state.db).await
}

fn cursor_and_more(changes: &[Change], prev: i64, limit: i64) -> (i64, bool) {
    let has_more = changes.len() as i64 == limit;
    let new_cursor = changes.last().map(|c| c.change_seq).unwrap_or(prev);
    (new_cursor, has_more)
}

/// The conversation payload of a delta, including `is_trashed` (which the plain
/// `Conversation` model omits). Column aliases produce the JSON key names the
/// client already expects (`user_id`, `model`).
#[derive(sqlx::FromRow, Serialize)]
struct ConvDelta {
    id:            Uuid,
    user_id:       Uuid,
    agent_id:      Option<Uuid>,
    title:         Option<String>,
    model:         String,
    message_count: i32,
    total_tokens:  i32,
    is_pinned:     bool,
    is_archived:   bool,
    is_trashed:    bool,
    folder_id:     Option<Uuid>,
    position:      i32,
    created_at:    chrono::DateTime<chrono::Utc>,
    updated_at:    chrono::DateTime<chrono::Utc>,
}

/// GET /conversations/delta — conversations (all, incl. archived / trashed) +
/// tombstones, each modified change inlining its full message list.
pub async fn conversations_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let changes = changes_since(
        &st.db, sync::CONVERSATIONS_TABLE, sync::CONV_TOMBSTONES, user.id, q.cursor, limit,
    )
    .await?;
    let (new_cursor, has_more) = cursor_and_more(&changes, q.cursor, limit);
    let live_ids: Vec<Uuid> = changes.iter().filter(|c| !c.deleted).map(|c| c.id).collect();

    // The id list is owner-scoped; the row fetch repeats the scope so a change of
    // ownership between the two queries can never hand out somebody else's row.
    let convs: Vec<ConvDelta> = if live_ids.is_empty() {
        Vec::new()
    } else {
        let mut qb = DbQueryBuilder::new(
            st.db.backend(),
            "SELECT id, owner_id AS user_id, agent_id, title, model_id AS model, message_count, \
                    total_tokens, is_pinned, is_archived, is_trashed, folder_id, position, \
                    created_at, updated_at FROM assistant.conversations",
        );
        qb.push(" WHERE owner_id = ").push_bind(user.id).push(" AND id").push_in(live_ids.iter().copied());
        qb.fetch_all_as::<ConvDelta>(&st.db).await?
    };
    let messages: Vec<Message> = select_in(
        &st,
        "SELECT id, conversation_id, role, content, tool_calls, prompt_tokens, \
                completion_tokens, feedback, created_at FROM assistant.messages",
        "conversation_id",
        &live_ids,
        " ORDER BY conversation_id, created_at, id",
    )
    .await?;

    let mut msg_map: std::collections::HashMap<Uuid, Vec<&Message>> = Default::default();
    for m in &messages {
        msg_map.entry(m.conversation_id).or_default().push(m);
    }
    let conv_map: std::collections::HashMap<Uuid, &ConvDelta> =
        convs.iter().map(|c| (c.id, c)).collect();

    let empty_m: Vec<&Message> = Vec::new();
    let mut out = Vec::with_capacity(changes.len());
    for c in &changes {
        if c.deleted {
            out.push(json!({ "uuid": c.id, "kind": "deleted", "change_seq": c.change_seq }));
        } else if let Some(conv) = conv_map.get(&c.id) {
            out.push(json!({
                "uuid": c.id,
                "kind": "modified",
                "change_seq": c.change_seq,
                "conversation": conv,
                "messages": msg_map.get(&c.id).unwrap_or(&empty_m),
            }));
        }
    }
    Ok(Json(json!({ "changes": out, "cursor": new_cursor, "has_more": has_more })))
}

/// GET /folders/delta
pub async fn folders_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let changes = changes_since(
        &st.db, sync::FOLDERS_TABLE, sync::FOLDER_TOMBSTONES, user.id, q.cursor, limit,
    )
    .await?;
    let (new_cursor, has_more) = cursor_and_more(&changes, q.cursor, limit);
    let live_ids: Vec<Uuid> = changes.iter().filter(|c| !c.deleted).map(|c| c.id).collect();

    let folders: Vec<Folder> = if live_ids.is_empty() {
        Vec::new()
    } else {
        let mut qb = DbQueryBuilder::new(
            st.db.backend(),
            "SELECT id, owner_id, name, color, position, created_at, updated_at \
             FROM assistant.folders",
        );
        qb.push(" WHERE owner_id = ").push_bind(user.id).push(" AND id").push_in(live_ids.iter().copied());
        qb.fetch_all_as::<Folder>(&st.db).await?
    };
    let folder_map: std::collections::HashMap<Uuid, &Folder> =
        folders.iter().map(|f| (f.id, f)).collect();

    let mut out = Vec::with_capacity(changes.len());
    for c in &changes {
        if c.deleted {
            out.push(json!({ "uuid": c.id, "kind": "deleted", "change_seq": c.change_seq }));
        } else if let Some(f) = folder_map.get(&c.id) {
            out.push(json!({ "uuid": c.id, "kind": "modified", "change_seq": c.change_seq, "folder": f }));
        }
    }
    Ok(Json(json!({ "changes": out, "cursor": new_cursor, "has_more": has_more })))
}

/// The agents feed cannot use `changes_since`: it is owner-scoped, but the agent
/// delta also carries the shared system agents (owner_id NULL). One row of the
/// hand-built union.
#[derive(sqlx::FromRow)]
struct AgentChangeRow {
    id:         Uuid,
    change_seq: i64,
    is_deleted: i64,
}

/// GET /agents/delta — the owner's agents ∪ shared system agents (owner_id NULL).
pub async fn agents_delta(
    State(st): State<AppState>,
    user: AssistantUser,
    Query(q): Query<DeltaQuery>,
) -> AssistantResult<Json<Value>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 500);
    let backend = st.db.backend();
    // Cast the marker so it decodes as i64 on every engine (a bare 0/1 is int4 on
    // PostgreSQL and would not decode into i64).
    let live_flag = backend.cast("0", SqlType::BigInt);
    let tomb_flag = backend.cast("1", SqlType::BigInt);
    let sql = format!(
        "SELECT id, change_seq, {live_flag} AS is_deleted FROM assistant.agents \
             WHERE {visible} AND change_seq > $2 \
         UNION ALL \
         SELECT id, change_seq, {tomb_flag} AS is_deleted FROM assistant.agent_tombstones \
             WHERE owner_id = $3 AND change_seq > $4 \
         ORDER BY change_seq LIMIT $5",
        visible = visible_sql!(),
    );
    let rows: Vec<AgentChangeRow> = st.db.fetch_all_as::<AgentChangeRow>(
        &sql,
        params![user.id, q.cursor, user.id, q.cursor, limit],
    )
    .await?;
    let changes: Vec<Change> = rows
        .into_iter()
        .map(|r| Change { id: r.id, change_seq: r.change_seq, deleted: r.is_deleted != 0 })
        .collect();
    let (new_cursor, has_more) = cursor_and_more(&changes, q.cursor, limit);
    let live_ids: Vec<Uuid> = changes.iter().filter(|c| !c.deleted).map(|c| c.id).collect();

    // Same scope as the id list, repeated on the row itself: this payload carries
    // `system_prompt`, so it must never be reachable by id alone. The `Agent`
    // model's serde field names already match the client's expected keys
    // (`default_model`, `created_by`).
    let agents: Vec<Agent> = if live_ids.is_empty() {
        Vec::new()
    } else {
        let mut qb = DbQueryBuilder::new(
            backend,
            "SELECT id, name, description, system_prompt, preferred_model, avatar_emoji, \
                    avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at \
             FROM assistant.agents",
        );
        qb.push(" WHERE (owner_id = ").push_bind(user.id).push(" OR is_system = ").push_bind(true).push(")");
        qb.push(" AND id").push_in(live_ids.iter().copied());
        qb.fetch_all_as::<Agent>(&st.db).await?
    };
    let agent_map: std::collections::HashMap<Uuid, &Agent> =
        agents.iter().map(|a| (a.id, a)).collect();

    let mut out = Vec::with_capacity(changes.len());
    for c in &changes {
        if c.deleted {
            out.push(json!({ "uuid": c.id, "kind": "deleted", "change_seq": c.change_seq }));
        } else if let Some(a) = agent_map.get(&c.id) {
            out.push(json!({ "uuid": c.id, "kind": "modified", "change_seq": c.change_seq, "agent": a }));
        }
    }
    Ok(Json(json!({ "changes": out, "cursor": new_cursor, "has_more": has_more })))
}

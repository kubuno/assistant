use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use kubuno_db::params;
use uuid::Uuid;

use crate::{
    errors::{AssistantError, AssistantResult},
    handlers::agent_access::{self, reachable_sql, visible_sql},
    middleware::AssistantUser,
    models::{Agent, CreateAgentDto, UpdateAgentDto},
    state::AppState,
    sync,
};

/// Columns of an agent as the API returns it. Shared by the queries below so a
/// column added to one is added to all.
///
/// A macro rather than a constant so that it expands to a string *literal*: the
/// queries are then assembled with `concat!` and stay compile-time
/// `&'static str`, which the driver accepts without an injection audit.
macro_rules! agent_columns {
    () => {
        "id, name, description, system_prompt, preferred_model, avatar_emoji, \
         avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at"
    };
}

pub async fn list_agents(
    State(st): State<AppState>,
    user: AssistantUser,
) -> AssistantResult<Json<Vec<Agent>>> {
    let agents = st.db.fetch_all_as::<Agent>(
        concat!(
            "SELECT ", agent_columns!(),
            " FROM assistant.agents WHERE ", visible_sql!(),
            " ORDER BY created_at",
        ),
        params![user.id],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "liste des agents");
        AssistantError::from(e)
    })?;

    Ok(Json(agents))
}

pub async fn get_agent(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<Json<Agent>> {
    let agent = st.db.fetch_optional_as::<Agent>(
        concat!(
            "SELECT ", agent_columns!(),
            " FROM assistant.agents WHERE ", reachable_sql!(),
        ),
        params![id, user.id],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, agent_id = %id, "lecture d'un agent");
        AssistantError::from(e)
    })?
    .ok_or_else(agent_access::not_found)?;

    Ok(Json(agent))
}

pub async fn create_agent(
    State(st): State<AppState>,
    user: AssistantUser,
    Json(dto): Json<CreateAgentDto>,
) -> AssistantResult<(StatusCode, Json<Agent>)> {
    // Instance policy: an administrator may keep the agent catalogue to the ones
    // the module ships with.
    if !st.instance().allow_custom_agents {
        return Err(AssistantError::Forbidden);
    }
    if dto.name.trim().is_empty() {
        return Err(AssistantError::Validation("Le nom de l'agent est requis".into()));
    }

    let id = dto.id.unwrap_or_else(kubuno_db::new_id);
    let now = chrono::Utc::now();

    // "Skip it if the id is already there" rather than letting the primary key
    // raise: a client-minted id that is already taken must not surface as a
    // database error, which would answer differently for a free id and a taken
    // one. Spelled with the dialect helpers so it is `INSERT IGNORE` on MySQL and
    // `ON CONFLICT (id) DO NOTHING` on PostgreSQL/SQLite.
    let backend = st.db.backend();
    let insert_sql = format!(
        "INSERT {ignore}INTO assistant.agents \
             (id, name, description, system_prompt, preferred_model, owner_id, change_seq, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9){do_nothing}",
        ignore = backend.insert_ignore_prefix(),
        do_nothing = backend.on_conflict_do_nothing(&["id"]),
    );

    let mut tx = st.db.begin().await?;
    let seq = sync::next_agent_seq(&mut tx).await?;
    let affected = tx.execute(
        &insert_sql,
        params![
            id,
            dto.name.trim(),
            dto.description.as_deref(),
            &dto.system_prompt,
            dto.default_model.as_deref(),
            user.id,
            seq,
            now,
            now
        ],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "création d'un agent");
        AssistantError::from(e)
    })?;
    if affected > 0 {
        tx.commit().await?;
        let agent = st.db.fetch_one_as::<Agent>(
            concat!("SELECT ", agent_columns!(), " FROM assistant.agents WHERE id = $1"),
            params![id],
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(agent)));
    }
    // The id is taken; the seq we took is discarded with the rollback.
    tx.rollback().await?;

    // A local-first client replaying its own creation gets its agent back; an id
    // held by anybody else gets the plain "not found" — the caller learns
    // nothing about whose it is or what it holds.
    let Some(id) = dto.id else {
        tracing::error!("collision d'identifiant sur un agent généré par la base");
        return Err(AssistantError::Internal(anyhow::anyhow!(
            "collision d'identifiant d'agent"
        )));
    };
    let existing = st.db.fetch_optional_as::<Agent>(
        concat!(
            "SELECT ", agent_columns!(),
            " FROM assistant.agents WHERE id = $1 AND owner_id = $2",
        ),
        params![id, user.id],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, agent_id = %id, "relecture d'un agent après conflit d'identifiant");
        AssistantError::from(e)
    })?
    .ok_or_else(agent_access::not_found)?;

    Ok((StatusCode::OK, Json(existing)))
}

pub async fn update_agent(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateAgentDto>,
) -> AssistantResult<Json<Agent>> {
    // Scoped to the caller: an agent owned by somebody else is "not found", the
    // same answer an unknown id gets. Only the shared system agents — which every
    // account already sees — are refused as forbidden, because they are read-only.
    match agent_access::reachable(&st.db, id, user.id).await? {
        None        => return Err(agent_access::not_found()),
        Some(true)  => return Err(AssistantError::Forbidden),
        Some(false) => {}
    }

    let mut tx = st.db.begin().await?;
    let seq = sync::next_agent_seq(&mut tx).await?;
    let affected = tx.execute(
        "UPDATE assistant.agents SET \
             name            = COALESCE($1, name), \
             description     = COALESCE($2, description), \
             system_prompt   = COALESCE($3, system_prompt), \
             preferred_model = COALESCE($4, preferred_model), \
             change_seq      = $5 \
         WHERE id = $6 AND owner_id = $7",
        params![
            dto.name.as_deref(),
            dto.description.as_deref(),
            dto.system_prompt.as_deref(),
            dto.default_model.as_deref(),
            seq,
            id,
            user.id
        ],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, agent_id = %id, "mise à jour d'un agent");
        AssistantError::from(e)
    })?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(agent_access::not_found());
    }
    tx.commit().await?;

    let agent = st.db.fetch_one_as::<Agent>(
        concat!("SELECT ", agent_columns!(), " FROM assistant.agents WHERE id = $1"),
        params![id],
    )
    .await?;

    Ok(Json(agent))
}

pub async fn delete_agent(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<StatusCode> {
    match agent_access::reachable(&st.db, id, user.id).await? {
        None        => return Err(agent_access::not_found()),
        Some(true)  => return Err(AssistantError::Forbidden),
        Some(false) => {}
    }

    let mut tx = st.db.begin().await?;
    let seq = sync::next_agent_seq(&mut tx).await?;
    let affected = tx.execute(
        "DELETE FROM assistant.agents WHERE id = $1 AND owner_id = $2",
        params![id, user.id],
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, agent_id = %id, "suppression d'un agent");
        AssistantError::from(e)
    })?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(agent_access::not_found());
    }
    kubuno_db::journal::record_tombstone(
        &mut tx,
        sync::AGENT_TOMBSTONES,
        id,
        user.id,
        seq,
    )
    .await?;
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

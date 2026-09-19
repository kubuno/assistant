use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    errors::{AssistantError, AssistantResult},
    handlers::agent_access::{self, reachable_sql, visible_sql},
    middleware::AssistantUser,
    models::{Agent, CreateAgentDto, UpdateAgentDto},
    state::AppState,
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
    let agents = sqlx::query_as::<_, Agent>(concat!(
        "SELECT ",
        agent_columns!(),
        " FROM assistant.agents WHERE ",
        visible_sql!(),
        " ORDER BY created_at",
    ))
        .bind(user.id)
        .fetch_all(&st.db)
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
    let agent = sqlx::query_as::<_, Agent>(concat!(
        "SELECT ",
        agent_columns!(),
        " FROM assistant.agents WHERE ",
        reachable_sql!(),
    ))
        .bind(id)
        .bind(user.id)
        .fetch_optional(&st.db)
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

    // `ON CONFLICT DO NOTHING` rather than letting the primary key raise: a
    // client-minted id that is already taken must not surface as a database
    // error, which would answer differently for a free id and for a taken one.
    let created = sqlx::query_as::<_, Agent>(concat!(
        r#"INSERT INTO assistant.agents (id, name, description, system_prompt, preferred_model, owner_id)
           VALUES (COALESCE($6, uuid_generate_v4()), $1, $2, $3, $4, $5)
           ON CONFLICT (id) DO NOTHING
           RETURNING "#,
        agent_columns!(),
    ))
        .bind(dto.name.trim())
        .bind(dto.description.as_deref())
        .bind(&dto.system_prompt)
        .bind(dto.default_model.as_deref())
        .bind(user.id)
        .bind(dto.id)
        .fetch_optional(&st.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "création d'un agent");
            AssistantError::from(e)
        })?;

    if let Some(agent) = created {
        return Ok((StatusCode::CREATED, Json(agent)));
    }

    // The id is taken. A local-first client replaying its own creation gets its
    // agent back; an id held by anybody else gets the plain "not found" — the
    // caller learns nothing about whose it is or what it holds.
    let Some(id) = dto.id else {
        tracing::error!("collision d'identifiant sur un agent généré par la base");
        return Err(AssistantError::Internal(anyhow::anyhow!(
            "collision d'identifiant d'agent"
        )));
    };
    let existing = sqlx::query_as::<_, Agent>(concat!(
        "SELECT ",
        agent_columns!(),
        " FROM assistant.agents WHERE id = $1 AND owner_id = $2",
    ))
        .bind(id)
        .bind(user.id)
        .fetch_optional(&st.db)
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

    let agent = sqlx::query_as::<_, Agent>(concat!(
        r#"UPDATE assistant.agents SET
               name            = COALESCE($3, name),
               description     = COALESCE($4, description),
               system_prompt   = COALESCE($5, system_prompt),
               preferred_model = COALESCE($6, preferred_model)
           WHERE id = $1 AND owner_id = $2
           RETURNING "#,
        agent_columns!(),
    ))
        .bind(id)
        .bind(user.id)
        .bind(dto.name.as_deref())
        .bind(dto.description.as_deref())
        .bind(dto.system_prompt.as_deref())
        .bind(dto.default_model.as_deref())
        .fetch_optional(&st.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, agent_id = %id, "mise à jour d'un agent");
            AssistantError::from(e)
        })?
        .ok_or_else(agent_access::not_found)?;

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

    sqlx::query("DELETE FROM assistant.agents WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, agent_id = %id, "suppression d'un agent");
            AssistantError::from(e)
        })?;

    Ok(StatusCode::NO_CONTENT)
}

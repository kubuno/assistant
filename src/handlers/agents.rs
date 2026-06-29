use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    errors::{JarvisError, JarvisResult},
    middleware::JarvisUser,
    models::{Agent, CreateAgentDto, UpdateAgentDto},
    state::AppState,
};

pub async fn list_agents(
    State(st): State<AppState>,
    user: JarvisUser,
) -> JarvisResult<Json<Vec<Agent>>> {
    let agents = sqlx::query_as::<_, Agent>(
        r#"SELECT id, name, description, system_prompt, preferred_model, avatar_emoji, avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at
           FROM jarvis.agents WHERE is_system = true OR owner_id = $1 ORDER BY created_at"#,
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;

    Ok(Json(agents))
}

pub async fn get_agent(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<Json<Agent>> {
    let agent = sqlx::query_as::<_, Agent>(
        r#"SELECT id, name, description, system_prompt, preferred_model, avatar_emoji, avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at
           FROM jarvis.agents WHERE id = $1 AND (is_system = true OR owner_id = $2)"#,
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("agent introuvable".into()))?;

    Ok(Json(agent))
}

pub async fn create_agent(
    State(st): State<AppState>,
    user: JarvisUser,
    Json(dto): Json<CreateAgentDto>,
) -> JarvisResult<(StatusCode, Json<Agent>)> {
    if dto.name.trim().is_empty() {
        return Err(JarvisError::Validation("Le nom de l'agent est requis".into()));
    }

    let agent = sqlx::query_as::<_, Agent>(
        r#"INSERT INTO jarvis.agents (name, description, system_prompt, preferred_model, owner_id)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, name, description, system_prompt, preferred_model, avatar_emoji, avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at"#,
    )
    .bind(dto.name.trim())
    .bind(dto.description.as_deref())
    .bind(&dto.system_prompt)
    .bind(dto.default_model.as_deref())
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;

    Ok((StatusCode::CREATED, Json(agent)))
}

pub async fn update_agent(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateAgentDto>,
) -> JarvisResult<Json<Agent>> {
    let is_system: Option<bool> = sqlx::query_scalar(
        "SELECT is_system FROM jarvis.agents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?;

    match is_system {
        None        => return Err(JarvisError::NotFound("agent introuvable".into())),
        Some(true)  => return Err(JarvisError::Forbidden),
        Some(false) => {}
    }

    let agent = sqlx::query_as::<_, Agent>(
        r#"UPDATE jarvis.agents SET
               name            = COALESCE($3, name),
               description     = COALESCE($4, description),
               system_prompt   = COALESCE($5, system_prompt),
               preferred_model = COALESCE($6, preferred_model)
           WHERE id = $1 AND owner_id = $2
           RETURNING id, name, description, system_prompt, preferred_model, avatar_emoji, avatar_color, prompt_suggestions, is_system, owner_id, created_at, updated_at"#,
    )
    .bind(id)
    .bind(user.id)
    .bind(dto.name.as_deref())
    .bind(dto.description.as_deref())
    .bind(dto.system_prompt.as_deref())
    .bind(dto.default_model.as_deref())
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("agent introuvable".into()))?;

    Ok(Json(agent))
}

pub async fn delete_agent(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<StatusCode> {
    let is_system: Option<bool> = sqlx::query_scalar(
        "SELECT is_system FROM jarvis.agents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?;

    match is_system {
        None        => return Err(JarvisError::NotFound("agent introuvable".into())),
        Some(true)  => return Err(JarvisError::Forbidden),
        Some(false) => {}
    }

    sqlx::query("DELETE FROM jarvis.agents WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

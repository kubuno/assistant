use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    errors::{AssistantError, AssistantResult},
    middleware::AssistantUser,
    models::{CreateFolderDto, Folder, UpdateFolderDto},
    state::AppState,
};

/// Columns of a folder as the API returns it. A macro rather than a constant so
/// that it expands to a string *literal*: the queries below are assembled with
/// `concat!` and stay compile-time `&'static str`, which the driver accepts
/// without an injection audit.
macro_rules! cols {
    () => {
        "id, owner_id, name, color, position, created_at, updated_at"
    };
}

pub async fn list_folders(
    State(st): State<AppState>,
    user: AssistantUser,
) -> AssistantResult<Json<Vec<Folder>>> {
    let folders = sqlx::query_as::<_, Folder>(
        "SELECT id, owner_id, name, color, position, created_at, updated_at
         FROM assistant.folders WHERE owner_id = $1 ORDER BY position, created_at",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(folders))
}

pub async fn create_folder(
    State(st): State<AppState>,
    user: AssistantUser,
    Json(dto): Json<CreateFolderDto>,
) -> AssistantResult<(StatusCode, Json<Folder>)> {
    let name = dto.name.trim();
    if name.is_empty() {
        return Err(AssistantError::Validation("Le nom du dossier est requis".into()));
    }
    // Position = à la fin.
    let pos: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position) + 1, 0) FROM assistant.folders WHERE owner_id = $1")
        .bind(user.id)
        .fetch_one(&st.db)
        .await?;
    let folder = sqlx::query_as::<_, Folder>(concat!(
        "INSERT INTO assistant.folders (id, owner_id, name, color, position) \
         VALUES (COALESCE($5, uuid_generate_v4()), $1, $2, $3, $4) RETURNING ",
        cols!(),
    ))
    .bind(user.id)
    .bind(name)
    .bind(dto.color.as_deref())
    .bind(pos)
    .bind(dto.id)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

pub async fn update_folder(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateFolderDto>,
) -> AssistantResult<Json<Folder>> {
    let folder = sqlx::query_as::<_, Folder>(concat!(
        r#"UPDATE assistant.folders SET
               name     = COALESCE($3, name),
               color    = COALESCE($4, color),
               position = COALESCE($5, position),
               updated_at = NOW()
           WHERE id = $1 AND owner_id = $2
           RETURNING "#,
        cols!(),
    ))
    .bind(id)
    .bind(user.id)
    .bind(dto.name.as_deref())
    .bind(dto.color.as_deref())
    .bind(dto.position)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| AssistantError::NotFound("dossier introuvable".into()))?;
    Ok(Json(folder))
}

pub async fn delete_folder(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<StatusCode> {
    // ON DELETE SET NULL détache les conversations (elles ne sont pas supprimées).
    let affected = sqlx::query("DELETE FROM assistant.folders WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AssistantError::NotFound("dossier introuvable".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

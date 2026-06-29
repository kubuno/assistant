use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    errors::{JarvisError, JarvisResult},
    middleware::JarvisUser,
    models::{CreateFolderDto, Folder, UpdateFolderDto},
    state::AppState,
};

const COLS: &str = "id, owner_id, name, color, position, created_at, updated_at";

pub async fn list_folders(
    State(st): State<AppState>,
    user: JarvisUser,
) -> JarvisResult<Json<Vec<Folder>>> {
    let folders = sqlx::query_as::<_, Folder>(
        "SELECT id, owner_id, name, color, position, created_at, updated_at
         FROM jarvis.folders WHERE owner_id = $1 ORDER BY position, created_at",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(folders))
}

pub async fn create_folder(
    State(st): State<AppState>,
    user: JarvisUser,
    Json(dto): Json<CreateFolderDto>,
) -> JarvisResult<(StatusCode, Json<Folder>)> {
    let name = dto.name.trim();
    if name.is_empty() {
        return Err(JarvisError::Validation("Le nom du dossier est requis".into()));
    }
    // Position = à la fin.
    let pos: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position) + 1, 0) FROM jarvis.folders WHERE owner_id = $1")
        .bind(user.id)
        .fetch_one(&st.db)
        .await?;
    let folder = sqlx::query_as::<_, Folder>(
        &format!("INSERT INTO jarvis.folders (owner_id, name, color, position) VALUES ($1, $2, $3, $4) RETURNING {COLS}"),
    )
    .bind(user.id)
    .bind(name)
    .bind(dto.color.as_deref())
    .bind(pos)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

pub async fn update_folder(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateFolderDto>,
) -> JarvisResult<Json<Folder>> {
    let folder = sqlx::query_as::<_, Folder>(
        &format!(
            r#"UPDATE jarvis.folders SET
                   name     = COALESCE($3, name),
                   color    = COALESCE($4, color),
                   position = COALESCE($5, position),
                   updated_at = NOW()
               WHERE id = $1 AND owner_id = $2
               RETURNING {COLS}"#,
        ),
    )
    .bind(id)
    .bind(user.id)
    .bind(dto.name.as_deref())
    .bind(dto.color.as_deref())
    .bind(dto.position)
    .fetch_optional(&st.db)
    .await?
    .ok_or_else(|| JarvisError::NotFound("dossier introuvable".into()))?;
    Ok(Json(folder))
}

pub async fn delete_folder(
    State(st): State<AppState>,
    user: JarvisUser,
    Path(id): Path<Uuid>,
) -> JarvisResult<StatusCode> {
    // ON DELETE SET NULL détache les conversations (elles ne sont pas supprimées).
    let affected = sqlx::query("DELETE FROM jarvis.folders WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(JarvisError::NotFound("dossier introuvable".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

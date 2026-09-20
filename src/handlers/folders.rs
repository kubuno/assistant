use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use kubuno_db::params;
use uuid::Uuid;

use crate::{
    errors::{AssistantError, AssistantResult},
    middleware::AssistantUser,
    models::{CreateFolderDto, Folder, UpdateFolderDto},
    state::AppState,
    sync,
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
    let folders = st.db.fetch_all_as::<Folder>(
        concat!(
            "SELECT ", cols!(),
            " FROM assistant.folders WHERE owner_id = $1 ORDER BY position, created_at",
        ),
        params![user.id],
    )
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
    // Position = à la fin. `MAX(position)` is NULL for the first folder, so it is
    // read as `Option` and the successor computed in Rust (portable across the
    // three engines' return types).
    let max_pos: Option<i32> = st.db.fetch_optional_scalar::<i32>(
        "SELECT MAX(position) FROM assistant.folders WHERE owner_id = $1",
        params![user.id],
    )
    .await?;
    let pos = max_pos.map(|p| p + 1).unwrap_or(0);

    let id = dto.id.unwrap_or_else(kubuno_db::new_id);
    let now = chrono::Utc::now();

    let mut tx = st.db.begin().await?;
    let seq = sync::next_folder_seq(&mut tx).await?;
    tx.execute(
        "INSERT INTO assistant.folders (id, owner_id, name, color, position, change_seq, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        params![id, user.id, name, dto.color.as_deref(), pos, seq, now, now],
    )
    .await?;
    tx.commit().await?;

    // DbTx has no typed fetch, so re-select the persisted row on the pool.
    let folder = st.db.fetch_one_as::<Folder>(
        concat!("SELECT ", cols!(), " FROM assistant.folders WHERE id = $1"),
        params![id],
    )
    .await?;
    Ok((StatusCode::CREATED, Json(folder)))
}

pub async fn update_folder(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateFolderDto>,
) -> AssistantResult<Json<Folder>> {
    let now = chrono::Utc::now();
    let mut tx = st.db.begin().await?;
    let seq = sync::next_folder_seq(&mut tx).await?;
    // `change_seq = $5` always assigns a fresh value, so the row genuinely
    // changes and `rows_affected` reflects the WHERE match on every engine
    // (MySQL reports 0 for an UPDATE that changed nothing).
    let affected = tx.execute(
        "UPDATE assistant.folders SET \
             name       = COALESCE($1, name), \
             color      = COALESCE($2, color), \
             position   = COALESCE($3, position), \
             updated_at = $4, \
             change_seq = $5 \
         WHERE id = $6 AND owner_id = $7",
        params![dto.name.as_deref(), dto.color.as_deref(), dto.position, now, seq, id, user.id],
    )
    .await?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(AssistantError::NotFound("dossier introuvable".into()));
    }
    tx.commit().await?;

    let folder = st.db.fetch_one_as::<Folder>(
        concat!("SELECT ", cols!(), " FROM assistant.folders WHERE id = $1"),
        params![id],
    )
    .await?;
    Ok(Json(folder))
}

pub async fn delete_folder(
    State(st): State<AppState>,
    user: AssistantUser,
    Path(id): Path<Uuid>,
) -> AssistantResult<StatusCode> {
    // Deleting a folder detaches its conversations (they are not deleted). The
    // PostgreSQL trigger design leaned on `ON DELETE SET NULL` firing the
    // conversation's BEFORE UPDATE bump; a foreign-key cascade runs no code on
    // MySQL/SQLite, so the detach — and the change_seq bump that lets a client
    // learn `folder_id` became NULL — is done explicitly here, in one
    // transaction with the folder's own delete and tombstone.
    let mut tx = st.db.begin().await?;
    let folder_seq = sync::next_folder_seq(&mut tx).await?;
    let conv_seq = sync::next_conv_seq(&mut tx).await?;
    tx.execute(
        "UPDATE assistant.conversations SET folder_id = NULL, change_seq = $1 \
         WHERE folder_id = $2 AND owner_id = $3",
        params![conv_seq, id, user.id],
    )
    .await?;
    let affected = tx.execute(
        "DELETE FROM assistant.folders WHERE id = $1 AND owner_id = $2",
        params![id, user.id],
    )
    .await?;
    if affected == 0 {
        tx.rollback().await?;
        return Err(AssistantError::NotFound("dossier introuvable".into()));
    }
    kubuno_db::journal::record_tombstone(
        &mut tx,
        sync::FOLDER_TOMBSTONES,
        id,
        user.id,
        folder_seq,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Folder {
    pub id:         Uuid,
    #[sqlx(rename = "owner_id")]
    pub owner_id:   Uuid,
    pub name:       String,
    pub color:      Option<String>,
    pub position:   i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderDto {
    pub name:  String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFolderDto {
    pub name:     Option<String>,
    pub color:    Option<String>,
    pub position: Option<i32>,
}

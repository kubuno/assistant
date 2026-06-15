use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Agent {
    pub id:               Uuid,
    pub name:             String,
    pub description:      Option<String>,
    pub system_prompt:    String,
    #[sqlx(rename = "preferred_model")]
    pub default_model:    Option<String>,
    pub is_system:        bool,
    #[sqlx(rename = "owner_id")]
    pub created_by:       Option<Uuid>,
    pub created_at:       DateTime<Utc>,
    pub updated_at:       DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentDto {
    pub name:          String,
    pub description:   Option<String>,
    pub system_prompt: String,
    pub default_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentDto {
    pub name:          Option<String>,
    pub description:   Option<String>,
    pub system_prompt: Option<String>,
    pub default_model: Option<String>,
}

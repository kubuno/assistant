use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id:            Uuid,
    #[sqlx(rename = "owner_id")]
    pub user_id:       Uuid,
    pub agent_id:      Option<Uuid>,
    pub title:         Option<String>,
    #[sqlx(rename = "model_id")]
    pub model:         String,
    pub message_count: i32,
    pub total_tokens:  i32,
    pub is_pinned:     bool,
    pub is_archived:   bool,
    pub created_at:    DateTime<Utc>,
    pub updated_at:    DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConversationDto {
    pub title:    Option<String>,
    pub agent_id: Option<Uuid>,
    pub model:    Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConversationDto {
    pub title:       Option<String>,
    pub is_pinned:   Option<bool>,
    pub is_archived: Option<bool>,
    pub model:       Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConversationSummary {
    pub conversation:  Conversation,
    pub last_message:  Option<String>,
}

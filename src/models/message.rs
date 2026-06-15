use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id:               Uuid,
    pub conversation_id:  Uuid,
    pub role:             String,
    pub content:          String,
    pub prompt_tokens:    i32,
    pub completion_tokens: i32,
    pub created_at:       DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageDto {
    pub content:  String,
    pub model:    Option<String>,
    pub agent_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    Delta   { content: String },
    Done    { message_id: Uuid, prompt_tokens: i32, completion_tokens: i32 },
    Error   { message: String },
}

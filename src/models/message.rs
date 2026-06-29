use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id:               Uuid,
    pub conversation_id:  Uuid,
    pub role:             String,
    pub content:          String,
    #[serde(default)]
    pub tool_calls:       serde_json::Value,
    pub prompt_tokens:    i32,
    pub completion_tokens: i32,
    /// Retour utilisateur : "like" / "dislike" / null.
    #[serde(default)]
    pub feedback:         Option<String>,
    pub created_at:       DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageDto {
    #[serde(default)]
    pub content:  String,
    pub model:    Option<String>,
    pub agent_id: Option<Uuid>,
    /// Régénère la dernière réponse : ne ré-insère pas le message utilisateur,
    /// supprime la dernière réponse assistant et relance depuis l'historique.
    #[serde(default)]
    pub regenerate: bool,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackDto {
    /// "like", "dislike" ou null pour retirer.
    pub feedback: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    Delta    { content: String },
    /// A tool call performed during the agentic loop (or a UI action to
    /// dispatch client-side). `call` is `{ tool, kind, args, result?, ui? }`.
    ToolCall { call: serde_json::Value },
    Done     { message_id: Uuid, prompt_tokens: i32, completion_tokens: i32 },
    Error    { message: String },
}

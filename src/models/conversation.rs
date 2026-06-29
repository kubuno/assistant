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
    /// Dossier de rattachement (NULL = sans dossier).
    pub folder_id:     Option<Uuid>,
    /// Ordre manuel dans la liste (glisser-déposer). Plus petit = plus haut.
    pub position:      i32,
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

// `Option<Option<T>>` permet de distinguer « champ absent » de « mis à null »
// (déplacer une conversation hors de tout dossier).
fn de_double_opt<'de, D, T>(de: D) -> Result<Option<Option<T>>, D::Error>
where D: serde::Deserializer<'de>, T: Deserialize<'de> {
    Ok(Some(Option::<T>::deserialize(de)?))
}

#[derive(Debug, Deserialize)]
pub struct UpdateConversationDto {
    pub title:       Option<String>,
    pub is_pinned:   Option<bool>,
    pub is_archived: Option<bool>,
    pub model:       Option<String>,
    #[serde(default, deserialize_with = "de_double_opt")]
    pub folder_id:   Option<Option<Uuid>>,
    pub position:    Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ConversationSummary {
    pub conversation:  Conversation,
    pub last_message:  Option<String>,
}

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AssistantError {
    #[error("Non authentifié")]
    Unauthorized,
    #[error("Accès refusé")]
    Forbidden,
    #[error("Ressource introuvable: {0}")]
    NotFound(String),
    #[error("Données invalides: {0}")]
    Validation(String),
    /// A provider could not be reached or refused the call.
    ///
    /// CAUTION: the message is rendered verbatim into the 503 the client receives, so
    /// whatever a provider client puts in here is PUBLIC: never a URL carrying a
    /// credential, and never a provider body that has not gone through
    /// `services::endpoint::redact`.
    #[error("Ollama indisponible: {0}")]
    OllamaUnavailable(String),
    #[error("Erreur base de données")]
    Database(#[from] sqlx::Error),
    #[error("Erreur interne")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AssistantError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            AssistantError::Unauthorized          => (StatusCode::UNAUTHORIZED,           "UNAUTHORIZED",       self.to_string()),
            AssistantError::Forbidden             => (StatusCode::FORBIDDEN,              "FORBIDDEN",          self.to_string()),
            AssistantError::NotFound(m)           => (StatusCode::NOT_FOUND,              "NOT_FOUND",          m.clone()),
            AssistantError::Validation(m)         => (StatusCode::UNPROCESSABLE_ENTITY,   "VALIDATION_ERROR",   m.clone()),
            AssistantError::OllamaUnavailable(m)  => (StatusCode::SERVICE_UNAVAILABLE,    "OLLAMA_UNAVAILABLE", m.clone()),
            AssistantError::Database(e)           => {
                tracing::error!(error = %e, "Erreur DB assistant");
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "Erreur base de données".into())
            }
            AssistantError::Internal(e)           => {
                tracing::error!(error = %e, "Erreur interne assistant");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Erreur interne".into())
            }
        };
        (status, Json(json!({ "error": code, "message": msg }))).into_response()
    }
}

pub type AssistantResult<T> = Result<T, AssistantError>;

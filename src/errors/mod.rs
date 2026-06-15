use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum JarvisError {
    #[error("Non authentifié")]
    Unauthorized,
    #[error("Accès refusé")]
    Forbidden,
    #[error("Ressource introuvable: {0}")]
    NotFound(String),
    #[error("Données invalides: {0}")]
    Validation(String),
    #[error("Ollama indisponible: {0}")]
    OllamaUnavailable(String),
    #[error("Erreur base de données")]
    Database(#[from] sqlx::Error),
    #[error("Erreur interne")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for JarvisError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match &self {
            JarvisError::Unauthorized          => (StatusCode::UNAUTHORIZED,           "UNAUTHORIZED",       self.to_string()),
            JarvisError::Forbidden             => (StatusCode::FORBIDDEN,              "FORBIDDEN",          self.to_string()),
            JarvisError::NotFound(m)           => (StatusCode::NOT_FOUND,              "NOT_FOUND",          m.clone()),
            JarvisError::Validation(m)         => (StatusCode::UNPROCESSABLE_ENTITY,   "VALIDATION_ERROR",   m.clone()),
            JarvisError::OllamaUnavailable(m)  => (StatusCode::SERVICE_UNAVAILABLE,    "OLLAMA_UNAVAILABLE", m.clone()),
            JarvisError::Database(e)           => {
                tracing::error!(error = %e, "Erreur DB jarvis");
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "Erreur base de données".into())
            }
            JarvisError::Internal(e)           => {
                tracing::error!(error = %e, "Erreur interne jarvis");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Erreur interne".into())
            }
        };
        (status, Json(json!({ "error": code, "message": msg }))).into_response()
    }
}

pub type JarvisResult<T> = Result<T, JarvisError>;

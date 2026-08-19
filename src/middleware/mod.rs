use axum::{extract::FromRequestParts, http::{request::Parts, StatusCode}};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AssistantUser {
    pub id:    Uuid,
    pub role:  String,
    pub email: String,
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AssistantUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get("x-kubuno-user-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let role = parts
            .headers
            .get("x-kubuno-user-role")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("user")
            .to_string();

        let email = parts
            .headers
            .get("x-kubuno-user-email")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        Ok(AssistantUser { id, role, email })
    }
}

/// Same as [`AssistantUser`] but rejects any caller whose role is not `admin`.
///
/// Instance-wide provider configuration (API keys, endpoints, default models)
/// is administrator-only: it must never be readable or writable by an ordinary
/// authenticated user. Hiding the tab in the frontend is not enough — the route
/// itself has to refuse, otherwise a direct API call bypasses the UI guard.
#[derive(Debug, Clone)]
pub struct AssistantAdmin(pub AssistantUser);

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AssistantAdmin {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AssistantUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(AssistantAdmin(user))
    }
}

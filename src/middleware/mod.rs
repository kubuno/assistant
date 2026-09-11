use axum::{extract::FromRequestParts, http::{request::Parts, StatusCode}};
use uuid::Uuid;

use crate::state::AppState;

/// This module's id, used as the token audience.
const MODULE_ID: &str = "assistant";

#[derive(Debug, Clone)]
pub struct AssistantUser {
    pub id:    Uuid,
    pub role:  String,
    pub email: String,
}

/// Authenticate from the signed `X-Kubuno-Auth` token the core mints with this
/// module's internal secret (see `kubuno-modauth`) instead of trusting the plain
/// `X-Kubuno-User-*` headers, which any process reaching this module's loopback
/// port could forge to impersonate any user. Specialised to `AppState` because
/// verification needs the module's internal secret.
#[axum::async_trait]
impl FromRequestParts<AppState> for AssistantUser {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(kubuno_modauth::TOKEN_HEADER)
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let user = kubuno_modauth::verify(
            state.settings.core.internal_secret.as_bytes(),
            token,
            MODULE_ID,
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

        Ok(AssistantUser {
            id: user.id,
            role: user.role,
            email: user.email,
        })
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
impl FromRequestParts<AppState> for AssistantAdmin {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AssistantUser::from_request_parts(parts, state).await?;
        if user.role != "admin" {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(AssistantAdmin(user))
    }
}

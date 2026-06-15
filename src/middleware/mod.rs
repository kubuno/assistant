use axum::{extract::FromRequestParts, http::{request::Parts, StatusCode}};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JarvisUser {
    pub id:    Uuid,
    pub role:  String,
    pub email: String,
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for JarvisUser {
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

        Ok(JarvisUser { id, role, email })
    }
}

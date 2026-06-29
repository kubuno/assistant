//! Tool execution endpoint — runs a single MCP tool via the core gateway on the
//! user's behalf. Used by the UI to execute a `confirm`-gated tool *after* the
//! user has explicitly approved it (the agentic loop deliberately skips those).

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::{errors::JarvisResult, middleware::JarvisUser, services::McpClient, state::AppState};

#[derive(Deserialize)]
pub struct CallToolDto {
    pub tool:      String,
    pub arguments: serde_json::Value,
}

/// POST /jarvis/tools/call — execute `{ tool, arguments }` via the core MCP
/// gateway with the caller's identity. Returns `{ result, is_error }`.
pub async fn call_tool(
    State(st): State<AppState>,
    user: JarvisUser,
    Json(dto): Json<CallToolDto>,
) -> JarvisResult<Json<serde_json::Value>> {
    let mcp = McpClient::new(&st.settings.core.url, &st.settings.core.internal_secret);
    let (text, is_error) = mcp.call_tool(user.id, &dto.tool, &dto.arguments).await?;
    Ok(Json(serde_json::json!({ "result": text, "is_error": is_error })))
}

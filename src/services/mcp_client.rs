//! Minimal MCP client — talks to the core's internal MCP gateway
//! (`POST {core}/internal/mcp`) on behalf of a user, so the assistant can
//! discover and invoke the tools every module declares. Identity is carried by
//! the `x-kubuno-user-id` header; trust by `x-internal-secret` (jarvis is an
//! internal, trusted module).

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

/// A tool as advertised by the gateway (`tools/list`).
#[derive(Debug, Clone)]
pub struct ToolCatalogItem {
    pub name:         String,
    pub description:  String,
    pub input_schema: Value,
    pub annotations:  Option<Value>,
}

impl ToolCatalogItem {
    /// `annotations.kubuno_ui` → this tool is a client-side UI action, not a
    /// server-executed tool. Returns the `{service, method}` descriptor if so.
    pub fn ui_action(&self) -> Option<Value> {
        self.annotations.as_ref().and_then(|a| a.get("kubuno_ui").cloned())
    }
    /// `annotations.confirm` → the assistant should confirm before calling.
    pub fn needs_confirm(&self) -> bool {
        self.annotations.as_ref()
            .and_then(|a| a.get("confirm"))
            .and_then(|c| c.as_bool())
            .unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct McpClient {
    http:     Client,
    core_url: String,
    secret:   String,
}

impl McpClient {
    pub fn new(core_url: &str, internal_secret: &str) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            http,
            core_url: core_url.trim_end_matches('/').to_string(),
            secret:   internal_secret.to_string(),
        }
    }

    async fn rpc(&self, user_id: Uuid, body: Value) -> Result<Value> {
        let resp = self.http
            .post(format!("{}/internal/mcp", self.core_url))
            .header("x-internal-secret", &self.secret)
            .header("x-kubuno-user-id", user_id.to_string())
            .json(&body)
            .send().await
            .context("connexion passerelle MCP du core")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            anyhow::bail!("passerelle MCP {status}: {text}");
        }
        resp.json::<Value>().await.context("réponse MCP invalide")
    }

    /// Discover all tools available to this user.
    pub async fn list_tools(&self, user_id: Uuid) -> Result<Vec<ToolCatalogItem>> {
        let resp = self.rpc(user_id, json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list"
        })).await?;
        let tools = resp.pointer("/result/tools").and_then(|t| t.as_array()).cloned()
            .unwrap_or_default();
        Ok(tools.into_iter().filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            Some(ToolCatalogItem {
                name,
                description:  t.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                input_schema: t.get("inputSchema").cloned().unwrap_or_else(|| json!({ "type": "object" })),
                annotations:  t.get("annotations").cloned(),
            })
        }).collect())
    }

    /// Execute a tool. Returns `(result_text, is_error)`.
    pub async fn call_tool(&self, user_id: Uuid, name: &str, arguments: &Value) -> Result<(String, bool)> {
        let resp = self.rpc(user_id, json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })).await?;
        let result = resp.pointer("/result").cloned().unwrap_or(Value::Null);
        let text = result.pointer("/content/0/text").and_then(|t| t.as_str()).unwrap_or("").to_string();
        let is_error = result.get("isError").and_then(|e| e.as_bool()).unwrap_or(false);
        Ok((text, is_error))
    }
}

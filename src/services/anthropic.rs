use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;

use axum::async_trait;

use super::agentic::{AgenticProvider, AgenticTurn, ToolOutcome, ToolUse};
use super::mcp_client::ToolCatalogItem;
use super::provider::{LlmMessage, StreamChunk};

const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone)]
pub struct AnthropicService {
    client:        Client,
    api_key:       String,
    base_url:      String,
    default_model: String,
}

#[derive(Deserialize)]
struct ContentBlockDelta { text: Option<String> }
#[derive(Deserialize)]
struct MessageDeltaUsage { output_tokens: Option<i32> }
#[derive(Deserialize)]
struct InputUsage { input_tokens: Option<i32> }

impl AnthropicService {
    pub fn new(base_url: &str, api_key: &str, default_model: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("création client Anthropic")?;
        Ok(Self {
            client,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        })
    }

    pub fn default_model(&self) -> &str { &self.default_model }

    pub fn list_models(&self) -> Vec<String> {
        // Anthropic does not have a public list endpoint — return known models
        vec![
            "claude-opus-4-5".to_string(),
            "claude-sonnet-4-5".to_string(),
            "claude-haiku-4-5".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
        ]
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
    ) -> Result<mpsc::UnboundedReceiver<StreamChunk>> {
        // Separate system message (Anthropic uses a top-level system field)
        let (system, turns): (Vec<_>, Vec<_>) = messages.into_iter()
            .partition(|m| m.role == "system");
        let system_text = system.into_iter()
            .map(|m| m.content)
            .collect::<Vec<_>>()
            .join("\n");

        let mut body = json!({
            "model":      model,
            "max_tokens": 4096,
            "messages":   turns,
            "stream":     true,
        });
        if !system_text.is_empty() {
            body["system"] = json!(system_text);
        }

        let resp = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send().await
            .context("connexion Anthropic")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body   = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic {status}: {body}");
        }

        let (tx, rx) = mpsc::unbounded_channel::<StreamChunk>();
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut buf   = String::new();
            let mut event = String::new();
            let mut prompt_tokens     = 0i32;
            let mut completion_tokens = 0i32;

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk { Ok(b) => b, Err(e) => { tracing::warn!(error = %e); break; } };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line: String = buf.drain(..=pos).collect();
                    let line = line.trim_end_matches('\n').trim_end_matches('\r');

                    if let Some(ev) = line.strip_prefix("event: ") {
                        event = ev.to_string();
                    } else if let Some(data) = line.strip_prefix("data: ") {
                        match event.as_str() {
                            "content_block_delta" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                    let delta_obj = v.get("delta");
                                    let text = delta_obj
                                        .and_then(|d| serde_json::from_value::<ContentBlockDelta>(d.clone()).ok())
                                        .and_then(|d| d.text);
                                    if let Some(t) = text {
                                        if !t.is_empty() {
                                            let _ = tx.send(StreamChunk {
                                                delta: Some(t),
                                                done: false,
                                                prompt_tokens: 0,
                                                completion_tokens: 0,
                                            });
                                        }
                                    }
                                }
                            }
                            "message_start" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(usage) = v.pointer("/message/usage") {
                                        if let Ok(u) = serde_json::from_value::<InputUsage>(usage.clone()) {
                                            prompt_tokens = u.input_tokens.unwrap_or(0);
                                        }
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(usage) = v.get("usage") {
                                        if let Ok(u) = serde_json::from_value::<MessageDeltaUsage>(usage.clone()) {
                                            completion_tokens = u.output_tokens.unwrap_or(0);
                                        }
                                    }
                                }
                            }
                            "message_stop" => {
                                let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
                                return;
                            }
                            _ => {}
                        }
                    }
                }
            }
            let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
        });

        Ok(rx)
    }
}

#[async_trait]
impl AgenticProvider for AnthropicService {
    fn tool_schema(&self, t: &ToolCatalogItem) -> serde_json::Value {
        json!({
            "name":         t.name,
            "description":  t.description,
            "input_schema": t.input_schema,
        })
    }

    async fn complete_once(
        &self,
        model: &str,
        system: &str,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
    ) -> Result<AgenticTurn> {
        let mut body = json!({
            "model":      model,
            "max_tokens": 4096,
            "messages":   messages,
        });
        if !system.is_empty() { body["system"] = json!(system); }
        if !tools.is_empty()  { body["tools"]  = json!(tools); }

        let resp = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send().await
            .context("connexion Anthropic")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic {status}: {text}");
        }

        let v: serde_json::Value = resp.json().await.context("réponse Anthropic invalide")?;
        let content = v.get("content").cloned().unwrap_or_else(|| json!([]));
        let mut text = String::new();
        let mut tool_uses = Vec::new();
        if let Some(arr) = content.as_array() {
            for block in arr {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(|x| x.as_str()) { text.push_str(t); }
                    }
                    Some("tool_use") => {
                        tool_uses.push(ToolUse {
                            id:    block.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            name:  block.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            input: block.get("input").cloned().unwrap_or_else(|| json!({})),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(AgenticTurn {
            text,
            tool_uses,
            assistant_msg: json!({ "role": "assistant", "content": content }),
            input_tokens:  v.pointer("/usage/input_tokens").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            output_tokens: v.pointer("/usage/output_tokens").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
        })
    }

    fn tool_results_turn(&self, outcomes: &[ToolOutcome<'_>]) -> Vec<serde_json::Value> {
        // Anthropic: all tool_result blocks go in a single user turn.
        let blocks: Vec<serde_json::Value> = outcomes.iter().map(|o| json!({
            "type": "tool_result",
            "tool_use_id": o.tu.id,
            "content": o.content,
            "is_error": o.is_error,
        })).collect();
        vec![json!({ "role": "user", "content": blocks })]
    }
}

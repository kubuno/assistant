use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;

use super::provider::{LlmMessage, StreamChunk};

#[derive(Debug, Clone)]
pub struct GoogleService {
    client:        Client,
    api_key:       String,
    base_url:      String,
    default_model: String,
}

impl GoogleService {
    pub fn new(base_url: &str, api_key: &str, default_model: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("création client Google AI")?;
        Ok(Self {
            client,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        })
    }

    pub fn default_model(&self) -> &str { &self.default_model }

    pub fn list_models(&self) -> Vec<String> {
        vec![
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-flash-lite".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-1.5-pro".to_string(),
        ]
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
    ) -> Result<mpsc::UnboundedReceiver<StreamChunk>> {
        // Separate system instructions
        let (system_parts, user_parts): (Vec<_>, Vec<_>) = messages.into_iter()
            .partition(|m| m.role == "system");
        let system_text = system_parts.into_iter().map(|m| m.content).collect::<Vec<_>>().join("\n");

        let contents: Vec<_> = user_parts.into_iter().map(|m| {
            json!({
                "role": if m.role == "assistant" { "model" } else { "user" },
                "parts": [{ "text": m.content }],
            })
        }).collect();

        let mut body = json!({ "contents": contents });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
        }

        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, model, self.api_key
        );

        let resp = self.client.post(&url).json(&body).send().await
            .context("connexion Google AI")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body   = resp.text().await.unwrap_or_default();
            anyhow::bail!("Google AI {status}: {body}");
        }

        let (tx, rx) = mpsc::unbounded_channel::<StreamChunk>();
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut buf               = String::new();
            let mut prompt_tokens     = 0i32;
            let mut completion_tokens = 0i32;

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk { Ok(b) => b, Err(e) => { tracing::warn!(error = %e); break; } };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line: String = buf.drain(..=pos).collect();
                    let line = line.trim_end_matches('\n').trim_end_matches('\r');
                    let Some(data) = line.strip_prefix("data: ") else { continue };

                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        // Extract text delta
                        if let Some(text) = v.pointer("/candidates/0/content/parts/0/text")
                            .and_then(|t| t.as_str())
                        {
                            if !text.is_empty() {
                                let _ = tx.send(StreamChunk {
                                    delta: Some(text.to_string()),
                                    done: false,
                                    prompt_tokens: 0,
                                    completion_tokens: 0,
                                });
                            }
                        }
                        // Token counts in usage metadata
                        if let Some(usage) = v.get("usageMetadata") {
                            prompt_tokens     = usage["promptTokenCount"].as_i64().unwrap_or(0) as i32;
                            completion_tokens = usage["candidatesTokenCount"].as_i64().unwrap_or(0) as i32;
                        }
                        // finishReason signals end
                        if v.pointer("/candidates/0/finishReason").and_then(|r| r.as_str()).is_some() {
                            let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
        });

        Ok(rx)
    }
}

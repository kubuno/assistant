use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;

use super::provider::{LlmMessage, StreamChunk};

#[derive(Debug, Clone)]
pub struct OpenAiService {
    client:        Client,
    api_key:       String,
    base_url:      String,
    default_model: String,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
}
#[derive(Deserialize)]
struct SseDelta {
    #[serde(default)]
    content: String,
}
#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
    usage:   Option<SseUsage>,
}
#[derive(Deserialize)]
struct SseUsage {
    prompt_tokens:     i32,
    completion_tokens: i32,
}

#[derive(Deserialize)]
struct ModelObj { id: String }
#[derive(Deserialize)]
struct ModelsResp { data: Vec<ModelObj> }

impl OpenAiService {
    pub fn new(base_url: &str, api_key: &str, default_model: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("création client OpenAI")?;
        Ok(Self {
            client,
            api_key: api_key.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        })
    }

    pub fn default_model(&self) -> &str { &self.default_model }

    pub async fn list_models(&self) -> Vec<String> {
        let resp = self.client
            .get(format!("{}/models", self.base_url))
            .bearer_auth(&self.api_key)
            .send().await;
        match resp {
            Ok(r) if r.status().is_success() => {
                r.json::<ModelsResp>().await
                    .map(|m| m.data.into_iter().map(|o| o.id).collect())
                    .unwrap_or_default()
            }
            _ => vec![self.default_model.clone()],
        }
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
    ) -> Result<mpsc::UnboundedReceiver<StreamChunk>> {
        let body = json!({
            "model":    model,
            "messages": messages,
            "stream":   true,
            "stream_options": { "include_usage": true },
        });

        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await
            .context("connexion OpenAI")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body   = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI {status}: {body}");
        }

        let (tx, rx) = mpsc::unbounded_channel::<StreamChunk>();
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut buf = String::new();
            let mut prompt_tokens = 0i32;
            let mut completion_tokens = 0i32;

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk { Ok(b) => b, Err(e) => { tracing::warn!(error = %e); break; } };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find('\n') {
                    let line: String = buf.drain(..=pos).collect();
                    let line = line.trim();
                    if line == "data: [DONE]" {
                        let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
                        return;
                    }
                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    if data.is_empty() { continue; }
                    if let Ok(chunk) = serde_json::from_str::<SseChunk>(data) {
                        if let Some(u) = chunk.usage {
                            prompt_tokens     = u.prompt_tokens;
                            completion_tokens = u.completion_tokens;
                        }
                        for choice in chunk.choices {
                            if !choice.delta.content.is_empty() {
                                let _ = tx.send(StreamChunk {
                                    delta: Some(choice.delta.content),
                                    done: false,
                                    prompt_tokens: 0,
                                    completion_tokens: 0,
                                });
                            }
                        }
                    }
                }
            }
            let _ = tx.send(StreamChunk { delta: None, done: true, prompt_tokens, completion_tokens });
        });

        Ok(rx)
    }
}

use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

use super::provider::{LlmMessage, StreamChunk as CommonChunk};

#[derive(Debug, Clone)]
pub struct OllamaService {
    client:        Client,
    base_url:      String,
    default_model: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model:    &'a str,
    messages: &'a [OllamaMessage],
    stream:   bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct OllamaMessage {
    pub role:    String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    message:    Option<ChunkMessage>,
    done:       bool,
    #[serde(default)]
    prompt_eval_count:    i32,
    #[serde(default)]
    eval_count:           i32,
}

#[derive(Debug, Deserialize)]
struct ChunkMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

/// Ollama-specific chunk (kept for internal use).
pub struct StreamChunk {
    pub delta:             Option<String>,
    pub done:              bool,
    pub prompt_tokens:     i32,
    pub completion_tokens: i32,
}

impl OllamaService {
    pub fn new(base_url: &str, default_model: &str, timeout_secs: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("création client HTTP Ollama")?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
        })
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub async fn list_models(&self) -> Result<Vec<String>> {
        let resp: ModelsResponse = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .context("liste modèles Ollama")?
            .json()
            .await
            .context("parse liste modèles")?;
        Ok(resp.models.into_iter().map(|m| m.name).collect())
    }

    /// Same as `chat_stream` but accepts the unified `LlmMessage` type.
    pub async fn chat_stream_unified(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
    ) -> Result<mpsc::UnboundedReceiver<CommonChunk>> {
        let ollama_msgs: Vec<OllamaMessage> = messages.into_iter()
            .map(|m| OllamaMessage { role: m.role, content: m.content })
            .collect();
        let mut inner_rx = self.chat_stream(model, ollama_msgs).await?;
        let (tx, rx) = mpsc::unbounded_channel::<CommonChunk>();
        tokio::spawn(async move {
            while let Some(c) = inner_rx.recv().await {
                let _ = tx.send(CommonChunk {
                    delta:             c.delta,
                    done:              c.done,
                    prompt_tokens:     c.prompt_tokens,
                    completion_tokens: c.completion_tokens,
                });
            }
        });
        Ok(rx)
    }

    pub async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<OllamaMessage>,
    ) -> Result<mpsc::UnboundedReceiver<StreamChunk>> {
        let req = ChatRequest { model, messages: &messages, stream: true };
        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&req)
            .send()
            .await
            .context("connexion Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama {status}: {body}");
        }

        let (tx, rx) = mpsc::unbounded_channel::<StreamChunk>();

        tokio::spawn(async move {
            let mut byte_stream = response.bytes_stream();
            let mut buf = Vec::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(error = %e, "erreur lecture flux Ollama");
                        break;
                    }
                };
                buf.extend_from_slice(&chunk);

                // Process complete newline-delimited JSON objects
                while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line = buf.drain(..=pos).collect::<Vec<_>>();
                    let line = match std::str::from_utf8(&line) {
                        Ok(s) => s.trim().to_string(),
                        Err(_) => continue,
                    };
                    if line.is_empty() { continue; }

                    match serde_json::from_str::<ChatChunk>(&line) {
                        Ok(parsed) => {
                            let delta = parsed.message.map(|m| m.content);
                            let done = parsed.done;
                            let _ = tx.send(StreamChunk {
                                delta,
                                done,
                                prompt_tokens:     parsed.prompt_eval_count,
                                completion_tokens: parsed.eval_count,
                            });
                            if done { break; }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, line = %line, "parse chunk Ollama");
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

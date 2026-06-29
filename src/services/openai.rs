use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;

use axum::async_trait;
use serde_json::Value;

use super::agentic::{AgenticProvider, AgenticTurn, ToolOutcome, ToolUse};
use super::mcp_client::ToolCatalogItem;
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

/// Vrai si un id de modèle OpenAI est utilisable en *chat completions* (et donc
/// dans la boucle agentique). Exclut embeddings / audio (TTS/STT) / image /
/// modération / modèles « completions » hérités. Liste blanche par famille +
/// liste noire par mot-clé (couvre `gpt-4o-audio-preview`, `gpt-4o-transcribe`…).
pub fn is_openai_chat_model(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    const EXCLUDE: &[&str] = &[
        "embedding", "embed", "whisper", "tts", "audio", "transcribe", "realtime",
        "moderation", "dall-e", "image", "-instruct", "search", "similarity", "edit",
    ];
    if EXCLUDE.iter().any(|k| id.contains(k)) { return false; }
    // Familles de chat connues.
    id.starts_with("gpt-") || id.starts_with("chatgpt")
        || id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4")
}

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
                    // L'API OpenAI renvoie TOUS les modèles (chat, embeddings, audio,
                    // image, modération…). On ne garde que ceux utilisables en chat,
                    // sinon le sélecteur expose des modèles inutilisables.
                    .map(|m| m.data.into_iter().map(|o| o.id).filter(|id| is_openai_chat_model(id)).collect())
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

#[async_trait]
impl AgenticProvider for OpenAiService {
    fn tool_schema(&self, t: &ToolCatalogItem) -> Value {
        json!({
            "type": "function",
            "function": {
                "name":        t.name,
                "description": t.description,
                "parameters":  t.input_schema,
            }
        })
    }

    async fn complete_once(
        &self,
        model: &str,
        system: &str,
        messages: &[Value],
        tools: &[Value],
    ) -> Result<AgenticTurn> {
        // OpenAI carries the system prompt as a leading message.
        let mut msgs: Vec<Value> = Vec::with_capacity(messages.len() + 1);
        if !system.is_empty() {
            msgs.push(json!({ "role": "system", "content": system }));
        }
        msgs.extend_from_slice(messages);

        let mut body = json!({
            "model":    model,
            "messages": msgs,
            "stream":   false,
        });
        if !tools.is_empty() { body["tools"] = json!(tools); }

        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send().await
            .context("connexion OpenAI")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text   = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI {status}: {text}");
        }

        let v: Value = resp.json().await.context("réponse OpenAI invalide")?;
        let message = v.pointer("/choices/0/message").cloned().unwrap_or_else(|| json!({}));
        let text = message.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

        let mut tool_uses = Vec::new();
        if let Some(calls) = message.get("tool_calls").and_then(|c| c.as_array()) {
            for call in calls {
                let id   = call.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let func = call.get("function").cloned().unwrap_or_else(|| json!({}));
                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if name.is_empty() { continue; }
                // OpenAI returns arguments as a JSON string.
                let input = match func.get("arguments") {
                    Some(Value::String(s)) => serde_json::from_str::<Value>(s).unwrap_or_else(|_| json!({})),
                    Some(other)            => other.clone(),
                    None                   => json!({}),
                };
                tool_uses.push(ToolUse { id, name, input });
            }
        }

        Ok(AgenticTurn {
            text,
            tool_uses,
            assistant_msg: message,
            input_tokens:  v.pointer("/usage/prompt_tokens").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            output_tokens: v.pointer("/usage/completion_tokens").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
        })
    }

    fn tool_results_turn(&self, outcomes: &[ToolOutcome<'_>]) -> Vec<Value> {
        // OpenAI: one message per result, keyed by tool_call_id.
        outcomes.iter().map(|o| json!({
            "role": "tool",
            "tool_call_id": o.tu.id,
            "content": o.content,
        })).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::is_openai_chat_model;

    #[test]
    fn keeps_chat_models() {
        for id in ["gpt-4o", "gpt-4o-mini", "gpt-4.1", "gpt-3.5-turbo", "gpt-3.5-turbo-16k",
                   "chatgpt-4o-latest", "o1", "o1-mini", "o3-mini", "o4-mini"] {
            assert!(is_openai_chat_model(id), "devrait garder {id}");
        }
    }

    #[test]
    fn drops_non_chat_models() {
        for id in ["text-embedding-ada-002", "text-embedding-3-large", "whisper-1", "tts-1",
                   "tts-1-hd", "dall-e-3", "omni-moderation-latest", "text-moderation-stable",
                   "gpt-4o-audio-preview", "gpt-4o-realtime-preview", "gpt-4o-transcribe",
                   "gpt-4o-mini-tts", "gpt-3.5-turbo-instruct", "babbage-002", "davinci-002"] {
            assert!(!is_openai_chat_model(id), "devrait retirer {id}");
        }
    }
}

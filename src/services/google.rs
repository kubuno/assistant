use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use url::Url;

use super::endpoint::{endpoint, redact};
use super::provider::{LlmMessage, StreamChunk};
use super::provider::DEFAULT_MAX_OUTPUT_TOKENS;

/// Header the API accepts for its credential.
///
/// This provider also reads the key from a `key=` query parameter, which is what
/// this client used to do — and a query string ends up in proxy logs, in browser
/// history when copied, and in any error text that quotes the URL back. The
/// header carries the same value where nothing echoes it.
const API_KEY_HEADER: &str = "x-goog-api-key";

#[derive(Debug, Clone)]
pub struct GoogleService {
    client:        Client,
    api_key:       String,
    base_url:      String,
    default_model: String,
    /// Instance-wide answer-length ceiling, applied to every request.
    max_output_tokens: u32,
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
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
        })
    }

    /// Applies the instance-wide answer-length ceiling.
    pub fn with_max_output_tokens(mut self, cap: u32) -> Self {
        self.max_output_tokens = cap.max(1);
        self
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

    /// Streaming endpoint for `model`.
    ///
    /// `model` is caller-supplied — it comes from the request body, and an
    /// instance with an empty model allowlist (the default) accepts any string —
    /// so it is percent-encoded as a single path segment instead of being
    /// interpolated raw. The credential is NOT in this URL: it goes in a header,
    /// which is what makes the URL safe to log or to quote in an error.
    fn stream_url(&self, model: &str) -> Result<Url> {
        let resource = format!("{model}:streamGenerateContent");
        let mut url = endpoint(&self.base_url, &["v1beta", "models", &resource])?;
        url.set_query(Some("alt=sse"));
        Ok(url)
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

        let mut body = json!({
            "contents": contents,
            "generationConfig": { "maxOutputTokens": self.max_output_tokens },
        });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
        }

        let url = self.stream_url(model)?;

        let resp = self.client
            .post(url)
            .header(API_KEY_HEADER, &self.api_key)
            .json(&body)
            .send()
            .await
            .context("connexion Google AI")?;

        if !resp.status().is_success() {
            let status = resp.status();
            // The body is the provider's text and travels to the client inside a
            // 503; a provider that echoes the credential it refused must not be
            // allowed to hand it back through us.
            let body   = redact(&resp.text().await.unwrap_or_default(), &self.api_key);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::endpoint::shown;

    const KEY: &str = "AIza-cle-tres-secrete-0123456789";

    fn service() -> GoogleService {
        GoogleService::new("https://fournisseur.example", KEY, "gemini-2.0-flash")
            .expect("service constructible")
    }

    #[test]
    fn the_api_key_is_absent_from_the_request_url() {
        let url = service().stream_url("gemini-2.0-flash").expect("URL");
        let text = url.as_str();
        assert!(!text.contains(KEY), "clé présente dans « {text} »");
        assert!(!text.contains("key="));
        assert_eq!(url.query(), Some("alt=sse"));
        assert_eq!(
            url.path(),
            "/v1beta/models/gemini-2.0-flash:streamGenerateContent"
        );
    }

    /// The URL as an error or a log line would carry it: still no credential,
    /// whatever the caller asked for as a model.
    #[test]
    fn a_url_put_into_an_error_carries_no_credential() {
        let svc = service();
        for model in [
            "gemini-2.0-flash",
            "../../autre-endpoint",
            "modele?key=vole",
            "modele#tronque",
            "a/b/c",
        ] {
            let url = svc.stream_url(model).expect("URL");
            let text = shown(&url);
            assert!(!text.contains(KEY), "clé présente dans « {text} »");
            assert!(
                text.starts_with("https://fournisseur.example/v1beta/models/"),
                "requête détournée vers « {text} »"
            );
        }
    }

    #[test]
    fn a_model_name_cannot_reach_another_endpoint() {
        let url = service().stream_url("../../../v1/tokens").expect("URL");
        let depth = url.path().split('/').filter(|s| !s.is_empty()).count();
        assert_eq!(depth, 3, "chemin inattendu : {}", url.path());
    }
}

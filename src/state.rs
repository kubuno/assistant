use crate::config::Settings;
use crate::services::{AnthropicService, GoogleService, OllamaService, OpenAiService};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db:        PgPool,
    pub settings:  Arc<Settings>,
    pub ollama:    Arc<OllamaService>,
    pub openai:    Option<Arc<OpenAiService>>,
    pub anthropic: Option<Arc<AnthropicService>>,
    pub google:    Option<Arc<GoogleService>>,
}

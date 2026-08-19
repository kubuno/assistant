/// Answer-length ceiling used when no instance policy has been read yet.
///
/// This is the value the Anthropic client used to hard-code; keeping it as the
/// compiled floor means a module that cannot reach the core behaves exactly as
/// it did before the setting existed.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;

/// Common streaming chunk emitted by all LLM providers.
pub struct StreamChunk {
    pub delta:             Option<String>,
    pub done:              bool,
    pub prompt_tokens:     i32,
    pub completion_tokens: i32,
}

/// Unified message format passed to providers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LlmMessage {
    pub role:    String,
    pub content: String,
}

/// A model as returned by the /models endpoint.
#[derive(Debug, serde::Serialize)]
pub struct ProviderModel {
    pub id:         String,
    pub name:       String,
    pub provider:   String,
    pub is_default: bool,
}

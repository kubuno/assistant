//! Instance-wide settings of the assistant module, as the administrator left
//! them in the console.
//!
//! Declared by `module.toml`'s `[[settings]]`, stored in `core.settings`, and read
//! back here through `/internal/modules/assistant/settings` — a module owns its
//! own schema and cannot read the core's tables, and a background worker has no
//! user token for the public config route.
//!
//! Every field here is read by code that acts on it: a knob that changes nothing
//! is worse than an absent one. Provider credentials are deliberately NOT here —
//! a secret has no place in the core's settings store, which answers in clear to
//! anything holding the internal secret. They stay in the module's own
//! `assistant.provider_config` table, behind the guarded admin routes.

use serde_json::Value;

/// `Clone` rather than `Copy`: the model allowlist is a list. Callers take a
/// snapshot once per request, so the clone is paid at most once per message.
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    /// Whether conversations may run on a provider other than the local engine.
    /// `false` pins the instance to what it hosts itself: remote providers stop
    /// being offered, and a conversation already bound to one is refused.
    pub allow_cloud_providers: bool,
    /// Models a conversation may use. Empty = every model the provider offers.
    /// Compared case-insensitively against the model identifier.
    pub allowed_models: Vec<String>,
    /// Whether users may create and edit agents of their own. `false` leaves the
    /// agents shipped with the module (which are already read-only) in place.
    pub allow_custom_agents: bool,
    /// Whether the assistant may call tools at all. `false` short-circuits the
    /// tool catalogue and refuses tool execution.
    pub enable_tools: bool,
    /// How many tool round-trips a single answer may take before the loop stops.
    pub max_tool_rounds: usize,
    /// Ceiling on the length, in characters, of a message a user may send.
    pub max_message_chars: usize,
    /// How many past messages of the conversation are replayed to the model.
    /// `0` = the whole conversation, which is what the module did before.
    pub history_window_messages: i64,
    /// Ceiling on the answer length, in tokens, asked of the provider.
    pub max_output_tokens: u32,
    /// How long a conversation is kept before the purge deletes it, in days.
    /// `0` = kept forever.
    pub conversation_retention_days: i64,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            allow_cloud_providers:      true,
            allowed_models:             Vec::new(),
            allow_custom_agents:        true,
            enable_tools:               true,
            max_tool_rounds:            6,
            max_message_chars:          32_000,
            history_window_messages:    0,
            max_output_tokens:          4096,
            conversation_retention_days: 0,
        }
    }
}

impl InstanceConfig {
    /// Maps the core's `{key: value}` object onto the struct. Every read falls
    /// back to the compiled default rather than to a permissive value; an
    /// out-of-range number is treated as a mistake and ignored the same way.
    /// `0` is MEANINGFUL for `history_window_messages` (replay everything) and
    /// `conversation_retention_days` (never purge), so it is accepted there.
    pub fn from_settings(settings: &Value) -> Self {
        let d = Self::default();
        let int_in = |key: &str, min: i64, max: i64, fallback: i64| -> i64 {
            settings
                .get(key)
                .and_then(Value::as_i64)
                .filter(|n| (min..=max).contains(n))
                .unwrap_or(fallback)
        };
        let bool_of = |key: &str, fallback: bool| {
            settings.get(key).and_then(Value::as_bool).unwrap_or(fallback)
        };
        Self {
            allow_cloud_providers: bool_of("allow_cloud_providers", d.allow_cloud_providers),
            allowed_models:        parse_list(settings, "allowed_models"),
            allow_custom_agents:   bool_of("allow_custom_agents", d.allow_custom_agents),
            enable_tools:          bool_of("enable_tools", d.enable_tools),
            max_tool_rounds:       int_in("max_tool_rounds", 1, 30, d.max_tool_rounds as i64) as usize,
            max_message_chars:     int_in("max_message_chars", 100, 2_000_000, d.max_message_chars as i64) as usize,
            history_window_messages: int_in("history_window_messages", 0, 10_000, d.history_window_messages),
            max_output_tokens:     int_in("max_output_tokens", 128, 200_000, d.max_output_tokens as i64) as u32,
            conversation_retention_days: int_in(
                "conversation_retention_days", 0, 3650, d.conversation_retention_days,
            ),
        }
    }

    /// Whether `model` is allowed. An empty allowlist allows everything, so an
    /// administrator who never touched the setting changes nothing.
    pub fn model_allowed(&self, model: &str) -> bool {
        if self.allowed_models.is_empty() {
            return true;
        }
        let m = model.trim().to_ascii_lowercase();
        self.allowed_models.iter().any(|a| *a == m)
    }
}

/// One entry per line, blank lines and `#` comments ignored, lower-cased.
fn parse_list(settings: &Value, key: &str) -> Vec<String> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| l.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads the instance settings from the core. Any failure yields `None`, so the
/// caller keeps the values it already had rather than reverting to defaults
/// because the core was briefly unreachable.
pub async fn fetch(
    http: &reqwest::Client,
    core_url: &str,
    secret: &str,
) -> Option<InstanceConfig> {
    let url = format!("{core_url}/internal/modules/assistant/settings");
    let resp = http
        .get(&url)
        .header("X-Internal-Secret", secret)
        .send()
        .await
        .map_err(|e| tracing::warn!(error = %e, "Lecture des réglages d'instance assistant"))
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "Réglages d'instance assistant refusés par le core");
        return None;
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| tracing::warn!(error = %e, "Réglages d'instance assistant : réponse illisible"))
        .ok()?;

    Some(InstanceConfig::from_settings(body.get("settings")?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_keys_keep_the_compiled_defaults() {
        let c = InstanceConfig::from_settings(&json!({}));
        assert!(c.allow_cloud_providers);
        assert!(c.allowed_models.is_empty());
        assert!(c.allow_custom_agents);
        assert!(c.enable_tools);
        assert_eq!(c.max_tool_rounds, 6);
        assert_eq!(c.max_message_chars, 32_000);
        assert_eq!(c.history_window_messages, 0);
        assert_eq!(c.max_output_tokens, 4096);
        assert_eq!(c.conversation_retention_days, 0);
    }

    #[test]
    fn an_empty_allowlist_allows_every_model() {
        let c = InstanceConfig::from_settings(&json!({ "allowed_models": "  \n # rien\n" }));
        assert!(c.allowed_models.is_empty());
        assert!(c.model_allowed("n-importe-quoi"));
    }

    #[test]
    fn the_allowlist_is_case_insensitive_and_ignores_comments() {
        let c = InstanceConfig::from_settings(&json!({
            "allowed_models": "# modèles validés\nLlama3.1:8b\n\nmistral-small\n",
        }));
        assert_eq!(c.allowed_models.len(), 2);
        assert!(c.model_allowed("llama3.1:8b"));
        assert!(c.model_allowed(" LLAMA3.1:8B "));
        assert!(!c.model_allowed("autre-modele"));
    }

    #[test]
    fn zero_is_meaningful_for_the_window_and_the_retention() {
        let c = InstanceConfig::from_settings(&json!({
            "history_window_messages": 0, "conversation_retention_days": 0,
        }));
        assert_eq!(c.history_window_messages, 0);
        assert_eq!(c.conversation_retention_days, 0);
    }

    #[test]
    fn out_of_range_falls_back() {
        let c = InstanceConfig::from_settings(&json!({
            "max_tool_rounds": 0, "max_output_tokens": 5, "max_message_chars": 1,
        }));
        assert_eq!(c.max_tool_rounds, 6);
        assert_eq!(c.max_output_tokens, 4096);
        assert_eq!(c.max_message_chars, 32_000);
    }

    #[test]
    fn the_instance_can_be_pinned_to_what_it_hosts() {
        let c = InstanceConfig::from_settings(&json!({ "allow_cloud_providers": false }));
        assert!(!c.allow_cloud_providers);
    }
}

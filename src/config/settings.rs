use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server:    ServerSettings,
    pub core:      CoreSettings,
    pub database:  DatabaseSettings,
    pub ollama:    OllamaSettings,
    pub providers: ProvidersSettings,
    pub logging:   LoggingSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreSettings {
    pub url:             String,
    pub internal_secret: String,
}

/// The `[database]` section is owned by kubuno-db: which of its fields matter
/// depends on the engine the administrator picks (`database.engine`), and the
/// pool is opened by `kubuno_db::connect`.
pub use kubuno_db::DbSettings as DatabaseSettings;

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaSettings {
    pub enabled:       bool,
    pub url:           String,
    pub default_model: String,
    pub timeout_secs:  u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProvidersSettings {
    #[serde(default)]
    pub openai:    OpenAiSettings,
    #[serde(default)]
    pub anthropic: AnthropicSettings,
    #[serde(default)]
    pub google:    GoogleSettings,
}

fn openai_base_url() -> String { "https://api.openai.com/v1".into() }
fn openai_default_model() -> String { "gpt-4o-mini".into() }

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OpenAiSettings {
    #[serde(default)]
    pub enabled:       bool,
    #[serde(default)]
    pub api_key:       String,
    #[serde(default = "openai_base_url")]
    pub base_url:      String,
    #[serde(default = "openai_default_model")]
    pub default_model: String,
}

fn anthropic_base_url() -> String { "https://api.anthropic.com".into() }
fn anthropic_default_model() -> String { "claude-3-5-haiku-20241022".into() }
fn google_base_url() -> String { "https://generativelanguage.googleapis.com".into() }
fn google_default_model() -> String { "gemini-2.0-flash".into() }

#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicSettings {
    #[serde(default)]
    pub enabled:       bool,
    #[serde(default)]
    pub api_key:       String,
    #[serde(default = "anthropic_base_url")]
    pub base_url:      String,
    #[serde(default = "anthropic_default_model")]
    pub default_model: String,
}

impl Default for AnthropicSettings {
    fn default() -> Self {
        Self {
            enabled:       false,
            api_key:       String::new(),
            base_url:      anthropic_base_url(),
            default_model: anthropic_default_model(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleSettings {
    #[serde(default)]
    pub enabled:       bool,
    #[serde(default)]
    pub api_key:       String,
    #[serde(default = "google_base_url")]
    pub base_url:      String,
    #[serde(default = "google_default_model")]
    pub default_model: String,
}

impl Default for GoogleSettings {
    fn default() -> Self {
        Self {
            enabled:       false,
            api_key:       String::new(),
            base_url:      google_base_url(),
            default_model: google_default_model(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingSettings {
    pub level:  String,
    pub format: LogFormat,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3107)?
            .set_default("core.url", "http://127.0.0.1:8080")?
            .set_default("core.internal_secret", "")?
            .set_default("database.max_connections", 10u64)?
            .set_default("database.min_connections", 1u64)?
            .set_default("database.connect_timeout", 10u64)?
            .set_default("database.run_migrations", true)?
            .set_default("database.engine", "postgres")?
            // SQLite only: where `<schema>.sqlite` lives.
            .set_default("database.path", "./data/db")?
            .set_default("ollama.enabled", true)?
            .set_default("ollama.url", "http://localhost:11434")?
            .set_default("ollama.default_model", "llama3.2:3b")?
            .set_default("ollama.timeout_secs", 120u64)?
            .set_default("providers.openai.enabled", false)?
            .set_default("providers.openai.api_key", "")?
            .set_default("providers.openai.base_url", "https://api.openai.com/v1")?
            .set_default("providers.openai.default_model", "gpt-4o")?
            .set_default("providers.anthropic.enabled", false)?
            .set_default("providers.anthropic.api_key", "")?
            .set_default("providers.anthropic.default_model", "claude-3-5-sonnet-20241022")?
            .set_default("providers.google.enabled", false)?
            .set_default("providers.google.api_key", "")?
            .set_default("providers.google.default_model", "gemini-1.5-flash")?
            .set_default("logging.level", "info")?
            .set_default("logging.format", "pretty")?
            .add_source(File::with_name("config").required(false))
            .add_source(File::with_name("/etc/kubuno/modules/assistant/config").required(false))
            .add_source(
                Environment::with_prefix("KAS")
                    .separator("__")
                    .try_parsing(true),
            );

        // Variables injectées par le superviseur core — priorité maximale
        if let Ok(v) = std::env::var("KUBUNO_CORE_URL")        { builder = builder.set_override("core.url",             v)?; }
        if let Ok(v) = std::env::var("KUBUNO_INTERNAL_SECRET") { builder = builder.set_override("core.internal_secret", v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_HOST")         { builder = builder.set_override("database.host",     v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_PORT")         { builder = builder.set_override("database.port",     v.parse::<i64>().unwrap_or(5432))?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_USER")         { builder = builder.set_override("database.user",     v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_PASSWORD")     { builder = builder.set_override("database.password", v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_NAME")         { builder = builder.set_override("database.database", v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_PATH")         { builder = builder.set_override("database.path",     v)?; }
        if let Ok(v) = std::env::var("KUBUNO_DB_ENGINE")       { builder = builder.set_override("database.engine",   v)?; }

        builder.build()?.try_deserialize()
    }
}

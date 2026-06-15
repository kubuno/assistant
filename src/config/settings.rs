use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::time::Duration;

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

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub url:             Option<String>,
    pub host:            Option<String>,
    pub port:            Option<u16>,
    pub user:            Option<String>,
    pub password:        Option<String>,
    pub database:        Option<String>,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(with = "duration_secs")]
    pub connect_timeout: Duration,
    pub run_migrations:  bool,
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> anyhow::Result<sqlx::postgres::PgConnectOptions> {
        use anyhow::Context;
        use std::str::FromStr;
        if self.host.is_some() || self.user.is_some() {
            let user     = self.user.as_deref().context("database.user requis")?;
            let password = self.password.as_deref().context("database.password requis")?;
            let database = self.database.as_deref().context("database.database requis")?;
            return Ok(sqlx::postgres::PgConnectOptions::new()
                .host(self.host.as_deref().unwrap_or("localhost"))
                .port(self.port.unwrap_or(5432))
                .username(user)
                .password(password)
                .database(database));
        }
        if let Some(url) = &self.url {
            return sqlx::postgres::PgConnectOptions::from_str(url)
                .context("database.url invalide");
        }
        Err(anyhow::anyhow!("database : fournissez url ou host/user/password/database"))
    }
}

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
            .add_source(File::with_name("/etc/kubuno/modules/jarvis/config").required(false))
            .add_source(
                Environment::with_prefix("KJ")
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

        builder.build()?.try_deserialize()
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer};
    use std::time::Duration;
    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de> {
        let secs = u64::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

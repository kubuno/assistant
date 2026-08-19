//! Live set of the LLM services the module talks to.
//!
//! Until this existed the four services were built ONCE at boot from the on-disk
//! deploy config, while the administration screen wrote to
//! `assistant.provider_config` — a table nothing ever read. An administrator who
//! pasted a key saw it saved and nothing happened. The table is now the source of
//! truth: this module rebuilds the services from it, and the set is swapped in
//! place so an edit takes effect on the next message rather than the next
//! restart.
//!
//! Two other rules are enforced here, at the single point every request goes
//! through rather than scattered across the handlers:
//!   * a remote provider the instance policy forbids is simply not built, so
//!     nothing downstream can reach it — not the dispatcher, not the agentic
//!     loop, not the model picker;
//!   * a base address that is not plain http(s) is refused, so a malformed row
//!     cannot turn a credentialed request into something other than an HTTP call.
//!
//! The deploy config stays the fallback: a fresh install with its credentials in
//! `config.toml` and an untouched table keeps working exactly as before.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::{InstanceConfig, Settings};
use crate::services::{AnthropicService, GoogleService, OllamaService, OpenAiService};

/// The providers a request may use, as of the last reload.
#[derive(Clone)]
pub struct ProviderSet {
    /// The locally hosted engine. Always present: it needs no credential and is
    /// what an instance pinned to its own hardware runs on.
    pub ollama:    Arc<OllamaService>,
    pub openai:    Option<Arc<OpenAiService>>,
    pub anthropic: Option<Arc<AnthropicService>>,
    pub google:    Option<Arc<GoogleService>>,
}

/// One row of `assistant.provider_config`.
#[derive(sqlx::FromRow)]
struct Row {
    provider:      String,
    enabled:       bool,
    api_key:       String,
    base_url:      String,
    default_model: String,
}

/// Accepts only a plain http(s) address carrying a host.
///
/// Loopback and private addresses are deliberately ACCEPTED here: pointing the
/// module at an inference server on the same machine is the normal self-hosted
/// setup, and refusing it would break the module's main use. What is refused is
/// a scheme that would make a credentialed request something other than an HTTP
/// call. Only an administrator can write this field.
pub fn validate_base_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim())
        .map_err(|_| "Adresse invalide".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Schéma non autorisé : {other}")),
    }
    if parsed.host_str().is_none() {
        return Err("Adresse sans hôte".into());
    }
    Ok(())
}

/// Picks the stored value, or the deploy config when the row leaves it empty.
fn or_static<'a>(stored: &'a str, fallback: &'a str) -> &'a str {
    let s = stored.trim();
    if s.is_empty() { fallback } else { s }
}

impl ProviderSet {
    /// Builds the set from the database, the deploy config and the instance
    /// policy.
    ///
    /// Never fails: a provider that cannot be built is left absent and the
    /// reason is logged, because the module must keep answering on the engine it
    /// hosts even when a remote credential is wrong. `boot_local` is the engine
    /// built at start-up, kept as the last resort so the set always has one.
    ///
    /// No value read here is ever logged: the rows carry API keys.
    pub async fn load(
        db:         &PgPool,
        settings:   &Settings,
        instance:   &InstanceConfig,
        boot_local: &Arc<OllamaService>,
    ) -> Self {
        let cap = instance.max_output_tokens;
        let rows = sqlx::query_as::<_, Row>(
            "SELECT provider, enabled, api_key, base_url, default_model \
             FROM assistant.provider_config",
        )
        .fetch_all(db)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "Lecture de la configuration des fournisseurs");
            Vec::new()
        });

        let row_of = |name: &str| rows.iter().find(|r| r.provider == name);

        // ── Local engine ────────────────────────────────────────────────────
        let local = row_of("ollama");
        let local_url = local
            .map(|r| or_static(&r.base_url, &settings.ollama.url))
            .unwrap_or(&settings.ollama.url);
        let local_model = local
            .map(|r| or_static(&r.default_model, &settings.ollama.default_model))
            .unwrap_or(&settings.ollama.default_model);

        let ollama = match validate_base_url(local_url) {
            Err(e) => {
                tracing::error!(error = %e, "Moteur local : adresse refusée, configuration de déploiement conservée");
                boot_local.clone()
            }
            Ok(()) => match OllamaService::new(local_url, local_model, settings.ollama.timeout_secs) {
                Ok(svc) => Arc::new(svc.with_max_output_tokens(cap)),
                Err(e) => {
                    tracing::error!(error = %e, "Moteur local : initialisation impossible, configuration de déploiement conservée");
                    boot_local.clone()
                }
            },
        };

        // ── Remote providers ────────────────────────────────────────────────
        // A single decision point: forbidden by the instance = never built, so
        // no handler can reach one by accident.
        if !instance.allow_cloud_providers {
            return Self { ollama, openai: None, anthropic: None, google: None };
        }

        let openai = row_of("openai").and_then(|r| {
            let base  = or_static(&r.base_url,      &settings.providers.openai.base_url);
            let key   = or_static(&r.api_key,       &settings.providers.openai.api_key);
            let model = or_static(&r.default_model, &settings.providers.openai.default_model);
            build(r.enabled, "openai", base, key, || {
                OpenAiService::new(base, key, model).map(|s| s.with_max_output_tokens(cap))
            })
        });

        let anthropic = row_of("anthropic").and_then(|r| {
            let base  = or_static(&r.base_url,      &settings.providers.anthropic.base_url);
            let key   = or_static(&r.api_key,       &settings.providers.anthropic.api_key);
            let model = or_static(&r.default_model, &settings.providers.anthropic.default_model);
            build(r.enabled, "anthropic", base, key, || {
                AnthropicService::new(base, key, model).map(|s| s.with_max_output_tokens(cap))
            })
        });

        let google = row_of("google").and_then(|r| {
            let base  = or_static(&r.base_url,      &settings.providers.google.base_url);
            let key   = or_static(&r.api_key,       &settings.providers.google.api_key);
            let model = or_static(&r.default_model, &settings.providers.google.default_model);
            build(r.enabled, "google", base, key, || {
                GoogleService::new(base, key, model).map(|s| s.with_max_output_tokens(cap))
            })
        });

        Self { ollama, openai, anthropic, google }
    }
}

/// Shared shape of the three credentialed providers: skip when disabled or
/// uncredentialed, refuse a malformed address, log the reason WITHOUT the key.
fn build<S, F>(enabled: bool, name: &str, base: &str, key: &str, make: F) -> Option<Arc<S>>
where
    F: FnOnce() -> anyhow::Result<S>,
{
    if !enabled || key.is_empty() {
        return None;
    }
    if let Err(e) = validate_base_url(base) {
        tracing::error!(provider = name, error = %e, "Fournisseur ignoré : adresse invalide");
        return None;
    }
    match make() {
        Ok(svc) => Some(Arc::new(svc)),
        Err(e) => {
            tracing::error!(provider = name, error = %e, "Fournisseur ignoré : initialisation impossible");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_http_addresses_are_accepted() {
        assert!(validate_base_url("https://api.exemple.com/v1").is_ok());
        // Loopback is legitimate here: a locally hosted inference server.
        assert!(validate_base_url("http://127.0.0.1:11434").is_ok());
        assert!(validate_base_url("file:///etc/passwd").is_err());
        assert!(validate_base_url("ftp://exemple.com").is_err());
        assert!(validate_base_url("pas une adresse").is_err());
        assert!(validate_base_url("").is_err());
    }

    #[test]
    fn an_empty_stored_value_falls_back_to_the_deploy_config() {
        assert_eq!(or_static("   ", "https://defaut"), "https://defaut");
        assert_eq!(or_static("https://choisi", "https://defaut"), "https://choisi");
    }
}

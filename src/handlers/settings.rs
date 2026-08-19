//! Provider administration — the credentials the module uses to reach an LLM.
//!
//! These values are SECRETS, so they do not live in the core's settings store
//! (which answers in clear to anything holding the internal secret) but in the
//! module's own `assistant.provider_config` table, behind an admin guard. The
//! key is never returned in full, never echoed back after a write, and never
//! logged — the API answers with a masked preview and a flag saying whether a
//! key is on file, which is everything a form needs to render.
//!
//! A write takes effect immediately: the live provider set is rebuilt from the
//! table before the response is sent, so an administrator who pastes a key can
//! send a message straight away instead of waiting for a restart.

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{
    errors::{AssistantError, AssistantResult},
    middleware::AssistantAdmin,
    services::registry::validate_base_url,
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct ProviderConfig {
    pub provider:      String,
    pub enabled:       bool,
    /// Masked preview only — never the key itself.
    pub api_key:       String,
    /// Whether a key is on file. Lets the form say "configured" without ever
    /// having to reason about the masked string.
    pub has_api_key:   bool,
    pub base_url:      String,
    pub default_model: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ProviderConfigRow {
    pub provider:      String,
    pub enabled:       bool,
    pub api_key:       String,
    pub base_url:      String,
    pub default_model: String,
}

impl From<ProviderConfigRow> for ProviderConfig {
    fn from(r: ProviderConfigRow) -> Self {
        Self {
            provider:      r.provider,
            enabled:       r.enabled,
            api_key:       mask_key(&r.api_key),
            has_api_key:   !r.api_key.is_empty(),
            base_url:      r.base_url,
            default_model: r.default_model,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderDto {
    pub enabled:       Option<bool>,
    /// Absent = leave the stored key untouched. An empty string CLEARS it, which
    /// is how an administrator revokes a credential from the form.
    pub api_key:       Option<String>,
    pub base_url:      Option<String>,
    pub default_model: Option<String>,
}

/// Shows just enough to recognise which key is on file, never enough to use it.
/// A short value is starred out entirely rather than half-revealed.
fn mask_key(key: &str) -> String {
    if key.is_empty() { return String::new(); }
    if key.len() <= 8 { return "*".repeat(key.len()); }
    format!("{}…{}", &key[..4], &key[key.len() - 4..])
}

pub async fn list_providers(
    State(st): State<AppState>,
    _admin: AssistantAdmin,
) -> AssistantResult<Json<Vec<ProviderConfig>>> {
    let rows = sqlx::query_as::<_, ProviderConfigRow>(
        "SELECT provider, enabled, api_key, base_url, default_model \
         FROM assistant.provider_config ORDER BY provider",
    )
    .fetch_all(&st.db)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Lecture de la configuration des fournisseurs");
        AssistantError::from(e)
    })?;

    Ok(Json(rows.into_iter().map(ProviderConfig::from).collect()))
}

pub async fn update_provider(
    State(st): State<AppState>,
    _admin: AssistantAdmin,
    axum::extract::Path(provider): axum::extract::Path<String>,
    Json(dto): Json<UpdateProviderDto>,
) -> AssistantResult<Json<ProviderConfig>> {
    let valid_providers = ["ollama", "openai", "anthropic", "google"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(AssistantError::NotFound(format!("Provider inconnu: {provider}")));
    }

    // Validate the address BEFORE writing: a row the loader would refuse would
    // otherwise be saved, reported as saved, and silently disable the provider.
    if let Some(url) = dto.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        validate_base_url(url).map_err(AssistantError::Validation)?;
    }

    let row = sqlx::query_as::<_, ProviderConfigRow>(
        r#"UPDATE assistant.provider_config SET
               enabled       = COALESCE($2, enabled),
               api_key       = COALESCE($3, api_key),
               base_url      = COALESCE($4, base_url),
               default_model = COALESCE($5, default_model),
               updated_at    = NOW()
           WHERE provider = $1
           RETURNING provider, enabled, api_key, base_url, default_model"#,
    )
    .bind(&provider)
    .bind(dto.enabled)
    .bind(dto.api_key.as_deref())
    .bind(dto.base_url.as_deref())
    .bind(dto.default_model.as_deref())
    .fetch_one(&st.db)
    .await
    .map_err(|e| {
        // `provider` is a value from the whitelist above, never a secret.
        tracing::error!(error = %e, provider = %provider, "Écriture de la configuration d'un fournisseur");
        AssistantError::from(e)
    })?;

    // The table is the source of truth for the running services: rebuild them
    // now so the change is live rather than pending a restart.
    st.reload_providers().await;

    Ok(Json(ProviderConfig::from(row)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_never_returned_in_full() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("court"), "*****");
        let masked = mask_key("sk-abcdefghijklmnop");
        assert!(!masked.contains("efghijkl"));
        assert!(masked.starts_with("sk-a"));
        assert!(masked.ends_with("mnop"));
    }
}

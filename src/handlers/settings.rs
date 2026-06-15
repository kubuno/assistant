use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{errors::JarvisResult, middleware::JarvisUser, state::AppState};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ProviderConfig {
    pub provider:      String,
    pub enabled:       bool,
    /// API key is never returned in full — masked on read
    #[sqlx(skip)]
    pub api_key:       String,
    pub base_url:      String,
    pub default_model: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ProviderConfigRow {
    pub provider:      String,
    pub enabled:       bool,
    pub api_key:       String,
    pub base_url:      String,
    pub default_model: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderDto {
    pub enabled:       Option<bool>,
    pub api_key:       Option<String>,
    pub base_url:      Option<String>,
    pub default_model: Option<String>,
}

fn mask_key(key: &str) -> String {
    if key.is_empty() { return String::new(); }
    if key.len() <= 8 { return "*".repeat(key.len()); }
    format!("{}…{}", &key[..4], &key[key.len()-4..])
}

pub async fn list_providers(
    State(st): State<AppState>,
    _user: JarvisUser,
) -> JarvisResult<Json<Vec<ProviderConfig>>> {
    let rows = sqlx::query_as::<_, ProviderConfigRow>(
        "SELECT provider, enabled, api_key, base_url, default_model FROM jarvis.provider_config ORDER BY provider"
    )
    .fetch_all(&st.db)
    .await?;

    let configs = rows.into_iter().map(|r| ProviderConfig {
        provider:      r.provider,
        enabled:       r.enabled,
        api_key:       mask_key(&r.api_key),
        base_url:      r.base_url,
        default_model: r.default_model,
    }).collect();

    Ok(Json(configs))
}

pub async fn update_provider(
    State(st): State<AppState>,
    _user: JarvisUser,
    axum::extract::Path(provider): axum::extract::Path<String>,
    Json(dto): Json<UpdateProviderDto>,
) -> JarvisResult<Json<ProviderConfig>> {
    let valid_providers = ["ollama", "openai", "anthropic", "google"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(crate::errors::JarvisError::NotFound(format!("Provider inconnu: {provider}")));
    }

    let row = sqlx::query_as::<_, ProviderConfigRow>(
        r#"UPDATE jarvis.provider_config SET
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
    .await?;

    Ok(Json(ProviderConfig {
        provider:      row.provider,
        enabled:       row.enabled,
        api_key:       mask_key(&row.api_key),
        base_url:      row.base_url,
        default_model: row.default_model,
    }))
}

use axum::{extract::State, Json};
use serde::Serialize;

use crate::{errors::JarvisResult, middleware::JarvisUser, state::AppState};

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id:         String,
    pub name:       String,
    pub provider:   String,
    pub is_default: bool,
}

pub async fn list_models(
    State(st): State<AppState>,
    _user: JarvisUser,
) -> JarvisResult<Json<Vec<ModelInfo>>> {
    let default_model = st.ollama.default_model().to_string();
    let mut infos: Vec<ModelInfo> = Vec::new();

    // Ollama models
    match st.ollama.list_models().await {
        Ok(models) => {
            for name in models {
                let is_default = name == default_model;
                infos.push(ModelInfo { id: name.clone(), name, provider: "ollama".into(), is_default });
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "impossible de lister les modèles Ollama");
            infos.push(ModelInfo {
                id: default_model.clone(), name: default_model,
                provider: "ollama".into(), is_default: true,
            });
        }
    }

    // OpenAI models
    if let Some(svc) = &st.openai {
        for id in svc.list_models().await {
            let is_default = id == svc.default_model();
            infos.push(ModelInfo { id: id.clone(), name: id, provider: "openai".into(), is_default });
        }
    }

    // Anthropic models
    if let Some(svc) = &st.anthropic {
        for id in svc.list_models() {
            let is_default = id == svc.default_model();
            infos.push(ModelInfo { id: id.clone(), name: id, provider: "anthropic".into(), is_default });
        }
    }

    // Google models
    if let Some(svc) = &st.google {
        for id in svc.list_models() {
            let is_default = id == svc.default_model();
            infos.push(ModelInfo { id: id.clone(), name: id, provider: "google".into(), is_default });
        }
    }

    Ok(Json(infos))
}

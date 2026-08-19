use std::sync::{Arc, RwLock};

use sqlx::PgPool;

use crate::config::{InstanceConfig, Settings};
use crate::services::{registry::ProviderSet, OllamaService};

#[derive(Clone)]
pub struct AppState {
    pub db:       PgPool,
    pub settings: Arc<Settings>,
    /// Admin-editable instance settings, refreshed in the background from the core.
    pub instance: Arc<RwLock<InstanceConfig>>,
    /// LLM services as of the last reload — rebuilt from `assistant.provider_config`
    /// whenever an administrator changes a provider, and on the settings refresh.
    pub providers: Arc<RwLock<ProviderSet>>,
    /// The locally hosted engine as built at start-up. Kept so a reload that
    /// cannot build one has something valid to fall back to.
    pub boot_local: Arc<OllamaService>,
}

impl AppState {
    /// Snapshot of the current instance settings. Takes the read lock briefly, so
    /// callers never hold it across `.await`. A poisoned lock yields the compiled
    /// defaults rather than a panic: losing a value must not take the module down.
    pub fn instance(&self) -> InstanceConfig {
        self.instance
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// Snapshot of the live provider set. Cheap: four reference-counted handles.
    /// A poisoned lock still yields the last written set — dropping every
    /// provider would silently break every conversation.
    pub fn providers(&self) -> ProviderSet {
        match self.providers.read() {
            Ok(p)  => p.clone(),
            Err(e) => e.into_inner().clone(),
        }
    }

    /// Rebuilds the provider set from the database, the deploy config and the
    /// current instance policy. Called at boot, after an administrator edits a
    /// provider, and on every settings refresh.
    pub async fn reload_providers(&self) {
        let fresh = ProviderSet::load(
            &self.db,
            &self.settings,
            &self.instance(),
            &self.boot_local,
        )
        .await;
        match self.providers.write() {
            Ok(mut guard) => *guard = fresh,
            Err(e) => tracing::error!(error = %e, "Jeu de fournisseurs non mis à jour"),
        }
    }
}

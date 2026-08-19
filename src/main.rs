use anyhow::{Context, Result};
use kubuno_assistant::{
    config::{InstanceConfig, Settings},
    router,
    services::{registry::ProviderSet, OllamaService},
    state::AppState,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// ── CLI dispatch ──────────────────────────────────────────────────────────────

/// Called by `kubuno assistant:<cmd>` — the first arg is the sub-command name.
/// Additional args are passed through.
async fn run_cli_command(cmd: &str, args: &[String]) -> Result<()> {
    let settings = Settings::load().context("Chargement configuration")?;

    match cmd {
        "models" => {
            let svc = OllamaService::new(&settings.ollama.url, &settings.ollama.default_model, 30)
                .context("Connexion Ollama")?;
            println!("Modèles disponibles :");
            println!("  [{provider:^10}] MODÈLE", provider = "FOURNISSEUR");
            println!("  {}", "─".repeat(60));
            match svc.list_models().await {
                Ok(models) => {
                    for m in &models {
                        let marker = if *m == settings.ollama.default_model { "  ★" } else { "   " };
                        println!("{marker} [{:^10}] {m}", "ollama");
                    }
                    println!("\n  {} modèle(s) Ollama", models.len());
                }
                Err(e) => println!("  Ollama inaccessible : {e}"),
            }
            if settings.providers.openai.enabled {
                println!("   [{:^10}] {}", "openai", settings.providers.openai.default_model);
            }
            if settings.providers.anthropic.enabled {
                println!("   [{:^10}] {} (et autres)", "anthropic", settings.providers.anthropic.default_model);
            }
            if settings.providers.google.enabled {
                println!("   [{:^10}] {} (et autres)", "google", settings.providers.google.default_model);
            }
        }

        "providers" => {
            println!("Fournisseurs LLM configurés :");
            println!("  {:<12} {:<8} MODÈLE PAR DÉFAUT", "FOURNISSEUR", "ACTIVÉ");
            println!("  {}", "─".repeat(60));
            let ollama_ok = OllamaService::new(&settings.ollama.url, &settings.ollama.default_model, 5)
                .is_ok();
            println!("  {:<12} {:<8} {}  ({})",
                "ollama",
                if settings.ollama.enabled { "✓" } else { "✗" },
                settings.ollama.default_model,
                if ollama_ok { &settings.ollama.url } else { "inaccessible" });
            println!("  {:<12} {:<8} {}",
                "openai",
                if settings.providers.openai.enabled { "✓" } else { "✗" },
                settings.providers.openai.default_model);
            println!("  {:<12} {:<8} {}",
                "anthropic",
                if settings.providers.anthropic.enabled { "✓" } else { "✗" },
                settings.providers.anthropic.default_model);
            println!("  {:<12} {:<8} {}",
                "google",
                if settings.providers.google.enabled { "✓" } else { "✗" },
                settings.providers.google.default_model);
        }

        "agents" => {
            let opts = settings.database.connect_options()?;
            let pool = PgPoolOptions::new().max_connections(2).connect_with(opts).await
                .context("Connexion PostgreSQL")?;

            #[derive(sqlx::FromRow)]
            struct AgentRow { name: String, description: Option<String>, is_system: bool }

            let agents = sqlx::query_as::<_, AgentRow>(
                "SELECT name, description, is_system FROM assistant.agents ORDER BY is_system DESC, name"
            ).fetch_all(&pool).await?;

            if agents.is_empty() {
                println!("Aucun agent configuré.");
            } else {
                println!("Agents Assistant :");
                for a in &agents {
                    let tag = if a.is_system { "[système]" } else { "[perso]  " };
                    let desc = a.description.as_deref().unwrap_or("");
                    println!("  {} {} — {}", tag, a.name, desc);
                }
                println!("\n  {} agent(s) au total", agents.len());
            }
        }

        "chat" => {
            let _model = args.iter().position(|a| a == "--model")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str)
                .unwrap_or(&settings.ollama.default_model);
            println!("Le chat interactif nécessite un terminal TTY.");
            println!("Utilisez l'interface web : http://{}:{}/assistant", settings.server.host, settings.server.port);
            println!("Ou démarrez le service avec : systemctl start kubuno-assistant");
        }

        unknown => {
            eprintln!("Commande assistant inconnue : {unknown}");
            eprintln!("Commandes disponibles : chat, models, providers, agents");
            std::process::exit(1);
        }
    }
    Ok(())
}

// ── Lecture de module.toml ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct Manifest {
    module:        ManifestModule,
    #[serde(default)]
    sidebar_items: Vec<SidebarItemRaw>,
    events:        Option<ManifestEvents>,
    #[serde(default)]
    cli_commands:  Vec<serde_json::Value>,
    /// Declarative instance settings, rendered by the core's generic admin form.
    #[serde(default)]
    settings:      Vec<SettingDefRaw>,
    /// Pages the admin panel is split into (`[[setting_groups]]`).
    #[serde(default)]
    setting_groups: Vec<SettingGroupRaw>,
}

/// One `[[setting_groups]]` entry of module.toml, forwarded verbatim. `id` is a
/// STABLE, UNTRANSLATED slug: it travels in the URL of the admin page.
#[derive(Deserialize, serde::Serialize)]
struct SettingGroupRaw {
    id:          String,
    label:       String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon:        Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position:    Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// One `[[settings]]` entry from module.toml, forwarded verbatim. The bounds are
/// part of the payload so the core's write path enforces them too — a console can
/// be bypassed, a server-side check cannot.
#[derive(Deserialize, serde::Serialize)]
struct SettingDefRaw {
    key:         String,
    scope:       String,
    #[serde(rename = "type")]
    value_type:  String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    values:      Option<serde_json::Value>,
    default:     serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label:       Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category:    Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group:       Option<String>,
    #[serde(default)]
    public:      bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    advanced:    bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    risk:        Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min:         Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max:         Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit:        Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    placeholder: Option<String>,
    /// The `string` value is a LIST, one entry per line -> textarea.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    multiline:   bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    depends_on:  Option<String>,
}

#[derive(Deserialize)]
struct ManifestModule {
    #[allow(dead_code)]
    id:            String,
    display_name:  String,
    description:   Option<String>,
    settings_path: Option<String>,
}

#[derive(Deserialize)]
struct SidebarItemRaw {
    id:       String,
    label:    String,
    icon:     String,
    path:     String,
    position: i32,
}

#[derive(Deserialize)]
struct ManifestEvents {
    #[serde(default)]
    subscribed: Vec<String>,
}

fn load_manifest() -> Option<Manifest> {
    let path = if let Ok(dir) = std::env::var("KUBUNO_MODULE_DIR") {
        std::path::PathBuf::from(dir).join("module.toml")
    } else {
        std::env::current_exe().ok()?.parent()?.join("module.toml")
    };

    let content = std::fs::read_to_string(&path)
        .map_err(|e| tracing::warn!(path = %path.display(), error = %e, "module.toml introuvable"))
        .ok()?;

    toml::from_str::<Manifest>(&content)
        .map_err(|e| tracing::error!(path = %path.display(), error = %e, "module.toml invalide"))
        .ok()
}

// ── Réglages d'instance : effets de bord ───────────────────────────────────

/// Whether two settings snapshots would produce the SAME provider set.
///
/// Only these two fields reach the constructors, so comparing them avoids
/// rebuilding four HTTP clients — and dropping their connection pools — once a
/// minute for nothing.
fn same_policy(a: &InstanceConfig, b: &InstanceConfig) -> bool {
    a.allow_cloud_providers == b.allow_cloud_providers
        && a.max_output_tokens == b.max_output_tokens
}

/// Deletes conversations untouched for longer than the instance keeps them.
///
/// Age is counted from the LAST activity, not from creation: a conversation
/// somebody still uses is not old. Messages follow through the foreign key, and
/// the delta tombstones are written by the table's own trigger, so a client that
/// synchronises learns about the removal. `0` = kept forever.
async fn purge_expired_conversations(state: &AppState) {
    let days = state.instance().conversation_retention_days;
    if days <= 0 {
        return;
    }
    match sqlx::query(
        "DELETE FROM assistant.conversations \
         WHERE updated_at < NOW() - ($1::int * INTERVAL '1 day')",
    )
    .bind(days as i32)
    .execute(&state.db)
    .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            tracing::info!(
                deleted = res.rows_affected(),
                retention_days = days,
                "Purge des conversations expirées"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::error!(error = %e, "Purge des conversations expirées"),
    }
}

// ── Point d'entrée ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    // If invoked as `kubuno-assistant <command> [args]`, run CLI mode
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd) = cli_args.first() {
        // Commands that don't start with '--' are CLI sub-commands
        if !cmd.starts_with('-') {
            return run_cli_command(cmd, &cli_args[1..]).await;
        }
    }

    let settings = Settings::load().context("Chargement de la configuration")?;

    let log_level = settings.logging.level.clone();
    let subscriber = tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level)),
    );
    match settings.logging.format {
        kubuno_assistant::config::LogFormat::Json   => subscriber.json().init(),
        kubuno_assistant::config::LogFormat::Pretty => subscriber.init(),
    }

    tracing::info!("Kubuno Assistant v{} démarrage…", env!("CARGO_PKG_VERSION"));

    // Sécurité : interdire toute exécution de processus sur l’hôte (voir kubuno-seccomp).
    kubuno_seccomp::lock_down_process_execution("assistant");

    // Pool PostgreSQL
    let opts = settings.database.connect_options()?;
    let pool = PgPoolOptions::new()
        .max_connections(settings.database.max_connections)
        .min_connections(settings.database.min_connections)
        .acquire_timeout(settings.database.connect_timeout)
        .connect_with(opts)
        .await
        .context("Connexion PostgreSQL")?;

    // Migrations
    if settings.database.run_migrations {
        sqlx::query("CREATE SCHEMA IF NOT EXISTS assistant")
            .execute(&pool)
            .await
            .context("Création du schéma assistant")?;

        let migration_opts = settings
            .database
            .connect_options()?
            .options([("search_path", "assistant,public")]);
        let migration_pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(settings.database.connect_timeout)
            .connect_with(migration_opts)
            .await
            .context("Pool de migration")?;

        sqlx::migrate!("./migrations")
            .run(&migration_pool)
            .await
            .context("Migrations")?;
    }

    // Local engine as built from the deploy config. It is the fallback the live
    // provider set falls back to when a stored row cannot be turned into a
    // working service; building it here means the failure is loud at start-up.
    let boot_local = Arc::new(
        OllamaService::new(
            &settings.ollama.url,
            &settings.ollama.default_model,
            settings.ollama.timeout_secs,
        )
        .context("Initialisation du moteur local")?,
    );

    let http = Client::new();

    // Admin-editable instance settings. A core that is not up yet leaves the
    // compiled defaults in place; the refresher below picks them up later.
    let instance = kubuno_assistant::config::fetch_instance(
        &http, &settings.core.url, &settings.core.internal_secret,
    )
    .await
    .unwrap_or_default();

    // Providers come from `assistant.provider_config`, narrowed by the instance
    // policy. Built before the server accepts a request so the first message
    // already sees what the administrator configured.
    let providers = ProviderSet::load(&pool, &settings, &instance, &boot_local).await;

    let state = AppState {
        db:        pool,
        settings:  Arc::new(settings.clone()),
        instance:  Arc::new(RwLock::new(instance)),
        providers: Arc::new(RwLock::new(providers)),
        boot_local,
    };

    // Refresh the instance settings every 60s so an admin edit takes effect
    // without a restart, and rebuild the providers with them: the answer-length
    // ceiling and the remote-provider policy are both applied at construction.
    {
        let http_r  = http.clone();
        let state_r = state.clone();
        let core_url = settings.core.url.clone();
        let secret   = settings.core.internal_secret.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                if let Some(fresh) =
                    kubuno_assistant::config::fetch_instance(&http_r, &core_url, &secret).await
                {
                    let changed = match state_r.instance.write() {
                        Ok(mut guard) => {
                            let differs = !same_policy(&guard, &fresh);
                            *guard = fresh;
                            differs
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Réglages d'instance non mis à jour");
                            false
                        }
                    };
                    if changed {
                        state_r.reload_providers().await;
                    }
                }
            }
        });
    }

    // Conversation retention. Hourly rather than on a timer per conversation:
    // the setting is a ceiling on age, not a promise about the minute.
    {
        let state_r = state.clone();
        tokio::spawn(async move {
            loop {
                purge_expired_conversations(&state_r).await;
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }

    // Enregistrement auprès du core
    register_with_core(&http, &settings).await;

    // Heartbeat toutes les 30s
    {
        let http2     = http.clone();
        let settings2 = settings.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let url    = format!("{}/internal/modules/assistant/heartbeat", settings2.core.url);
                let secret = &settings2.core.internal_secret;
                match http2.post(&url).header("X-Internal-Secret", secret.as_str()).send().await {
                    Ok(r) if r.status().is_success() => {}
                    Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
                        tracing::info!("Heartbeat 404 — ré-enregistrement…");
                        register_with_core(&http2, &settings2).await;
                    }
                    Ok(r) if r.status() == reqwest::StatusCode::FORBIDDEN => {
                        tracing::info!("Module désactivé par l'admin, attente…");
                    }
                    Ok(r)  => tracing::warn!(status = %r.status(), "Heartbeat réponse inattendue"),
                    Err(e) => tracing::warn!(error = %e, "Heartbeat erreur réseau"),
                }
            }
        });
    }

    let addr = format!("{}:{}", settings.server.host, settings.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Bind sur {addr}"))?;

    tracing::info!("Kubuno Assistant démarré sur http://{addr}");

    let app = router::build(state);
    axum::serve(listener, app.into_make_service())
        .await
        .context("Erreur du serveur HTTP")?;

    Ok(())
}

fn backoff(attempt: u32) -> u64 {
    if attempt <= 10 { (attempt * 2) as u64 } else { 30 }
}

async fn register_with_core(http: &Client, settings: &Settings) {
    let base_url = format!("http://{}:{}", settings.server.host, settings.server.port);
    let core_url = &settings.core.url;
    let secret   = &settings.core.internal_secret;

    let manifest = load_manifest();
    let display_name = manifest
        .as_ref()
        .map(|m| m.module.display_name.as_str())
        .unwrap_or("Assistant")
        .to_string();
    let description = manifest
        .as_ref()
        .and_then(|m| m.module.description.clone());
    let sidebar_items: Vec<Value> = manifest
        .as_ref()
        .map(|m| {
            m.sidebar_items
                .iter()
                .map(|s| {
                    json!({
                        "id":       s.id,
                        "label":    s.label,
                        "icon":     s.icon,
                        "path":     s.path,
                        "position": s.position,
                    })
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![json!({
                "id":       "assistant",
                "label":    "Assistant",
                "icon":     "Sparkles",
                "path":     "/assistant",
                "position": 50,
            })]
        });
    let subscribed_events: Vec<String> = manifest
        .as_ref()
        .and_then(|m| m.events.as_ref())
        .map(|e| e.subscribed.clone())
        .unwrap_or_default();
    let cli_commands: Vec<Value> = manifest
        .as_ref()
        .map(|m| m.cli_commands.clone())
        .unwrap_or_default();
    let settings_path = manifest
        .as_ref()
        .and_then(|m| m.module.settings_path.clone());
    let settings_schema: Vec<Value> = manifest
        .as_ref()
        .map(|m| m.settings.iter().map(|s| serde_json::to_value(s).unwrap_or(Value::Null)).collect())
        .unwrap_or_default();
    let setting_groups: Vec<Value> = manifest
        .as_ref()
        .map(|m| m.setting_groups.iter().map(|g| serde_json::to_value(g).unwrap_or(Value::Null)).collect())
        .unwrap_or_default();

    let payload = json!({
        "module_id":         "assistant",
        "display_name":      display_name,
        "description":       description,
        "base_url":          base_url,
        "version":           env!("CARGO_PKG_VERSION"),
        "routes":            [{ "method": "*", "path": "/*" }],
        "sidebar_items":     sidebar_items,
        "subscribed_events": subscribed_events,
        "cli_commands":      cli_commands,
        "settings_path":     settings_path,
        "settings_schema":   settings_schema,
        "setting_groups":    setting_groups,
    });

    for attempt in 1u32.. {
        let url = format!("{core_url}/internal/modules/register");
        match http
            .post(&url)
            .header("X-Internal-Secret", secret.as_str())
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Module assistant enregistré auprès du core");
                return;
            }
            Ok(resp) if resp.status() == reqwest::StatusCode::FORBIDDEN => {
                tracing::info!(attempt, "Module désactivé, nouvel essai dans 30s…");
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
            Ok(resp) => {
                let wait = backoff(attempt);
                tracing::warn!(attempt, status = %resp.status(), "Enregistrement échoué, retry dans {wait}s…");
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
            Err(e) => {
                let wait = backoff(attempt);
                tracing::warn!(attempt, error = %e, "Core inaccessible, retry dans {wait}s…");
                tokio::time::sleep(Duration::from_secs(wait)).await;
            }
        }
    }
    unreachable!()
}

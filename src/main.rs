use anyhow::{Context, Result};
use kubuno_jarvis::{
    config::Settings,
    router,
    services::{AnthropicService, GoogleService, OllamaService, OpenAiService},
    state::AppState,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;

// ── CLI dispatch ──────────────────────────────────────────────────────────────

/// Called by `kubuno jarvis:<cmd>` — the first arg is the sub-command name.
/// Additional args are passed through.
async fn run_cli_command(cmd: &str, args: &[String]) -> Result<()> {
    let settings = Settings::load().context("Chargement configuration")?;

    match cmd {
        "models" => {
            let svc = OllamaService::new(&settings.ollama.url, &settings.ollama.default_model, 30)
                .context("Connexion Ollama")?;
            println!("Modèles disponibles :");
            println!("  [{provider:^10}] {id}", provider = "FOURNISSEUR", id = "MODÈLE");
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
            println!("  {:<12} {:<8} {}", "FOURNISSEUR", "ACTIVÉ", "MODÈLE PAR DÉFAUT");
            println!("  {}", "─".repeat(60));
            let ollama_ok = OllamaService::new(&settings.ollama.url, &settings.ollama.default_model, 5)
                .map_or(false, |_| true);
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
                "SELECT name, description, is_system FROM jarvis.agents ORDER BY is_system DESC, name"
            ).fetch_all(&pool).await?;

            if agents.is_empty() {
                println!("Aucun agent configuré.");
            } else {
                println!("Agents Jarvis :");
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
            println!("Utilisez l'interface web : http://{}:{}/jarvis", settings.server.host, settings.server.port);
            println!("Ou démarrez le service avec : systemctl start kubuno-jarvis");
        }

        unknown => {
            eprintln!("Commande jarvis inconnue : {unknown}");
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

// ── Point d'entrée ─────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    // If invoked as `kubuno-jarvis <command> [args]`, run CLI mode
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
        kubuno_jarvis::config::LogFormat::Json   => subscriber.json().init(),
        kubuno_jarvis::config::LogFormat::Pretty => subscriber.init(),
    }

    tracing::info!("Kubuno Jarvis v{} démarrage…", env!("CARGO_PKG_VERSION"));

    // Sécurité : interdire toute exécution de processus sur l’hôte (voir kubuno-seccomp).
    kubuno_seccomp::lock_down_process_execution("jarvis");

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
        sqlx::query("CREATE SCHEMA IF NOT EXISTS jarvis")
            .execute(&pool)
            .await
            .context("Création du schéma jarvis")?;

        let migration_opts = settings
            .database
            .connect_options()?
            .options([("search_path", "jarvis,public")]);
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

    // Service Ollama (always initialized; may not be reachable if ollama.enabled = false)
    let ollama = Arc::new(
        OllamaService::new(
            &settings.ollama.url,
            &settings.ollama.default_model,
            settings.ollama.timeout_secs,
        )
        .context("Initialisation OllamaService")?,
    );

    // Optional cloud providers (loaded from DB at startup; can be updated via API)
    let openai = if settings.providers.openai.enabled && !settings.providers.openai.api_key.is_empty() {
        Some(Arc::new(OpenAiService::new(
            &settings.providers.openai.base_url,
            &settings.providers.openai.api_key,
            &settings.providers.openai.default_model,
        ).context("Initialisation OpenAiService")?))
    } else { None };

    let anthropic = if settings.providers.anthropic.enabled && !settings.providers.anthropic.api_key.is_empty() {
        Some(Arc::new(AnthropicService::new(
            &settings.providers.anthropic.base_url,
            &settings.providers.anthropic.api_key,
            &settings.providers.anthropic.default_model,
        ).context("Initialisation AnthropicService")?))
    } else { None };

    let google = if settings.providers.google.enabled && !settings.providers.google.api_key.is_empty() {
        Some(Arc::new(GoogleService::new(
            &settings.providers.google.base_url,
            &settings.providers.google.api_key,
            &settings.providers.google.default_model,
        ).context("Initialisation GoogleService")?))
    } else { None };

    let state = AppState {
        db:        pool,
        settings:  Arc::new(settings.clone()),
        ollama,
        openai,
        anthropic,
        google,
    };

    // Enregistrement auprès du core
    let http = Client::new();
    register_with_core(&http, &settings).await;

    // Heartbeat toutes les 30s
    {
        let http2     = http.clone();
        let settings2 = settings.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let url    = format!("{}/internal/modules/jarvis/heartbeat", settings2.core.url);
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

    tracing::info!("Kubuno Jarvis démarré sur http://{addr}");

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
        .unwrap_or("Jarvis")
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
                "id":       "jarvis",
                "label":    "Jarvis",
                "icon":     "Sparkles",
                "path":     "/jarvis",
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

    let payload = json!({
        "module_id":         "jarvis",
        "display_name":      display_name,
        "description":       description,
        "base_url":          base_url,
        "version":           env!("CARGO_PKG_VERSION"),
        "routes":            [{ "method": "*", "path": "/*" }],
        "sidebar_items":     sidebar_items,
        "subscribed_events": subscribed_events,
        "cli_commands":      cli_commands,
        "settings_path":     settings_path,
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
                tracing::info!("Module jarvis enregistré auprès du core");
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

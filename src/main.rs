mod agent;
mod api;
mod app;
mod config;
mod db;
mod drill;
mod eval;
mod inference;
mod models;
mod profile;
mod sandbox;
mod store;
mod verify;

use anyhow::Result;
use app::AppState;
use axum::Router;
use config::Config;
use inference::InferenceClient;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    let db = db::init(&cfg.database_url).await?;
    let (sandbox, sandbox_health) = sandbox::build(&cfg).await;

    let canary_path = std::env::var("PROOFSHEET_CANARY_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| cfg.data_dir.join("canary.txt"));
    if !canary_path.exists() {
        if let Some(parent) = canary_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &canary_path,
            format!("host-canary-{}", uuid::Uuid::new_v4().simple()),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&canary_path, std::fs::Permissions::from_mode(0o600));
        }
    }

    let inference = InferenceClient::new(
        cfg.inference_base_url.clone(),
        cfg.inference_api_key.clone(),
    );

    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "localhost".into());

    tracing::info!(
        "sandbox backend: {} (ready={}) image={}",
        sandbox_health.label,
        sandbox_health.ready,
        sandbox_health.image
    );
    if !sandbox_health.ready {
        tracing::warn!(
            "sandbox is not ready: {}. Runs will be refused until it is.",
            sandbox_health.detail
        );
    }

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        db,
        sandbox,
        sandbox_health,
        inference,
        started_at: std::time::Instant::now(),
        hostname,
        canary_path,
        run_slots: Arc::new(tokio::sync::Semaphore::new(2)),
    });

    let app = Router::new()
        .nest("/api", api::router())
        .fallback(api::static_handler)
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!("Proofsheet listening on http://{}", cfg.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

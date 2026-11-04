use crate::config::Config;
use crate::inference::InferenceClient;
use crate::sandbox::{Sandbox, SandboxHealth};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Instant;

pub struct AppState {
    pub cfg: Config,
    pub db: SqlitePool,
    pub sandbox: Arc<dyn Sandbox>,
    pub sandbox_health: SandboxHealth,
    pub inference: InferenceClient,
    pub started_at: Instant,
    pub hostname: String,
    pub canary_path: std::path::PathBuf,
    pub run_slots: Arc<tokio::sync::Semaphore>,
}

impl AppState {
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

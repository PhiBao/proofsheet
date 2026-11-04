use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub data_dir: PathBuf,
    pub database_url: String,
    pub inference_base_url: String,
    pub inference_api_key: String,
    pub model_plan: String,
    pub model_code: String,
    pub model_repair: String,
    pub sandbox_backend: String,
    pub sandbox_image: String,
    pub sandbox_timeout_secs: u64,
    pub sandbox_memory: String,
    pub sandbox_cpus: String,
    pub region: String,
    pub max_upload_bytes: usize,
    pub max_repair_attempts: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir = PathBuf::from(std::env::var("PROOFSHEET_DATA_DIR").unwrap_or_else(|_| "data".into()));
        std::fs::create_dir_all(&data_dir).context("create data dir")?;
        std::fs::create_dir_all(data_dir.join("uploads")).context("create uploads dir")?;
        std::fs::create_dir_all(data_dir.join("artifacts")).context("create artifacts dir")?;
        std::fs::create_dir_all(data_dir.join("work")).context("create work dir")?;

        let inference_api_key = std::env::var("VULTR_INFERENCE_API_KEY")
            .context("VULTR_INFERENCE_API_KEY is required (create a Serverless Inference subscription)")?;

        Ok(Self {
            bind: std::env::var("PROOFSHEET_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            database_url: std::env::var("PROOFSHEET_DATABASE_URL")
                .unwrap_or_else(|_| format!("sqlite://{}/proofsheet.db", data_dir.display())),
            data_dir,
            inference_base_url: std::env::var("VULTR_INFERENCE_BASE_URL")
                .unwrap_or_else(|_| "https://api.vultrinference.com/v1".into()),
            inference_api_key,
            model_plan: std::env::var("PROOFSHEET_MODEL_PLAN")
                .unwrap_or_else(|_| "deepseek-v4.1-flash-1017".into()),
            model_code: std::env::var("PROOFSHEET_MODEL_CODE")
                .unwrap_or_else(|_| "qwen3.8-27b".into()),
            model_repair: std::env::var("PROOFSHEET_MODEL_REPAIR")
                .unwrap_or_else(|_| "deepseek-v4.1-flash-1017".into()),
            sandbox_backend: std::env::var("PROOFSHEET_SANDBOX").unwrap_or_else(|_| "auto".into()),
            sandbox_image: std::env::var("PROOFSHEET_SANDBOX_IMAGE")
                .unwrap_or_else(|_| "proofsheet-sandbox:latest".into()),
            sandbox_timeout_secs: std::env::var("PROOFSHEET_SANDBOX_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(90),
            sandbox_memory: std::env::var("PROOFSHEET_SANDBOX_MEMORY").unwrap_or_else(|_| "1g".into()),
            sandbox_cpus: std::env::var("PROOFSHEET_SANDBOX_CPUS").unwrap_or_else(|_| "1.5".into()),
            region: std::env::var("PROOFSHEET_REGION").unwrap_or_else(|_| "local".into()),
            max_upload_bytes: std::env::var("PROOFSHEET_MAX_UPLOAD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(12 * 1024 * 1024),
            max_repair_attempts: std::env::var("PROOFSHEET_MAX_REPAIRS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        })
    }
}

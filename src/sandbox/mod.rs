pub mod docker;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SandboxSpec {
    /// Short label used in container names and evidence.
    pub label: String,
    pub workdir: PathBuf,
    pub command: Vec<String>,
    pub timeout: Duration,
    pub memory: Option<String>,
    pub cpus: Option<String>,
    pub network: bool,
    pub read_only_workdir: bool,
}

#[derive(Clone, Debug)]
pub struct SandboxResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i64>,
    pub duration_ms: i64,
    pub sandbox_id: String,
    /// gvisor | docker | microvm
    pub isolation: String,
    pub oom_killed: bool,
    pub timed_out: bool,
    pub destroyed: bool,
}

#[async_trait::async_trait]
pub trait Sandbox: Send + Sync {
    fn isolation(&self) -> String;
    fn describe(&self) -> String;
    async fn run(&self, spec: SandboxSpec) -> Result<SandboxResult>;
    async fn healthy(&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct SandboxHealth {
    pub isolation: String,
    pub label: String,
    pub image: String,
    pub ready: bool,
    pub detail: String,
}

pub async fn build(cfg: &crate::config::Config) -> (Arc<dyn Sandbox>, SandboxHealth) {
    let requested = cfg.sandbox_backend.as_str();

    if requested == "microsandbox" {
        let backend = docker::DockerSandbox::new(
            cfg.sandbox_image.clone(),
            None,
            cfg.sandbox_memory.clone(),
            cfg.sandbox_cpus.clone(),
        );
        return degraded(backend, cfg, "microsandbox backend not installed; refusing to fake isolation").await;
    }

    if requested == "docker" {
        let backend = docker::DockerSandbox::new(
            cfg.sandbox_image.clone(),
            None,
            cfg.sandbox_memory.clone(),
            cfg.sandbox_cpus.clone(),
        );
        return finalize(backend, cfg).await;
    }

    if requested == "runsc" {
        let backend = docker::DockerSandbox::new(
            cfg.sandbox_image.clone(),
            Some("runsc".to_string()),
            cfg.sandbox_memory.clone(),
            cfg.sandbox_cpus.clone(),
        );
        return finalize(backend, cfg).await;
    }

    // auto: prefer gVisor, fall back to plain Docker. Report which one honestly.
    let gvisor = docker::DockerSandbox::new(
        cfg.sandbox_image.clone(),
        Some("runsc".to_string()),
        cfg.sandbox_memory.clone(),
        cfg.sandbox_cpus.clone(),
    );
    if gvisor.healthy().await {
        let health = SandboxHealth {
            isolation: "gvisor".into(),
            label: "gVisor user-space kernel".into(),
            image: cfg.sandbox_image.clone(),
            ready: true,
            detail: "Docker runtime runsc is available".into(),
        };
        return (Arc::new(gvisor), health);
    }

    let docker = docker::DockerSandbox::new(
        cfg.sandbox_image.clone(),
        None,
        cfg.sandbox_memory.clone(),
        cfg.sandbox_cpus.clone(),
    );
    finalize(docker, cfg).await
}

async fn finalize(backend: docker::DockerSandbox, cfg: &crate::config::Config) -> (Arc<dyn Sandbox>, SandboxHealth) {
    let ready = backend.healthy().await;
    let health = SandboxHealth {
        isolation: backend.isolation(),
        label: backend.describe(),
        image: cfg.sandbox_image.clone(),
        ready,
        detail: if ready {
            "Sandbox image answered a probe run".into()
        } else {
            "Sandbox image or Docker runtime is not available".into()
        },
    };
    (Arc::new(backend), health)
}

async fn degraded(
    backend: docker::DockerSandbox,
    cfg: &crate::config::Config,
    detail: &str,
) -> (Arc<dyn Sandbox>, SandboxHealth) {
    let health = SandboxHealth {
        isolation: backend.isolation(),
        label: backend.describe(),
        image: cfg.sandbox_image.clone(),
        ready: false,
        detail: detail.to_string(),
    };
    (Arc::new(backend), health)
}

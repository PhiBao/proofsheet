use super::{Sandbox, SandboxResult, SandboxSpec};
use anyhow::{anyhow, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::process::Command;

#[derive(Clone)]
pub struct DockerSandbox {
    image: String,
    runtime: Option<String>,
    memory: String,
    cpus: String,
}

impl DockerSandbox {
    pub fn new(image: String, runtime: Option<String>, memory: String, cpus: String) -> Self {
        Self {
            image,
            runtime,
            memory,
            cpus,
        }
    }

    fn isolation_code(&self) -> String {
        if self.runtime.as_deref() == Some("runsc") {
            "gvisor".to_string()
        } else {
            "docker".to_string()
        }
    }

    fn docker_args(&self, spec: &SandboxSpec, name: &str) -> Vec<String> {
        let mut args: Vec<String> = vec!["run".into(), "--name".into(), name.into()];
        if let Some(rt) = &self.runtime {
            args.push("--runtime".into());
            args.push(rt.clone());
        }
        args.extend([
            "--rm=false".into(),
            "--network".into(),
            if spec.network { "bridge".into() } else { "none".into() },
            "--read-only".into(),
            "--tmpfs".into(),
            "/tmp:rw,size=256m".into(),
            "--memory".into(),
            spec.memory.clone().unwrap_or_else(|| self.memory.clone()),
            "--memory-swap".into(),
            spec.memory.clone().unwrap_or_else(|| self.memory.clone()),
            "--cpus".into(),
            spec.cpus.clone().unwrap_or_else(|| self.cpus.clone()),
            "--pids-limit".into(),
            "192".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
            "--user".into(),
            "65534:65534".into(),
            "-e".into(),
            "HOME=/tmp".into(),
            "-e".into(),
            "MPLCONFIGDIR=/tmp/mpl".into(),
            "-e".into(),
            "PYTHONUNBUFFERED=1".into(),
            "-e".into(),
            "PYTHONDONTWRITEBYTECODE=1".into(),
            "-e".into(),
            "PROOFSHEET_SANDBOX=1".into(),
        ]);
        let mount = if spec.read_only_workdir { "ro" } else { "rw" };
        args.push("-v".into());
        args.push(format!("{}:/work:{mount}", spec.workdir.display()));
        args.extend([
            "-w".into(),
            "/work".into(),
            self.image.clone(),
        ]);
        args.extend(spec.command.clone());
        args
    }
}

#[async_trait::async_trait]
impl Sandbox for DockerSandbox {
    fn isolation(&self) -> String {
        self.isolation_code()
    }

    fn describe(&self) -> String {
        match self.runtime.as_deref() {
            Some("runsc") => "gVisor user-space kernel".to_string(),
            _ => "Docker container (hardened)".to_string(),
        }
    }

    async fn healthy(&self) -> bool {
        let mut args: Vec<String> = vec![
            "run".into(),
            "--rm".into(),
            "--network".into(),
            "none".into(),
            "--read-only".into(),
            "--tmpfs".into(),
            "/tmp:rw,size=32m".into(),
            "--cap-drop".into(),
            "ALL".into(),
            "--pids-limit".into(),
            "64".into(),
        ];
        if let Some(rt) = &self.runtime {
            args.push("--runtime".into());
            args.push(rt.clone());
        }
        args.push(self.image.clone());
        args.extend(
            ["python3", "-c", "print('proofsheet-sandbox-ok')"]
                .iter()
                .map(|s| s.to_string()),
        );

        let out = tokio::time::timeout(
            Duration::from_secs(30),
            Command::new("docker").args(&args).output(),
        )
        .await;

        match out {
            Ok(Ok(o)) => {
                o.status.success()
                    && String::from_utf8_lossy(&o.stdout).contains("proofsheet-sandbox-ok")
            }
            _ => false,
        }
    }

    async fn run(&self, spec: SandboxSpec) -> Result<SandboxResult> {
        std::fs::create_dir_all(&spec.workdir)?;
        make_writable(Path::new(&spec.workdir))?;

        let name = format!(
            "proofsheet-{}-{}",
            sanitize(&spec.label),
            &uuid::Uuid::new_v4().to_string()[..8]
        );

        let args = self.docker_args(&spec, &name);
        let started = Instant::now();
        let child = Command::new("docker")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("could not start docker: {e}"))?;

        let (stdout, stderr, mut exit_code, timed_out) =
            match tokio::time::timeout(spec.timeout, child.wait_with_output()).await {
                Ok(Ok(output)) => (
                    String::from_utf8_lossy(&output.stdout).to_string(),
                    String::from_utf8_lossy(&output.stderr).to_string(),
                    output.status.code().map(|c| c as i64),
                    false,
                ),
                Ok(Err(e)) => {
                    let _ = force_remove(&name).await;
                    return Err(anyhow!("docker run failed: {e}"));
                }
                Err(_) => {
                    let _ = force_remove(&name).await;
                    (
                        String::new(),
                        format!(
                            "sandbox killed: wall-clock limit of {}s exceeded",
                            spec.timeout.as_secs()
                        ),
                        None,
                        true,
                    )
                }
            };

        let duration_ms = started.elapsed().as_millis() as i64;

        // Inspect before removal to capture OOM and exit state.
        let inspect = Command::new("docker")
            .args([
                "inspect",
                &name,
                "--format",
                "{{json .State}}",
            ])
            .output()
            .await;
        let mut oom_killed = false;
        if let Ok(out) = inspect {
            if out.status.success() {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                    oom_killed = v.get("OOMKilled").and_then(|b| b.as_bool()).unwrap_or(false);
                    if exit_code.is_none() {
                        exit_code = v.get("ExitCode").and_then(|c| c.as_i64());
                    }
                }
            }
        }

        let destroyed = force_remove(&name).await;

        Ok(SandboxResult {
            stdout,
            stderr,
            exit_code,
            duration_ms,
            sandbox_id: name,
            isolation: self.isolation_code(),
            oom_killed,
            timed_out,
            destroyed,
        })
    }
}

async fn force_remove(name: &str) -> bool {
    let out = Command::new("docker").args(["rm", "-f", name]).output().await;
    matches!(out, Ok(o) if o.status.success())
}

fn make_writable(path: &Path) -> Result<()> {
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o777);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    cleaned.trim_matches('-').chars().take(32).collect()
}

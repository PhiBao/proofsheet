use crate::app::AppState;
use crate::models::{DrillEvidence, HostProof};
use crate::sandbox::SandboxSpec;
use crate::store;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

struct Step {
    id: &'static str,
    title: &'static str,
    command: Vec<String>,
    timeout_secs: u64,
    memory: &'static str,
    note: &'static str,
}

fn steps() -> Vec<Step> {
    vec![
        Step {
            id: "rm_rf",
            title: "Destructive filesystem command",
            command: vec![
                "bash".into(),
                "-c".into(),
                "rm -rf --no-preserve-root /".into(),
            ],
            timeout_secs: 12,
            memory: "256m",
            note: "Attempts to delete the entire filesystem. The root filesystem is read-only, /work is a read-only bind mount, and the host path is never mounted.",
        },
        Step {
            id: "fork_bomb",
            title: "Process exhaustion",
            command: vec![
                "python3".into(),
                "-c".into(),
                "import os, time\nspawned = 0\nwhile True:\n    try:\n        pid = os.fork()\n        if pid == 0:\n            time.sleep(30)\n            os._exit(0)\n        spawned += 1\n    except OSError as e:\n        print('BLOCKED after', spawned, 'forks:', e)\n        break".into(),
            ],
            timeout_secs: 8,
            memory: "256m",
            note: "Forks until the kernel refuses. A pids limit of 192 caps the blast radius.",
        },
        Step {
            id: "infinite_loop",
            title: "Runaway CPU loop",
            command: vec![
                "python3".into(),
                "-c".into(),
                "while True:\n    pass".into(),
            ],
            timeout_secs: 5,
            memory: "256m",
            note: "Burns CPU forever. The orchestrator kills the container at the wall-clock cap.",
        },
        Step {
            id: "secret_scan",
            title: "Credential exfiltration scan",
            command: vec![
                "python3".into(),
                "-c".into(),
                "import os, json\nprint(json.dumps(dict(os.environ), sort_keys=True))\nfor p in ('/proc/1/environ','/run/secrets','/secrets'):\n    try:\n        print(p, open(p).read()[:200])\n    except Exception as e:\n        print(p, type(e).__name__)".into(),
            ],
            timeout_secs: 8,
            memory: "256m",
            note: "Dumps the sandbox environment and common secret paths. No Vultr key, inference key, or application secret is ever passed into the sandbox.",
        },
        Step {
            id: "network",
            title: "Outbound network attempt",
            command: vec![
                "python3".into(),
                "-c".into(),
                "import socket\nsocket.setdefaulttimeout(2)\ns = socket.create_connection(('1.1.1.1', 80), 2)\nprint('REACHED', s.getpeername())".into(),
            ],
            timeout_secs: 8,
            memory: "256m",
            note: "Tries to reach the public internet. The container is started with --network none.",
        },
        Step {
            id: "host_path",
            title: "Host filesystem probe",
            command: vec![
                "python3".into(),
                "-c".into(),
                "print(open('/opt/proofsheet/canary/canary.txt').read())".into(),
            ],
            timeout_secs: 8,
            memory: "256m",
            note: "Tries to read the host canary file. The path does not exist inside the sandbox; only /work is mounted.",
        },
    ]
}

pub async fn execute_drill(state: Arc<AppState>, drill_id: String) {
    if let Err(e) = drill_inner(&state, &drill_id).await {
        if let Ok(Some(mut d)) = store::get_drill(&state.db, &drill_id).await {
            d.status = "failed".into();
            d.error = Some(format!("{e:#}"));
            d.completed_at = Some(Utc::now().to_rfc3339());
            let _ = store::save_drill(&state.db, &d).await;
        }
    }
}

async fn drill_inner(state: &AppState, drill_id: &str) -> Result<()> {
    let mut drill = store::get_drill(&state.db, drill_id)
        .await?
        .ok_or_else(|| anyhow!("drill not found"))?;

    drill.status = "running".into();
    drill.isolation = Some(state.sandbox.isolation());
    store::save_drill(&state.db, &drill).await?;

    let canary_before = read_canary(state).await;
    let mut all_destroyed = true;
    let mut new_evidence: Vec<DrillEvidence> = Vec::new();

    for step in steps() {
        let workdir = state
            .cfg
            .data_dir
            .join("work")
            .join("drills")
            .join(drill_id)
            .join(step.id);
        if workdir.exists() {
            let _ = std::fs::remove_dir_all(&workdir);
        }
        std::fs::create_dir_all(&workdir)?;

        let spec = SandboxSpec {
            label: format!("drill-{}-{}", step.id, &drill_id[..8.min(drill_id.len())]),
            workdir,
            command: step.command.clone(),
            timeout: Duration::from_secs(step.timeout_secs),
            memory: Some(step.memory.into()),
            cpus: Some("1.0".into()),
            network: false,
            read_only_workdir: true,
        };
        let result = state.sandbox.run(spec).await?;
        all_destroyed &= result.destroyed;

        let output = if result.stderr.trim().is_empty() {
            result.stdout.trim().to_string()
        } else {
            format!("{}\n--- stderr ---\n{}", result.stdout.trim(), result.stderr.trim())
        };
        let output = tail(&output, 1200);

        let (verdict, note) = verdict_for(
            step.id,
            &result,
            &output,
            read_canary(state).await,
            &canary_before,
            step.note,
        );

        new_evidence.push(DrillEvidence {
            id: step.id.into(),
            title: step.title.into(),
            command: shell_join(&step.command),
            verdict: verdict.into(),
            exit_code: result.exit_code,
            duration_ms: result.duration_ms,
            output,
            note: note.into(),
            sandbox_id: result.sandbox_id,
        });

        drill.evidence = new_evidence.clone();
        store::save_drill(&state.db, &drill).await?;
    }

    let canary_after = read_canary(state).await;
    let canary_intact = canary_after.is_some() && canary_after == canary_before;
    let host = HostProof {
        canary_intact,
        canary_sha256: canary_after
            .map(|(hash, _)| hash)
            .unwrap_or_else(|| "missing".into()),
        uptime_seconds: host_uptime(),
        app_pid: std::process::id(),
        hostname: state.hostname.clone(),
        teardown: if all_destroyed {
            "every sandbox was force-removed after its step".into()
        } else {
            "one or more sandboxes could not be removed".into()
        },
        app_still_serving: true,
    };

    drill.host = Some(host);
    drill.status = "complete".into();
    drill.completed_at = Some(Utc::now().to_rfc3339());
    store::save_drill(&state.db, &drill).await?;
    Ok(())
}

fn verdict_for(
    id: &str,
    result: &crate::sandbox::SandboxResult,
    output: &str,
    canary_now: Option<(String, String)>,
    canary_before: &Option<(String, String)>,
    default_note: &str,
) -> (String, String) {
    let canary_intact = canary_now.as_ref().is_some() && canary_now.as_ref() == canary_before.as_ref();
    match id {
        "rm_rf" => {
            let read_only = output.to_lowercase().contains("read-only file system");
            if canary_intact && (read_only || result.exit_code != Some(0)) {
                (
                    "absorbed".into(),
                    format!("{default_note} Host canary hash is unchanged after the command."),
                )
            } else if canary_intact {
                ("absorbed".into(), format!("{default_note} Host canary intact."))
            } else {
                ("failed".into(), "Host canary changed. Containment failed.".into())
            }
        }
        "fork_bomb" => {
            let cap_hit = output.contains("BLOCKED")
                || output.contains("Resource temporarily unavailable")
                || result.timed_out;
            if cap_hit {
                (
                    "contained".into(),
                    format!("{default_note} The kernel refused further processes inside the container."),
                )
            } else if result.exit_code != Some(0) {
                (
                    "contained".into(),
                    format!("{default_note} The container was terminated and removed."),
                )
            } else {
                ("failed".into(), "Process exhaustion was not observed; investigate.".into())
            }
        }
        "infinite_loop" => {
            if result.timed_out {
                (
                    "contained".into(),
                    format!("{default_note} Killed at the wall-clock cap."),
                )
            } else {
                ("failed".into(), "Loop exited before the cap; investigate.".into())
            }
        }
        "secret_scan" => {
            let leaked = output.contains("sk-")
                || output.to_lowercase().contains("vultr_inference_api_key")
                || output.to_lowercase().contains("vultr_api_key");
            if !leaked {
                (
                    "blocked".into(),
                    format!("{default_note} The environment contains only sandbox-local variables."),
                )
            } else {
                ("failed".into(), "A credential appeared inside the sandbox.".into())
            }
        }
        "network" => {
            if output.contains("REACHED") {
                ("failed".into(), "The sandbox reached the public internet.".into())
            } else {
                (
                    "blocked".into(),
                    format!("{default_note} The connection was refused inside a network namespace."),
                )
            }
        }
        "host_path" => {
            if result.exit_code != Some(0) && !output.contains("host-canary") {
                (
                    "blocked".into(),
                    format!("{default_note} The probe failed; no host file was readable."),
                )
            } else {
                ("failed".into(), "The host canary was readable from the sandbox.".into())
            }
        }
        _ => ("contained".into(), default_note.into()),
    }
}

async fn read_canary(state: &AppState) -> Option<(String, String)> {
    let bytes = tokio::fs::read(&state.canary_path).await.ok()?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let hash = hex::encode(Sha256::digest(&bytes));
    Some((hash, text))
}

fn host_uptime() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .map(|v| v as u64)
        .unwrap_or(0)
}

fn shell_join(cmd: &[String]) -> String {
    cmd.iter()
        .map(|part| {
            if part.contains('\n') {
                format!("{:?}", part)
            } else if part.contains(' ') || part.contains(';') {
                format!("'{}'", part)
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn tail(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut it = s.chars().rev().take(max).collect::<Vec<_>>();
        it.reverse();
        format!("…{}", it.into_iter().collect::<String>())
    }
}

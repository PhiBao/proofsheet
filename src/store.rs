use crate::models::*;
use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};

pub async fn insert_dataset(db: &SqlitePool, d: &Dataset) -> Result<()> {
    sqlx::query(
        "INSERT INTO datasets (id, filename, sha256, bytes, rows, cols, profile_json, created_at)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(&d.id)
    .bind(&d.filename)
    .bind(&d.sha256)
    .bind(d.bytes)
    .bind(d.rows)
    .bind(d.cols)
    .bind(serde_json::to_string(&d.profile)?)
    .bind(&d.created_at)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_dataset(db: &SqlitePool, id: &str) -> Result<Option<Dataset>> {
    let row = sqlx::query("SELECT * FROM datasets WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| Dataset {
        id: r.get("id"),
        filename: r.get("filename"),
        sha256: r.get("sha256"),
        bytes: r.get("bytes"),
        rows: r.get("rows"),
        cols: r.get("cols"),
        profile: serde_json::from_str(r.get::<String, _>("profile_json").as_str()).unwrap_or_default(),
        created_at: r.get("created_at"),
    }))
}

pub async fn insert_run(db: &SqlitePool, r: &Run) -> Result<()> {
    sqlx::query(
        "INSERT INTO runs (id, dataset_id, question, mode, status, attempt, plan_json, definition_json, code, stdout, stderr,
            exit_code, duration_ms, sandbox_id, isolation, oom_killed, metrics_json, chart_data_json, chart_audit_json,
            consensus_json, comparison_json, template, notes_markdown,
            claims_json, clean_report_json, log_json, error, replayed_from, created_at, completed_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&r.id)
    .bind(&r.dataset_id)
    .bind(&r.question)
    .bind(&r.mode)
    .bind(&r.status)
    .bind(r.attempt)
    .bind(r.plan.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.definition.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&r.code)
    .bind(&r.stdout)
    .bind(&r.stderr)
    .bind(r.exit_code)
    .bind(r.duration_ms)
    .bind(&r.sandbox_id)
    .bind(&r.isolation)
    .bind(r.oom_killed as i64)
    .bind(serde_json::to_string(&r.metrics)?)
    .bind(r.chart_data.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.chart_audit.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.consensus.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.comparison.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&r.template)
    .bind(&r.notes_markdown)
    .bind(serde_json::to_string(&r.claims)?)
    .bind(r.clean_report.as_ref().map(serde_json::to_string).transpose()?)
    .bind(serde_json::to_string(&r.log)?)
    .bind(&r.error)
    .bind(&r.replayed_from)
    .bind(&r.created_at)
    .bind(&r.completed_at)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn save_run(db: &SqlitePool, r: &Run) -> Result<()> {
    sqlx::query(
        "UPDATE runs SET status=?, attempt=?, plan_json=?, definition_json=?, code=?, stdout=?, stderr=?, exit_code=?,
            duration_ms=?, sandbox_id=?, isolation=?, oom_killed=?, metrics_json=?, chart_data_json=?, chart_audit_json=?,
            consensus_json=?, comparison_json=?, template=?, notes_markdown=?, claims_json=?, clean_report_json=?,
            log_json=?, error=?, replayed_from=?, completed_at=?
         WHERE id=?",
    )
    .bind(&r.status)
    .bind(r.attempt)
    .bind(r.plan.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.definition.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&r.code)
    .bind(&r.stdout)
    .bind(&r.stderr)
    .bind(r.exit_code)
    .bind(r.duration_ms)
    .bind(&r.sandbox_id)
    .bind(&r.isolation)
    .bind(r.oom_killed as i64)
    .bind(serde_json::to_string(&r.metrics)?)
    .bind(r.chart_data.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.chart_audit.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.consensus.as_ref().map(serde_json::to_string).transpose()?)
    .bind(r.comparison.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&r.template)
    .bind(&r.notes_markdown)
    .bind(serde_json::to_string(&r.claims)?)
    .bind(r.clean_report.as_ref().map(serde_json::to_string).transpose()?)
    .bind(serde_json::to_string(&r.log)?)
    .bind(&r.error)
    .bind(&r.replayed_from)
    .bind(&r.completed_at)
    .bind(&r.id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_run(db: &SqlitePool, id: &str) -> Result<Option<Run>> {
    let row = sqlx::query("SELECT * FROM runs WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(row_to_run))
}

pub async fn list_runs_for_dataset(db: &SqlitePool, dataset_id: &str) -> Result<Vec<RunSummary>> {
    let rows = sqlx::query(
        "SELECT id, dataset_id, question, mode, status, attempt, error, created_at, completed_at
         FROM runs WHERE dataset_id = ? ORDER BY created_at DESC LIMIT 20",
    )
    .bind(dataset_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| RunSummary {
            id: r.get("id"),
            dataset_id: r.get("dataset_id"),
            question: r.get("question"),
            mode: r.get("mode"),
            status: r.get("status"),
            attempt: r.get("attempt"),
            error: r.get("error"),
            created_at: r.get("created_at"),
            completed_at: r.get("completed_at"),
        })
        .collect())
}

fn row_to_run(r: sqlx::sqlite::SqliteRow) -> Run {
    Run {
        id: r.get("id"),
        dataset_id: r.get("dataset_id"),
        question: r.get("question"),
        mode: r.get("mode"),
        status: r.get("status"),
        attempt: r.get("attempt"),
        plan: r
            .get::<Option<String>, _>("plan_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        definition: r
            .get::<Option<String>, _>("definition_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        code: r.get("code"),
        stdout: r.get("stdout"),
        stderr: r.get("stderr"),
        exit_code: r.get("exit_code"),
        duration_ms: r.get("duration_ms"),
        sandbox_id: r.get("sandbox_id"),
        isolation: r.get("isolation"),
        oom_killed: r.get::<i64, _>("oom_killed") != 0,
        metrics: r
            .get::<Option<String>, _>("metrics_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        chart_data: r
            .get::<Option<String>, _>("chart_data_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        chart_audit: r
            .get::<Option<String>, _>("chart_audit_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        consensus: r
            .get::<Option<String>, _>("consensus_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        comparison: r
            .get::<Option<String>, _>("comparison_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        template: r.get("template"),
        notes_markdown: r.get("notes_markdown"),
        claims: r
            .get::<Option<String>, _>("claims_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        clean_report: r
            .get::<Option<String>, _>("clean_report_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        log: r
            .get::<Option<String>, _>("log_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        error: r.get("error"),
        replayed_from: r.get("replayed_from"),
        created_at: r.get("created_at"),
        completed_at: r.get("completed_at"),
    }
}

pub async fn insert_drill(db: &SqlitePool, d: &Drill) -> Result<()> {
    sqlx::query(
        "INSERT INTO drills (id, status, isolation, evidence_json, host_json, error, created_at, completed_at)
         VALUES (?,?,?,?,?,?,?,?)",
    )
    .bind(&d.id)
    .bind(&d.status)
    .bind(&d.isolation)
    .bind(serde_json::to_string(&d.evidence)?)
    .bind(d.host.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&d.error)
    .bind(&d.created_at)
    .bind(&d.completed_at)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn save_drill(db: &SqlitePool, d: &Drill) -> Result<()> {
    sqlx::query(
        "UPDATE drills SET status=?, isolation=?, evidence_json=?, host_json=?, error=?, completed_at=? WHERE id=?",
    )
    .bind(&d.status)
    .bind(&d.isolation)
    .bind(serde_json::to_string(&d.evidence)?)
    .bind(d.host.as_ref().map(serde_json::to_string).transpose()?)
    .bind(&d.error)
    .bind(&d.completed_at)
    .bind(&d.id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn get_drill(db: &SqlitePool, id: &str) -> Result<Option<Drill>> {
    let row = sqlx::query("SELECT * FROM drills WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|r| Drill {
        id: r.get("id"),
        status: r.get("status"),
        isolation: r.get("isolation"),
        evidence: r
            .get::<Option<String>, _>("evidence_json")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        host: r
            .get::<Option<String>, _>("host_json")
            .and_then(|s| serde_json::from_str(&s).ok()),
        error: r.get("error"),
        created_at: r.get("created_at"),
        completed_at: r.get("completed_at"),
    }))
}

pub async fn artifact_dir(db_root: &std::path::Path, run_id: &str) -> Result<std::path::PathBuf> {
    let dir = db_root.join("artifacts").join(run_id);
    std::fs::create_dir_all(&dir).context("create artifact dir")?;
    Ok(dir)
}

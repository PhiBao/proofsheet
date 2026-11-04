use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub async fn init(database_url: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS datasets (
            id TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            bytes INTEGER NOT NULL,
            rows INTEGER NOT NULL,
            cols INTEGER NOT NULL,
            profile_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            dataset_id TEXT NOT NULL,
            question TEXT NOT NULL,
            mode TEXT NOT NULL,
            status TEXT NOT NULL,
            attempt INTEGER NOT NULL DEFAULT 0,
            plan_json TEXT,
            definition_json TEXT,
            code TEXT,
            stdout TEXT NOT NULL DEFAULT '',
            stderr TEXT NOT NULL DEFAULT '',
            exit_code INTEGER,
            duration_ms INTEGER,
            sandbox_id TEXT,
            isolation TEXT,
            oom_killed INTEGER NOT NULL DEFAULT 0,
            metrics_json TEXT,
            chart_data_json TEXT,
            chart_audit_json TEXT,
            consensus_json TEXT,
            comparison_json TEXT,
            template TEXT,
            notes_markdown TEXT,
            claims_json TEXT,
            clean_report_json TEXT,
            log_json TEXT NOT NULL DEFAULT '[]',
            error TEXT,
            replayed_from TEXT,
            created_at TEXT NOT NULL,
            completed_at TEXT
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS drills (
            id TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            isolation TEXT,
            evidence_json TEXT NOT NULL DEFAULT '[]',
            host_json TEXT,
            error TEXT,
            created_at TEXT NOT NULL,
            completed_at TEXT
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_runs_dataset ON runs(dataset_id);")
        .execute(&pool)
        .await?;

    // Lightweight forward migration for databases created by earlier builds.
    let _ = sqlx::query("ALTER TABLE runs ADD COLUMN replayed_from TEXT")
        .execute(&pool)
        .await;
    for ddl in [
        "ALTER TABLE runs ADD COLUMN definition_json TEXT",
        "ALTER TABLE runs ADD COLUMN chart_data_json TEXT",
        "ALTER TABLE runs ADD COLUMN chart_audit_json TEXT",
        "ALTER TABLE runs ADD COLUMN consensus_json TEXT",
        "ALTER TABLE runs ADD COLUMN comparison_json TEXT",
    ] {
        let _ = sqlx::query(ddl).execute(&pool).await;
    }

    Ok(pool)
}

use crate::agent;
use crate::app::AppState;
use crate::drill;
use crate::models::*;
use crate::profile::profile_csv;
use crate::store;
use crate::verify;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

const SAMPLES: &[(&str, &str, &str, &str)] = &[
    (
        "orders",
        "northwind_orders_messy.csv",
        "Northwind orders — 486 rows of real-world dirt",
        include_str!("../fixtures/northwind_orders_messy.csv"),
    ),
    (
        "saas",
        "atlas_saas_metrics_messy.csv",
        "Atlas SaaS metrics — 24 months, dual currencies",
        include_str!("../fixtures/atlas_saas_metrics_messy.csv"),
    ),
    (
        "orders_next",
        "northwind_orders_next_month.csv",
        "Northwind orders — next month's export (same schema, new dirt)",
        include_str!("../fixtures/northwind_orders_next_month.csv"),
    ),
];

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": self.message, "status": self.status.as_u16() })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{e:#}"),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("database error: {e}"),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("json error: {e}"),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("io error: {e}"),
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/proof", get(proof))
        .route("/samples", get(list_samples))
        .route("/samples/{key}", post(use_sample))
        .route("/datasets", post(upload_dataset))
        .route("/datasets/{id}", get(get_dataset))
        .route("/datasets/{id}/ask", post(ask))
        .route("/datasets/{id}/plan", post(plan_dataset))
        .route("/datasets/{id}/run", post(run_dataset))
        .route("/runs/{id}", get(get_run))
        .route("/runs/{id}/report", get(get_report))
        .route("/runs/{id}/replay", post(replay))
        .route("/runs/{id}/challenge", post(challenge))
        .route("/runs/{id}/bundle", get(bundle))
        .route("/artifacts/{id}/{file}", get(artifact))
        .route("/drills", post(start_drill))
        .route("/drills/latest", get(latest_drill))
        .route("/drills/{id}", get(get_drill))
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let (passed, total) = verify::selftest();
    let canary = tokio::fs::read(&state.canary_path).await.ok();
    let canary_prefix = canary.map(|b| hex::encode(Sha256::digest(&b))[..12].to_string());
    Json(json!({
        "ok": state.sandbox_health.ready,
        "product": "Proofsheet",
        "region": state.cfg.region,
        "hostname": state.hostname,
        "uptime_seconds": state.uptime_seconds(),
        "sandbox": {
            "ready": state.sandbox_health.ready,
            "isolation": state.sandbox_health.isolation,
            "label": state.sandbox_health.label,
            "image": state.sandbox_health.image,
            "detail": state.sandbox_health.detail,
        },
        "models": {
            "provider": "Vultr Serverless Inference",
            "base_url": state.cfg.inference_base_url,
            "plan": state.cfg.model_plan,
            "code": state.cfg.model_code,
            "repair": state.cfg.model_repair,
        },
        "verifier": { "passed": passed, "total": total },
        "canary": { "present": canary_prefix.is_some(), "sha256_prefix": canary_prefix },
    }))
}

async fn list_samples() -> Json<Value> {
    let items: Vec<Value> = SAMPLES
        .iter()
        .map(|(key, filename, description, body)| match profile_csv(body.as_bytes()) {
            Ok(p) => json!({
                "key": key,
                "filename": filename,
                "description": description,
                "rows": p.rows,
                "cols": p.columns.len(),
                "duplicate_rows": p.duplicate_rows,
                "issues": p.issues.len(),
                "dirty_score": p.dirty_score,
            }),
            Err(_) => json!({ "key": key, "filename": filename, "description": description }),
        })
        .collect();
    Json(json!({ "samples": items }))
}

async fn use_sample(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let sample = SAMPLES
        .iter()
        .find(|(k, _, _, _)| *k == key)
        .ok_or_else(|| ApiError::not_found("unknown sample"))?;
    let dataset = create_dataset(&state, sample.1, sample.3.as_bytes()).await?;
    Ok(Json(json!({ "dataset": dataset })))
}

async fn upload_dataset(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad(format!("invalid upload: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "upload.csv".to_string());
        let bytes = field
            .bytes()
            .await
            .map_err(|e| ApiError::bad(format!("could not read upload: {e}")))?;
        if bytes.len() > state.cfg.max_upload_bytes {
            return Err(ApiError::bad(format!(
                "file is larger than the {} MB limit",
                state.cfg.max_upload_bytes / 1024 / 1024
            )));
        }
        if bytes.len() < 8 {
            return Err(ApiError::bad("that file is empty"));
        }
        let dataset = create_dataset(&state, &filename, &bytes).await?;
        return Ok(Json(json!({ "dataset": dataset })));
    }
    Err(ApiError::bad("no file field in the upload"))
}

async fn create_dataset(
    state: &AppState,
    filename: &str,
    bytes: &[u8],
) -> Result<Dataset, ApiError> {
    let profile = profile_csv(bytes)
        .map_err(|e| ApiError::bad(format!("could not read that CSV: {e}")))?;
    let id = Uuid::new_v4().to_string();
    let sha256 = hex::encode(Sha256::digest(bytes));
    let path = state.cfg.data_dir.join("uploads").join(format!("{id}.csv"));
    tokio::fs::write(&path, bytes).await?;

    let dataset = Dataset {
        id,
        filename: filename.to_string(),
        sha256,
        bytes: bytes.len() as i64,
        rows: profile.rows,
        cols: profile.columns.len() as i64,
        profile,
        created_at: Utc::now().to_rfc3339(),
    };
    store::insert_dataset(&state.db, &dataset).await?;
    Ok(dataset)
}

async fn get_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let dataset = store::get_dataset(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    let runs = store::list_runs_for_dataset(&state.db, &id).await?;
    Ok(Json(json!({ "dataset": dataset, "runs": runs })))
}

#[derive(Deserialize)]
struct AskBody {
    question: String,
}

async fn ask(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AskBody>,
) -> Result<Json<Value>, ApiError> {
    let question = body.question.trim().to_string();
    if question.len() < 3 {
        return Err(ApiError::bad("ask a real question"));
    }
    if question.len() > 1200 {
        return Err(ApiError::bad("question is too long"));
    }
    let dataset = store::get_dataset(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    if !state.sandbox_health.ready {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "the sandbox is not ready on this host; refusing to run untrusted code".into(),
        });
    }

    let run = Run {
        id: Uuid::new_v4().to_string(),
        dataset_id: dataset.id,
        question,
        mode: "ask".into(),
        status: "queued".into(),
        attempt: 0,
        plan: None,
        definition: None,
        code: None,
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        duration_ms: None,
        sandbox_id: None,
        isolation: None,
        oom_killed: false,
        metrics: vec![],
        chart_data: None,
        chart_audit: None,
        consensus: None,
        comparison: None,
        template: None,
        notes_markdown: None,
        claims: vec![],
        clean_report: None,
        log: vec![],
        error: None,
        replayed_from: None,
        created_at: Utc::now().to_rfc3339(),
        completed_at: None,
    };
    store::insert_run(&state.db, &run).await?;
    let run_id = run.id.clone();
    let st = state.clone();
    tokio::spawn(async move { agent::execute_ask(st, run_id).await; });
    Ok(Json(json!({ "run_id": run.id })))
}

#[derive(Deserialize)]
struct PlanBody {
    question: String,
}

#[derive(Deserialize)]
struct RunBody {
    question: String,
    plan: Plan,
    definition: Definition,
}

fn clean_question(q: &str) -> Result<String, ApiError> {
    let q = q.trim().to_string();
    if q.len() < 3 {
        return Err(ApiError::bad("ask a real question"));
    }
    if q.len() > 1200 {
        return Err(ApiError::bad("question is too long"));
    }
    Ok(q)
}

async fn plan_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PlanBody>,
) -> Result<Json<Value>, ApiError> {
    let question = clean_question(&body.question)?;
    let dataset = store::get_dataset(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    let (plan, definition) = agent::generate_plan(&state, &dataset, &question).await?;
    Ok(Json(json!({ "plan": plan, "definition": definition, "question": question })))
}

async fn run_dataset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RunBody>,
) -> Result<Json<Value>, ApiError> {
    let question = clean_question(&body.question)?;
    if !state.sandbox_health.ready {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "the sandbox is not ready on this host; refusing to run untrusted code".into(),
        });
    }
    let dataset = store::get_dataset(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    let mut plan = body.plan;
    plan.metrics.retain(|m| !m.id.trim().is_empty());
    if plan.metrics.is_empty() {
        return Err(ApiError::bad("the plan needs at least one metric"));
    }
    if plan.metrics.len() > 6 {
        plan.metrics.truncate(6);
    }
    let definition_json = serde_json::to_value(&body.definition)?;
    let definition = agent::parse_definition(Some(&definition_json));

    let run = Run {
        id: Uuid::new_v4().to_string(),
        dataset_id: dataset.id,
        question,
        mode: "ask".into(),
        status: "queued".into(),
        attempt: 0,
        plan: Some(plan),
        definition: Some(definition),
        code: None,
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        duration_ms: None,
        sandbox_id: None,
        isolation: None,
        oom_killed: false,
        metrics: vec![],
        chart_data: None,
        chart_audit: None,
        consensus: None,
        comparison: None,
        template: None,
        notes_markdown: None,
        claims: vec![],
        clean_report: None,
        log: vec![],
        error: None,
        replayed_from: None,
        created_at: Utc::now().to_rfc3339(),
        completed_at: None,
    };
    store::insert_run(&state.db, &run).await?;
    let run_id = run.id.clone();
    let st = state.clone();
    tokio::spawn(async move { agent::execute_prepared(st, run_id).await; });
    Ok(Json(json!({ "run_id": run.id })))
}

async fn challenge(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let run = store::get_run(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    if run.status != "complete" {
        return Err(ApiError::conflict("only a complete report can be challenged"));
    }
    let template = run
        .template
        .clone()
        .ok_or_else(|| ApiError::conflict("this run has no note to challenge"))?;
    let tampered = format!(
        "{}\n\n## Bonus claim\nThe analysis proves revenue grew 47% this quarter and the top segment was {{{{m:made_up_metric}}}}.",
        template
    );
    let flags = verify::verify_template(&tampered, &run.metrics);
    let (markdown, _) = verify::render(&tampered, &run.metrics);
    Ok(Json(json!({
        "markdown": markdown,
        "flags": flags,
        "passed": flags.is_empty(),
        "note": "This is the real verifier applied to a deliberately tampered note. The production report is untouched."
    })))
}

fn integrity_hash(run: &Run, dataset: &Dataset) -> String {
    let mut h = Sha256::new();
    h.update(dataset.sha256.as_bytes());
    h.update(
        run.definition
            .as_ref()
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .unwrap_or_default()
            .as_bytes(),
    );
    h.update(run.code.as_deref().unwrap_or("").as_bytes());
    h.update(serde_json::to_string(&run.metrics).unwrap_or_default().as_bytes());
    h.update(
        run.chart_data
            .as_ref()
            .map(|c| serde_json::to_string(c).unwrap_or_default())
            .unwrap_or_default()
            .as_bytes(),
    );
    h.update(
        run.consensus
            .as_ref()
            .map(|c| c.status.as_str())
            .unwrap_or("")
            .as_bytes(),
    );
    hex::encode(h.finalize())
}

fn build_bundle(run: &Run, dataset: &Dataset) -> Value {
    let code_hash = hex::encode(Sha256::digest(run.code.as_deref().unwrap_or("").as_bytes()));
    let ledger_hash = hex::encode(Sha256::digest(
        serde_json::to_string(&run.metrics).unwrap_or_default().as_bytes(),
    ));
    let definition_hash = hex::encode(Sha256::digest(
        serde_json::to_string(&run.definition)
            .unwrap_or_default()
            .as_bytes(),
    ));
    let chart_hash = hex::encode(Sha256::digest(
        serde_json::to_string(&run.chart_data)
            .unwrap_or_default()
            .as_bytes(),
    ));
    json!({
        "format": "proofsheet.assurance.v1",
        "generated_at": Utc::now().to_rfc3339(),
        "product": "Proofsheet",
        "run": {
            "id": run.id,
            "question": run.question,
            "mode": run.mode,
            "created_at": run.created_at,
            "completed_at": run.completed_at,
            "attempt": run.attempt,
        },
        "dataset": {
            "filename": dataset.filename,
            "sha256": dataset.sha256,
            "rows": dataset.rows,
            "cols": dataset.cols,
        },
        "definition": run.definition,
        "plan": run.plan,
        "execution": {
            "code": run.code,
            "code_sha256": code_hash,
            "sandbox_id": run.sandbox_id,
            "isolation": run.isolation,
            "exit_code": run.exit_code,
            "duration_ms": run.duration_ms,
            "oom_killed": run.oom_killed,
            "teardown": "container force-removed after the task",
        },
        "ledger": run.metrics,
        "ledger_sha256": ledger_hash,
        "chart_data": run.chart_data,
        "chart_data_sha256": chart_hash,
        "chart_audit": run.chart_audit,
        "claims": run.claims,
        "clean_report": run.clean_report,
        "consensus": run.consensus,
        "comparison": run.comparison,
        "definition_sha256": definition_hash,
        "hashes": {
            "dataset_sha256": dataset.sha256,
            "code_sha256": code_hash,
            "ledger_sha256": ledger_hash,
            "definition_sha256": definition_hash,
            "chart_data_sha256": chart_hash,
            "integrity_hash": integrity_hash(run, dataset),
        },
        "integrity_hash": integrity_hash(run, dataset),
    })
}

async fn bundle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let run = store::get_run(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    let dataset = store::get_dataset(&state.db, &run.dataset_id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    let bytes = serde_json::to_vec_pretty(&build_bundle(&run, &dataset))?;
    let mut response = Response::new(axum::body::Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"proofsheet-{}.json\"", run.id))
            .unwrap_or(HeaderValue::from_static("attachment")),
    );
    Ok(response)
}

async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let run = store::get_run(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    Ok(Json(json!({ "run": run })))
}

async fn get_report(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let run = store::get_run(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("run not found"))?;
    if run.status != "complete" {
        return Err(ApiError::conflict("this run is not complete yet"));
    }
    let dataset = store::get_dataset(&state.db, &run.dataset_id)
        .await?
        .ok_or_else(|| ApiError::not_found("dataset not found"))?;
    let integrity = integrity_hash(&run, &dataset);
    let template = run.template.clone().unwrap_or_default();
    let (markdown, receipted) = verify::render(&template, &run.metrics);
    let claims: Vec<RenderedClaim> = run
        .metrics
        .iter()
        .map(|m| RenderedClaim {
            id: m.id.clone(),
            label: m.label.clone(),
            kind: m.kind.clone(),
            value: m.value,
            text: m.text.clone(),
            formatted: verify::format_value(m),
        })
        .collect();
    let report = Report {
        run: run.clone(),
        dataset,
        markdown,
        claims,
        receipted,
        unreceipted: run.claims.len(),
        replayed_from: run.replayed_from.clone(),
        integrity_hash: integrity,
    };
    Ok(Json(json!({ "report": report })))
}

#[derive(Deserialize)]
struct ReplayBody {
    dataset_id: Option<String>,
    /// Convenience for the demo: replay onto a bundled sample instead of an upload.
    sample_key: Option<String>,
}

async fn replay(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ReplayBody>,
) -> Result<Json<Value>, ApiError> {
    let source = store::get_run(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("source run not found"))?;
    if source.status != "complete" {
        return Err(ApiError::conflict("only a complete run can be replayed"));
    }
    let dataset = match (body.dataset_id.as_deref(), body.sample_key.as_deref()) {
        (Some(id), _) => store::get_dataset(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::not_found("replay dataset not found"))?,
        (None, Some(key)) => {
            let sample = SAMPLES
                .iter()
                .find(|(k, _, _, _)| *k == key)
                .ok_or_else(|| ApiError::not_found("unknown sample"))?;
            create_dataset(&state, sample.1, sample.3.as_bytes()).await?
        }
        _ => return Err(ApiError::bad("provide dataset_id or sample_key")),
    };

    let run = Run {
        id: Uuid::new_v4().to_string(),
        dataset_id: dataset.id,
        question: format!("Deterministic replay of: {}", source.question),
        mode: "replay".into(),
        status: "queued".into(),
        attempt: 0,
        plan: source.plan.clone(),
        definition: source.definition.clone(),
        code: None,
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        duration_ms: None,
        sandbox_id: None,
        isolation: None,
        oom_killed: false,
        metrics: vec![],
        chart_data: None,
        chart_audit: None,
        consensus: None,
        comparison: None,
        template: None,
        notes_markdown: None,
        claims: vec![],
        clean_report: None,
        log: vec![],
        error: None,
        replayed_from: Some(source.id.clone()),
        created_at: Utc::now().to_rfc3339(),
        completed_at: None,
    };
    store::insert_run(&state.db, &run).await?;
    let run_id = run.id.clone();
    let st = state.clone();
    tokio::spawn(async move { agent::execute_replay(st, run_id).await; });
    Ok(Json(json!({ "run_id": run.id, "replayed_from": source.id })))
}

async fn artifact(
    State(state): State<Arc<AppState>>,
    Path((id, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    const ALLOWED: &[(&str, &str)] = &[
        ("cleaned.csv", "text/csv; charset=utf-8"),
        ("chart.png", "image/png"),
        ("metrics.json", "application/json"),
        ("notes.md", "text/markdown; charset=utf-8"),
        ("clean_report.json", "application/json"),
        ("script.py", "text/x-python; charset=utf-8"),
    ];
    let (name, mime) = ALLOWED
        .iter()
        .find(|(n, _)| *n == file)
        .ok_or_else(|| ApiError::not_found("artifact not found"))?;
    if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err(ApiError::bad("invalid run id"));
    }
    let path = state.cfg.data_dir.join("artifacts").join(&id).join(name);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::not_found("artifact not found"))?;

    let disposition = if *name == "cleaned.csv" || *name == "script.py" {
        format!("attachment; filename=\"{name}\"")
    } else {
        format!("inline; filename=\"{name}\"")
    };
    let mut response = Response::new(axum::body::Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap_or(HeaderValue::from_static("inline")),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=300"),
    );
    Ok(response)
}

async fn start_drill(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    if !state.sandbox_health.ready {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "the sandbox is not ready on this host".into(),
        });
    }
    let drill = Drill {
        id: Uuid::new_v4().to_string(),
        status: "queued".into(),
        isolation: Some(state.sandbox.isolation()),
        evidence: vec![],
        host: None,
        error: None,
        created_at: Utc::now().to_rfc3339(),
        completed_at: None,
    };
    store::insert_drill(&state.db, &drill).await?;
    let id = drill.id.clone();
    let st = state.clone();
    tokio::spawn(async move { drill::execute_drill(st, id).await; });
    Ok(Json(json!({ "drill_id": drill.id })))
}

async fn get_drill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let drill = store::get_drill(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("drill not found"))?;
    Ok(Json(json!({ "drill": drill })))
}

async fn latest_drill(State(state): State<Arc<AppState>>) -> Result<Json<Value>, ApiError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM drills WHERE status = 'complete' ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(&state.db)
            .await?;
    match row {
        Some((id,)) => {
            let drill = store::get_drill(&state.db, &id).await?;
            Ok(Json(json!({ "drill": drill })))
        }
        None => Ok(Json(json!({ "drill": null }))),
    }
}

async fn proof(State(state): State<Arc<AppState>>) -> Json<Value> {
    let (passed, total) = verify::selftest();
    let golden = crate::eval::run();
    let golden_passed = golden.iter().filter(|c| c.passed).count();

    // Live fixture assertions: these are computed on every request, not hard-coded.
    let mut evals = Vec::new();
    for (key, filename, _, body) in SAMPLES {
        match profile_csv(body.as_bytes()) {
            Ok(p) => {
                let dirt_classes: std::collections::HashSet<&str> = p
                    .issues
                    .iter()
                    .map(|i| i.kind.as_str())
                    .filter(|k| !["clean_file", "note", "constant", "empty_column"].contains(k))
                    .collect();
                evals.push(json!({
                    "id": key,
                    "filename": filename,
                    "checks": [
                        { "name": "profile parses", "passed": true },
                        { "name": "duplicate rows detected", "passed": p.duplicate_rows > 0, "detail": p.duplicate_rows },
                        { "name": "multiple dirt classes detected", "passed": dirt_classes.len() >= 2, "detail": dirt_classes.len() },
                        { "name": "dirty score above ten", "passed": p.dirty_score > 10, "detail": p.dirty_score },
                    ],
                    "passed": p.duplicate_rows > 0 && dirt_classes.len() >= 2 && p.dirty_score > 10,
                }));
            }
            Err(e) => evals.push(json!({
                "id": key,
                "filename": filename,
                "passed": false,
                "error": e.to_string(),
            })),
        }
    }

    let last_drill: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drills WHERE status = 'complete' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    let drill = match last_drill {
        Some((id,)) => store::get_drill(&state.db, &id).await.ok().flatten(),
        None => None,
    };

    Json(json!({
        "verifier": { "passed": passed, "total": total },
        "golden": { "passed": golden_passed, "total": golden.len(), "checks": golden },
        "fixtures": evals,
        "containment": drill.map(|d| json!({
            "id": d.id,
            "isolation": d.isolation,
            "steps": d.evidence.len(),
            "host": d.host,
        })),
        "sandbox": {
            "ready": state.sandbox_health.ready,
            "isolation": state.sandbox_health.isolation,
            "label": state.sandbox_health.label,
        }
    }))
}

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = ASSETS.get_file(path) {
        return serve_file(file, path);
    }
    // SPA fallback
    if let Some(file) = ASSETS.get_file("index.html") {
        return serve_file(file, "index.html");
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn serve_file(file: &include_dir::File<'_>, path: &str) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path == "index.html" || path.ends_with("sw.js") {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    };
    let mut response = Response::new(axum::body::Body::from(file.contents().to_vec()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

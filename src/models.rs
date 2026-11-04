use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Profile {
    pub delimiter: String,
    pub rows: i64,
    pub duplicate_rows: i64,
    pub issues: Vec<Issue>,
    pub columns: Vec<ColumnProfile>,
    pub preview: Vec<Vec<String>>,
    /// 0 = clean, 100 = filthy. Deterministic heuristic used for the UI gauge.
    pub dirty_score: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ColumnProfile {
    pub name: String,
    /// number | currency | percent | date | text | empty
    pub kind: String,
    pub non_null: i64,
    pub nulls: i64,
    pub null_pct: f64,
    pub unique: i64,
    pub sample: Vec<String>,
    /// Distinct values for low-cardinality text columns; used to drive canonicalization.
    #[serde(default)]
    pub values: Vec<String>,
    /// Date formats detected in this column, in canonical precedence order.
    #[serde(default)]
    pub date_formats: Vec<String>,
    /// Non-empty values that a correct cleaning pass must normalize (computed by Rust).
    #[serde(default)]
    pub dirty_values: i64,
    pub issues: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Issue {
    /// error | warn | info
    pub severity: String,
    pub kind: String,
    pub column: Option<String>,
    pub count: i64,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dataset {
    pub id: String,
    pub filename: String,
    pub sha256: String,
    pub bytes: i64,
    pub rows: i64,
    pub cols: i64,
    pub profile: Profile,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Plan {
    pub title: String,
    pub summary: String,
    pub cleaning: Vec<String>,
    pub analysis: Vec<String>,
    pub metrics: Vec<MetricSpec>,
    pub chart: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricSpec {
    pub id: String,
    pub label: String,
    /// currency | number | percent | count
    pub kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Metric {
    pub id: String,
    pub label: String,
    /// currency | number | percent | count | text
    pub kind: String,
    #[serde(default)]
    pub value: f64,
    /// Set when kind == "text": a categorical fact such as the top country name.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CleanReport {
    pub rows_in: i64,
    pub rows_out: i64,
    pub duplicates_removed: i64,
    pub columns: Vec<ColumnFix>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColumnFix {
    pub name: String,
    pub before: String,
    pub after: String,
    pub fixes: i64,
    pub rule: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClaimFlag {
    /// literal_number | unknown_metric | empty_template | no_metrics
    pub kind: String,
    pub detail: String,
    pub token: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LogLine {
    pub at: String,
    pub level: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Definition {
    pub summary: String,
    pub assumptions: Vec<Assumption>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Assumption {
    pub key: String,
    pub label: String,
    pub value: String,
    /// bool | enum | text
    pub kind: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub why: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChartData {
    #[serde(default)]
    pub metric_id: String,
    /// sum | trend | breakdown
    #[serde(default)]
    pub aggregation: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub series: Vec<ChartPoint>,
    #[serde(default)]
    pub top_category: Option<String>,
    #[serde(default)]
    pub rows_used: i64,
    #[serde(default)]
    pub applied_assumptions: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvalCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChartAudit {
    pub passed: bool,
    pub checks: Vec<EvalCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Consensus {
    /// agree | disagree | checker_failed | skipped
    pub status: String,
    pub model: String,
    pub checks: Vec<EvalCheck>,
    pub deltas: Vec<MetricDelta>,
    pub checker_sandbox: Option<String>,
    pub checker_exit: Option<i64>,
    pub checker_code: Option<String>,
    pub checker_stderr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetricDelta {
    pub id: String,
    pub label: String,
    pub primary: String,
    pub checker: String,
    pub diff_pct: f64,
    pub agree: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Comparison {
    pub source_run_id: String,
    pub source_created_at: String,
    #[serde(default)]
    pub previous_rows: Option<i64>,
    #[serde(default)]
    pub current_rows: Option<i64>,
    pub metrics: Vec<ComparisonMetric>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ComparisonMetric {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub previous: f64,
    pub current: f64,
    pub delta: f64,
    pub delta_pct: Option<f64>,
    pub previous_text: Option<String>,
    pub current_text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Run {
    pub id: String,
    pub dataset_id: String,
    pub question: String,
    /// ask | replay
    pub mode: String,
    pub status: String,
    pub attempt: i64,
    pub plan: Option<Plan>,
    pub definition: Option<Definition>,
    pub code: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub sandbox_id: Option<String>,
    pub isolation: Option<String>,
    pub oom_killed: bool,
    pub metrics: Vec<Metric>,
    pub chart_data: Option<ChartData>,
    pub chart_audit: Option<ChartAudit>,
    pub consensus: Option<Consensus>,
    pub comparison: Option<Comparison>,
    pub template: Option<String>,
    pub notes_markdown: Option<String>,
    pub claims: Vec<ClaimFlag>,
    pub clean_report: Option<CleanReport>,
    pub log: Vec<LogLine>,
    pub error: Option<String>,
    /// Set on deterministic replays: the original run whose script was reused.
    pub replayed_from: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RenderedClaim {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub value: f64,
    pub text: Option<String>,
    pub formatted: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Report {
    pub run: Run,
    pub dataset: Dataset,
    pub markdown: String,
    pub claims: Vec<RenderedClaim>,
    pub receipted: usize,
    pub unreceipted: usize,
    /// When this report came from a deterministic replay, the original run it replayed.
    pub replayed_from: Option<String>,
    /// Hash over the dataset, definition, code, ledger and chart data.
    pub integrity_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DrillEvidence {
    pub id: String,
    pub title: String,
    pub command: String,
    /// absorbed | blocked | contained | failed
    pub verdict: String,
    pub exit_code: Option<i64>,
    pub duration_ms: i64,
    pub output: String,
    pub note: String,
    pub sandbox_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HostProof {
    pub canary_intact: bool,
    pub canary_sha256: String,
    pub uptime_seconds: u64,
    pub app_pid: u32,
    pub hostname: String,
    pub teardown: String,
    pub app_still_serving: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Drill {
    pub id: String,
    pub status: String,
    pub isolation: Option<String>,
    pub evidence: Vec<DrillEvidence>,
    pub host: Option<HostProof>,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RunSummary {
    pub id: String,
    pub dataset_id: String,
    pub question: String,
    pub mode: String,
    pub status: String,
    pub attempt: i64,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

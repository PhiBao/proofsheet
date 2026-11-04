export interface Issue {
  severity: "error" | "warn" | "info" | string;
  kind: string;
  column: string | null;
  count: number;
  detail: string;
}

export interface ColumnProfile {
  name: string;
  kind: string;
  non_null: number;
  nulls: number;
  null_pct: number;
  unique: number;
  sample: string[];
  values: string[];
  issues: string[];
}

export interface Profile {
  delimiter: string;
  rows: number;
  duplicate_rows: number;
  issues: Issue[];
  columns: ColumnProfile[];
  preview: string[][];
  dirty_score: number;
}

export interface Dataset {
  id: string;
  filename: string;
  sha256: string;
  bytes: number;
  rows: number;
  cols: number;
  profile: Profile;
  created_at: string;
}

export interface MetricSpec {
  id: string;
  label: string;
  kind: string;
}

export interface Assumption {
  key: string;
  label: string;
  value: string;
  kind: "bool" | "enum" | "text" | string;
  options: string[];
  why: string;
}

export interface Definition {
  summary: string;
  assumptions: Assumption[];
}

export interface Plan {
  title: string;
  summary: string;
  cleaning: string[];
  analysis: string[];
  metrics: MetricSpec[];
  chart: string;
}

export interface ChartPoint {
  label: string;
  value: number;
}

export interface ChartData {
  metric_id: string;
  aggregation: string;
  unit: string;
  series: ChartPoint[];
  top_category?: string | null;
  rows_used: number;
  applied_assumptions: string[];
}

export interface EvalCheck {
  name: string;
  passed: boolean;
  detail: string;
}

export interface ChartAudit {
  passed: boolean;
  checks: EvalCheck[];
}

export interface MetricDelta {
  id: string;
  label: string;
  primary: string;
  checker: string;
  diff_pct: number;
  agree: boolean;
}

export interface Consensus {
  status: string;
  model: string;
  checks: EvalCheck[];
  deltas: MetricDelta[];
  checker_sandbox?: string | null;
  checker_exit?: number | null;
  checker_code?: string | null;
  checker_stderr: string;
}

export interface ComparisonMetric {
  id: string;
  label: string;
  kind: string;
  previous: number;
  current: number;
  delta: number;
  delta_pct?: number | null;
  previous_text?: string | null;
  current_text?: string | null;
}

export interface Comparison {
  source_run_id: string;
  source_created_at: string;
  previous_rows?: number | null;
  current_rows?: number | null;
  metrics: ComparisonMetric[];
}

export interface Metric {
  id: string;
  label: string;
  kind: string;
  value: number;
  text?: string | null;
}

export interface ClaimFlag {
  kind: string;
  detail: string;
  token?: string | null;
}

export interface LogLine {
  at: string;
  level: string;
  text: string;
}

export interface CleanReport {
  rows_in: number;
  rows_out: number;
  duplicates_removed: number;
  columns: {
    name: string;
    before: string;
    after: string;
    fixes: number;
    rule: string;
  }[];
}

export interface Run {
  id: string;
  dataset_id: string;
  question: string;
  mode: string;
  status: string;
  attempt: number;
  plan: Plan | null;
  definition: Definition | null;
  code: string | null;
  stdout: string;
  stderr: string;
  exit_code: number | null;
  duration_ms: number | null;
  sandbox_id: string | null;
  isolation: string | null;
  oom_killed: boolean;
  metrics: Metric[];
  chart_data: ChartData | null;
  chart_audit: ChartAudit | null;
  consensus: Consensus | null;
  comparison: Comparison | null;
  template: string | null;
  notes_markdown: string | null;
  claims: ClaimFlag[];
  clean_report: CleanReport | null;
  log: LogLine[];
  error: string | null;
  replayed_from: string | null;
  created_at: string;
  completed_at: string | null;
}

export interface RenderedClaim {
  id: string;
  label: string;
  kind: string;
  value: number;
  text?: string | null;
  formatted: string;
}

export interface Report {
  run: Run;
  dataset: Dataset;
  markdown: string;
  claims: RenderedClaim[];
  receipted: number;
  unreceipted: number;
  replayed_from: string | null;
  integrity_hash: string;
}

export interface DrillEvidence {
  id: string;
  title: string;
  command: string;
  verdict: string;
  exit_code: number | null;
  duration_ms: number;
  output: string;
  note: string;
  sandbox_id: string;
}

export interface HostProof {
  canary_intact: boolean;
  canary_sha256: string;
  uptime_seconds: number;
  app_pid: number;
  hostname: string;
  teardown: string;
  app_still_serving: boolean;
}

export interface Drill {
  id: string;
  status: string;
  isolation: string | null;
  evidence: DrillEvidence[];
  host: HostProof | null;
  error: string | null;
  created_at: string;
  completed_at: string | null;
}

export interface Health {
  ok: boolean;
  product: string;
  region: string;
  hostname: string;
  uptime_seconds: number;
  sandbox: {
    ready: boolean;
    isolation: string;
    label: string;
    image: string;
    detail: string;
  };
  models: {
    provider: string;
    base_url: string;
    plan: string;
    code: string;
    repair: string;
  };
  verifier: { passed: number; total: number };
  canary: { present: boolean; sha256_prefix: string | null };
}

export interface Sample {
  key: string;
  filename: string;
  description: string;
  rows?: number;
  cols?: number;
  duplicate_rows?: number;
  issues?: number;
  dirty_score?: number;
}

export interface RunSummary {
  id: string;
  dataset_id: string;
  question: string;
  mode: string;
  status: string;
  attempt: number;
  error: string | null;
  created_at: string;
  completed_at: string | null;
}

export interface Proof {
  verifier: { passed: number; total: number };
  golden: {
    passed: number;
    total: number;
    checks: { name: string; passed: boolean; detail: string }[];
  };
  fixtures: {
    id: string;
    filename: string;
    passed: boolean;
    checks?: { name: string; passed: boolean; detail?: number }[];
  }[];
  containment: {
    id: string;
    isolation: string;
    steps: number;
    host: HostProof | null;
  } | null;
  sandbox: { ready: boolean; isolation: string; label: string };
}

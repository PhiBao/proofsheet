import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, ApiError } from "../lib/api";
import type { Dataset, Definition, Plan, Report, Run, RunSummary } from "../lib/types";
import { formatBytes, formatDate, isolationLabel, shortHash } from "../lib/format";
import { ProfileTable } from "../components/ProfileTable";
import { RunCard } from "../components/RunCard";
import { ReportView } from "../components/ReportView";
import { DefinitionPanel } from "../components/DefinitionPanel";

export function DatasetPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [dataset, setDataset] = useState<Dataset | null>(null);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [error, setError] = useState("");
  const [question, setQuestion] = useState("");
  const [askError, setAskError] = useState("");
  const [busy, setBusy] = useState(false);
  const [planning, setPlanning] = useState(false);
  const [preview, setPreview] = useState<{
    plan: Plan;
    definition: Definition;
    question: string;
  } | null>(null);
  const [planError, setPlanError] = useState("");
  const [runId, setRunId] = useState<string | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const runRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!id) return;
    setDataset(null);
    setReport(null);
    setRunId(null);
    api
      .dataset(id)
      .then((r) => {
        setDataset(r.dataset);
        setRuns(r.runs);
      })
      .catch((e) => setError(e instanceof ApiError ? e.message : "could not load dataset"));
  }, [id]);

  const suggestions = useMemo(() => {
    if (!dataset) return [];
    const cols = new Set(dataset.profile.columns.map((c) => c.name.toLowerCase()));
    if (cols.has("mrr") || cols.has("churn_rate")) {
      return [
        "Which region has the highest total MRR, and how is churn trending?",
        "Clean the export. What is the month-over-month growth in MRR?",
        "Which region churns the most, and how many customers does that represent?",
      ];
    }
    if (cols.has("unit_price") || cols.has("quantity")) {
      return [
        "Clean this export. What was monthly net revenue, and which country generated the most?",
        "What were the top five products by revenue after cleaning?",
        "How many duplicate or invalid orders were removed, and what did they cost?",
      ];
    }
    return [
      "Clean this file, then summarize the most important trend.",
      "Which category has the highest total, and what share does it represent?",
      "What data quality problems did you fix, and how did they change the answer?",
    ];
  }, [dataset]);

  const planAnalysis = async () => {
    if (!dataset || question.trim().length < 3) return;
    setPlanning(true);
    setAskError("");
    setPlanError("");
    try {
      const { plan, definition, question: cleaned } = await api.plan(
        dataset.id,
        question.trim()
      );
      setPreview({ plan, definition, question: cleaned });
    } catch (e) {
      setAskError(e instanceof ApiError ? e.message : "could not plan the analysis");
    } finally {
      setPlanning(false);
    }
  };

  const approveAndRun = async () => {
    if (!dataset || !preview) return;
    setBusy(true);
    setPlanError("");
    try {
      const { run_id } = await api.startRun(dataset.id, {
        question: preview.question,
        plan: preview.plan,
        definition: preview.definition,
      });
      setPreview(null);
      setRunId(run_id);
      setTimeout(() => runRef.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 60);
    } catch (e) {
      setPlanError(e instanceof ApiError ? e.message : "could not start the analysis");
    } finally {
      setBusy(false);
    }
  };

  const onComplete = async (run: Run) => {
    if (run.status !== "complete") return;
    try {
      const { report } = await api.report(run.id);
      setReport(report);
      setRuns((prev) => [
        {
          id: run.id,
          dataset_id: run.dataset_id,
          question: run.question,
          mode: run.mode,
          status: run.status,
          attempt: run.attempt,
          error: run.error,
          created_at: run.created_at,
          completed_at: run.completed_at,
        },
        ...prev.filter((r) => r.id !== run.id),
      ]);
    } catch {
      /* the report endpoint may lag a beat; keep the run card */
    }
  };

  if (error) {
    return (
      <div className="wrap">
        <div className="page-head">
          <div className="eyebrow">Error</div>
          <h1 className="page-title">We could not open that dataset.</h1>
          <p className="page-sub">{error}</p>
        </div>
        <div className="section">
          <Link to="/" className="btn">
            Back to intake
          </Link>
        </div>
      </div>
    );
  }

  if (!dataset) {
    return (
      <div className="wrap">
        <div className="page-head">
          <div className="eyebrow">Loading</div>
          <h1 className="page-title">Profiling…</h1>
        </div>
      </div>
    );
  }

  if (report) {
    return (
      <div className="wrap">
        <div className="page-head" style={{ paddingBottom: 18 }}>
          <div className="breadcrumb">
            <Link to="/">Intake</Link>
            <span>/</span>
            <Link to={`/d/${dataset.id}`} onClick={() => setReport(null)}>
              {dataset.filename}
            </Link>
            <span>/</span>
            <span>Report</span>
          </div>
          <div style={{ display: "flex", gap: 16, marginTop: 12, flexWrap: "wrap" }}>
            <Link to={`/r/${report.run.id}`} className="link">
              Open standalone report ↗
            </Link>
            <button
              className="link"
              style={{ background: "none", border: 0, padding: 0, cursor: "pointer" }}
              onClick={() => {
                setReport(null);
                setRunId(null);
              }}
            >
              New analysis on this file
            </button>
          </div>
        </div>
        <ReportView report={report} />
      </div>
    );
  }

  const latest = runs.find((r) => r.status === "complete");

  return (
    <div className="wrap">
      <div className="page-head">
        <div className="breadcrumb">
          <Link to="/">Intake</Link>
          <span>/</span>
          <span>Dataset</span>
        </div>
        <h1 className="page-title">{dataset.filename}</h1>
        <div className="page-sub">
          {dataset.rows} rows · {dataset.cols} columns · sha256 {shortHash(dataset.sha256, 14)} ·{" "}
          {formatBytes(dataset.bytes)}
        </div>
      </div>

      <div className="dataset-grid">
        <div style={{ minWidth: 0 }}>
          <div className="panel" ref={runRef} style={{ marginTop: 0 }}>
            <div className="panel-head">
              <div className="panel-title">Ask the analyst</div>
              <div className="eyebrow">Plan → definition → execution → receipt</div>
            </div>
            {!preview && (
              <>
                <div className="ask">
                  <textarea
                    value={question}
                    onChange={(e) => setQuestion(e.target.value)}
                    placeholder="What was monthly net revenue, and which country generated the most?"
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) planAnalysis();
                    }}
                  />
                  <div className="ask-foot">
                    <span className="ask-hint">⌘/Ctrl + Enter · max 1,200 chars</span>
                    <button
                      className="btn btn-primary"
                      onClick={planAnalysis}
                      disabled={planning || question.trim().length < 3}
                    >
                      {planning ? "Planning…" : "Plan analysis"}
                    </button>
                  </div>
                </div>
                {askError && <div className="upload-error">{askError}</div>}
                <div className="suggestions">
                  {suggestions.map((s) => (
                    <button key={s} className="chip-btn" onClick={() => setQuestion(s)}>
                      {s}
                    </button>
                  ))}
                </div>
              </>
            )}
            {preview && (
              <DefinitionPanel
                plan={preview.plan}
                definition={preview.definition}
                onChange={(definition) => setPreview({ ...preview, definition })}
                onRun={approveAndRun}
                onCancel={() => setPreview(null)}
                busy={busy}
                error={planError}
              />
            )}
          </div>

          <div style={{ marginTop: 30 }}>
            <ProfileTable profile={dataset.profile} />
          </div>

          {runId && <RunCard runId={runId} onComplete={onComplete} />}
        </div>

        <aside className="side">
          <div className="side-block">
            <div className="eyebrow">Dirt score</div>
            <div className="gauge">
              <div className="gauge-bar">
                <i
                  className={dataset.profile.dirty_score < 25 ? "low" : ""}
                  style={{ width: `${Math.max(2, dataset.profile.dirty_score)}%` }}
                />
              </div>
              <div className="gauge-label">
                <span>clean</span>
                <span>{dataset.profile.dirty_score}/100</span>
              </div>
            </div>
            <div className="muted small" style={{ marginTop: 10 }}>
              {dataset.profile.duplicate_rows} exact duplicate rows ·{" "}
              {dataset.profile.issues.filter((i) => i.severity !== "info").length} issues ·{" "}
              {dataset.profile.delimiter} delimited
            </div>
          </div>

          <div className="side-block">
            <div className="eyebrow">Provenance</div>
            <dl className="kv" style={{ marginTop: 8, gridTemplateColumns: "70px 1fr" }}>
              <dt>File</dt>
              <dd>{dataset.filename}</dd>
              <dt>Hash</dt>
              <dd>{shortHash(dataset.sha256, 16)}</dd>
              <dt>Uploaded</dt>
              <dd>{formatDate(dataset.created_at)}</dd>
              <dt>Sandbox</dt>
              <dd>gVisor · network off</dd>
            </dl>
          </div>

          <div className="side-block">
            <div className="eyebrow">Latest report</div>
            {latest ? (
              <div style={{ marginTop: 8 }}>
                <div className="small">{latest.question}</div>
                <Link to={`/r/${latest.id}`} className="link" style={{ display: "inline-block", marginTop: 8 }}>
                  Open report ↗
                </Link>
              </div>
            ) : (
              <div className="muted small" style={{ marginTop: 8 }}>
                No completed analysis yet. Ask a question to create one.
              </div>
            )}
          </div>

          <div className="side-block">
            <div className="eyebrow">What runs</div>
            <div className="muted small" style={{ marginTop: 8, lineHeight: 1.55 }}>
              A Rust planner call, a pandas script, a disposable sandbox, an independent checker, and a
              claim verifier. Model prompts receive column shapes and redacted examples, not your
              raw rows. The full file stays on this VM and inside the sandbox.
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

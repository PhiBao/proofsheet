import { useEffect, useRef, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { Run } from "../lib/types";
import { elapsed, isActive, statusLabel } from "../lib/format";

export function RunCard({
  runId,
  onComplete,
}: {
  runId: string;
  onComplete?: (run: Run) => void;
}) {
  const [run, setRun] = useState<Run | null>(null);
  const [error, setError] = useState("");
  const [, setTick] = useState(0);
  const doneRef = useRef(false);
  const completeRef = useRef(onComplete);
  completeRef.current = onComplete;
  const logRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    doneRef.current = false;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    setRun(null);
    setError("");

    const poll = async () => {
      try {
        const { run } = await api.run(runId);
        if (cancelled) return;
        setRun(run);
        if (run.status === "complete" || run.status === "failed") {
          if (!doneRef.current) {
            doneRef.current = true;
            completeRef.current?.(run);
          }
          return;
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof ApiError ? e.message : "could not load this run");
      }
      if (!cancelled) timer = setTimeout(poll, 900);
    };

    poll();
    const clock = setInterval(() => setTick((t) => t + 1), 1000);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      clearInterval(clock);
    };
  }, [runId]);

  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [run?.log.length]);

  if (!run) {
    return (
      <div className="run-card">
        <div className="run-head">
          <div className="run-status">
            <span className="dot pulse" /> Starting
          </div>
          <div className="run-meta">opening run {runId.slice(0, 8)}</div>
        </div>
        <div className="run-body muted small">Waiting for the control plane…</div>
        {error && <div className="run-body notice bad">{error}</div>}
      </div>
    );
  }

  const active = isActive(run.status);
  const failed = run.status === "failed";
  const logs = run.log.slice(-40);

  return (
    <div className="run-card">
      <div className="run-head">
        <div className="run-status">
          <span className={`dot ${active ? "pulse" : ""} ${failed ? "warn" : ""}`} />
          {statusLabel(run.status)}
        </div>
        <div className="run-meta">
          attempt {run.attempt || 1}
          {active ? ` · ${elapsed(run.created_at)}` : run.duration_ms ? ` · sandbox ${(run.duration_ms / 1000).toFixed(2)}s` : ""}
        </div>
      </div>

      {run.plan && (
        <div className="run-body" style={{ paddingBottom: 0 }}>
          <div className="eyebrow">Plan</div>
          <div style={{ fontFamily: "var(--serif)", fontSize: 22, marginTop: 4, letterSpacing: "-0.01em" }}>
            {run.plan.title}
          </div>
          <div className="muted small" style={{ marginTop: 4, maxWidth: "68ch" }}>
            {run.plan.summary}
          </div>
          <div className="metric-specs">
            {run.plan.metrics.map((m) => (
              <span className="spec" key={m.id}>
                {m.id} · {m.kind}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="run-log" ref={logRef}>
        {logs.length === 0 && <div className="log-line"><span /><span /><span>queued…</span></div>}
        {logs.map((line, i) => (
          <div className="log-line" key={i}>
            <span className="log-time">{line.at.slice(11, 19)}</span>
            <span className={`log-level ${line.level}`}>{line.level}</span>
            <span>{line.text}</span>
          </div>
        ))}
      </div>

      {run.status === "failed" && (
        <div className="run-body">
          <div className="notice bad" style={{ marginTop: 0 }}>
            <div className="notice-title">Run failed after {run.attempt} attempt(s)</div>
            {run.error || "Unknown error"}
          </div>
        </div>
      )}

      {run.status === "complete" && (
        <div className="run-body">
          <div className="notice good" style={{ marginTop: 0 }}>
            <div className="notice-title">Complete</div>
            {run.metrics.length} claims receipted · {run.claims.length} unverified · report below.
          </div>
        </div>
      )}
    </div>
  );
}

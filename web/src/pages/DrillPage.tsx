import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, ApiError } from "../lib/api";
import type { Drill } from "../lib/types";
import { formatDate } from "../lib/format";

const STEPS = [
  ["01", "Destructive filesystem command", "rm -rf --no-preserve-root / against a read-only root"],
  ["02", "Process exhaustion", "fork until the kernel refuses; pids capped at 192"],
  ["03", "Runaway CPU loop", "killed at the wall-clock cap"],
  ["04", "Credential exfiltration", "dump env and common secret paths"],
  ["05", "Outbound network", "connect to the public internet with --network none"],
  ["06", "Host filesystem probe", "read the host canary from inside the sandbox"],
];

export function DrillPage() {
  const [drill, setDrill] = useState<Drill | null>(null);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({});

  useEffect(() => {
    api
      .latestDrill()
      .then(({ drill }) => {
        if (drill) setDrill(drill);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!drill) return;
    if (drill.status === "complete" || drill.status === "failed") return;
    const id = drill.id;
    const timer = setInterval(async () => {
      try {
        const { drill: next } = await api.drill(id);
        setDrill(next);
      } catch {
        /* keep polling */
      }
    }, 1000);
    return () => clearInterval(timer);
  }, [drill?.id, drill?.status]);

  const start = async () => {
    setStarting(true);
    setError("");
    setOpen({});
    try {
      const { drill_id } = await api.startDrill();
      const { drill } = await api.drill(drill_id);
      setDrill(drill);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "could not start the drill");
    } finally {
      setStarting(false);
    }
  };

  const running = drill?.status === "running" || drill?.status === "queued";
  const complete = drill?.status === "complete";
  const host = drill?.host;

  return (
    <div className="wrap">
      <div className="drill-head">
        <div className="eyebrow">Containment drill · Blast Radius Zero</div>
        <h1 className="drill-title">
          Blast radius <em>zero.</em>
        </h1>
        <p className="drill-lede">
          An agent that cannot survive hostile code is a demo. This page runs the hostile code for
          real: every step below gets a fresh sandbox on this Vultr VM, no network, no credentials,
          a read-only root and hard caps. Then it checks whether the host noticed.
        </p>
        <div style={{ marginTop: 22, display: "flex", gap: 12, flexWrap: "wrap" }}>
          <button className="btn btn-primary" onClick={start} disabled={starting || running}>
            {starting ? "Starting…" : running ? "Drill running…" : complete ? "Run it again" : "Run the drill"}
          </button>
          {drill && (
            <span className="hero-note" style={{ alignSelf: "center" }}>
              {drill.isolation ? `${drill.isolation} · ` : ""}
              {formatDate(drill.created_at)}
            </span>
          )}
          {!drill && (
            <span className="hero-note" style={{ alignSelf: "center" }}>
              6 steps · ~30 seconds · not a recording
            </span>
          )}
        </div>
        {error && <div className="upload-error">{error}</div>}
      </div>

      {!drill && (
        <>
          <div className="stats" style={{ marginTop: 28 }}>
            <div className="stat">
              <div className="stat-value">06</div>
              <div className="stat-label">live attack steps</div>
            </div>
            <div className="stat">
              <div className="stat-value">0</div>
              <div className="stat-label">secrets in the sandbox</div>
            </div>
            <div className="stat">
              <div className="stat-value">1</div>
              <div className="stat-label">host canary, checked before and after</div>
            </div>
          </div>
          <div className="section">
            <div className="section-head">
              <h2 className="section-title">What will run</h2>
              <span className="eyebrow">Each in a fresh container</span>
            </div>
            <div className="evidence">
              {STEPS.map(([n, title, detail]) => (
                <div className="ev-row" key={n}>
                  <div>
                    <div className="eyebrow">{n}</div>
                    <div className="ev-title">{title}</div>
                  </div>
                  <div className="muted small">{detail}</div>
                  <span className="verdict pending">queued</span>
                </div>
              ))}
            </div>
          </div>
        </>
      )}

      {drill && (
        <>
          {host && (
            <div className="host-proof">
              <div className="host-cell">
                <div className="host-value">
                  <span className={`dot ${host.canary_intact ? "" : "warn"}`} />
                  {host.canary_intact ? "intact" : "changed"}
                </div>
                <div className="host-label">host canary</div>
              </div>
              <div className="host-cell">
                <div className="host-value">{host.uptime_seconds.toLocaleString()}s</div>
                <div className="host-label">host uptime</div>
              </div>
              <div className="host-cell">
                <div className="host-value">{host.app_pid}</div>
                <div className="host-label">control-plane pid</div>
              </div>
              <div className="host-cell">
                <div className="host-value">{host.teardown.startsWith("every") ? "removed" : "check"}</div>
                <div className="host-label">sandbox teardown</div>
              </div>
            </div>
          )}

          <div className="section-head" style={{ marginTop: 8 }}>
            <h2 className="section-title">Evidence</h2>
            <span className="eyebrow">
              {drill.evidence.length}/6 steps · {drill.status}
              {running ? " · live" : ""}
            </span>
          </div>

          <div className="evidence">
            {drill.evidence.map((e) => (
              <div className="ev-row" key={e.id}>
                <div>
                  <div className="ev-title">{e.title}</div>
                  <div className="ev-cmd">{e.command}</div>
                  <div className="ev-meta">
                    {e.sandbox_id} · {e.duration_ms} ms · exit{" "}
                    {e.exit_code == null ? "killed" : e.exit_code}
                  </div>
                </div>
                <div>
                  <div
                    className={`ev-output ${open[e.id] ? "open" : ""}`}
                    onClick={() => setOpen((prev) => ({ ...prev, [e.id]: !prev[e.id] }))}
                    title="Click to expand"
                  >
                    {e.output || "— no output —"}
                  </div>
                  <div className="ev-note">{e.note}</div>
                </div>
                <div style={{ textAlign: "right" }}>
                  <span className={`verdict ${e.verdict}`}>{e.verdict}</span>
                </div>
              </div>
            ))}
            {running && (
              <div className="ev-row">
                <div className="ev-title muted">Running…</div>
                <div className="muted small">
                  Step {drill.evidence.length + 1} of 6. Each step creates a new sandbox and destroys
                  it afterwards.
                </div>
                <span className="verdict pending">live</span>
              </div>
            )}
          </div>

          {complete && host && (
            <>
              <div className="notice good" style={{ marginTop: 22 }}>
                <div className="notice-title">Host unaffected</div>
                The canary file is byte-identical before and after all six steps. Every sandbox was
                force-removed. The control plane answered this page while the attacks were running.
              </div>
              <div className="section">
                <div className="section-head">
                  <h2 className="section-title">What each step proves</h2>
                </div>
                <div className="steps" style={{ gridTemplateColumns: "1fr 1fr" }}>
                  <div className="step" style={{ borderLeft: 0, paddingLeft: 0 }}>
                    <h3 style={{ fontSize: 19 }}>Process isolation</h3>
                    <p>
                      Execution never touches the app process or the host filesystem. The read-only
                      root and the bind-mounted workdir absorb the destructive command.
                    </p>
                    <h3 style={{ fontSize: 19, marginTop: 16 }}>Secret hygiene</h3>
                    <p>
                      The sandbox environment contains no Vultr key, inference key or application
                      secret. The scan sees only sandbox-local variables.
                    </p>
                  </div>
                  <div className="step">
                    <h3 style={{ fontSize: 19 }}>Resource limits</h3>
                    <p>
                      Time, memory and pid caps turn an infinite loop or a fork storm into a
                      bounded event with an exit code, not an outage.
                    </p>
                    <h3 style={{ fontSize: 19, marginTop: 16 }}>Lifecycle discipline</h3>
                    <p>
                      Each step starts a fresh sandbox and force-removes it. Nothing carries over
                      between tasks, and nothing is reused.
                    </p>
                  </div>
                </div>
              </div>
            </>
          )}

          {drill.status === "failed" && (
            <div className="notice bad" style={{ marginTop: 20 }}>
              <div className="notice-title">Drill failed</div>
              {drill.error || "One or more containment checks did not hold."}
            </div>
          )}
        </>
      )}

      <div className="section">
        <div className="containment-strip">
          <div>
            <h4>How is this wired?</h4>
            <div className="muted small" style={{ marginTop: 4 }}>
              A Rust control plane on the Vultr VM dispatches each step to a disposable container
              with a read-only root, a network namespace, no secrets, and time/memory/pid caps.
            </div>
          </div>
          <Link to="/how" className="btn">
            Read the architecture
          </Link>
        </div>
      </div>
    </div>
  );
}

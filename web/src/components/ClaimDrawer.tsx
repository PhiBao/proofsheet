import { useEffect, useState } from "react";
import type { RenderedClaim, Run } from "../lib/types";
import { duration, formatDate, isolationLabel } from "../lib/format";

export function ClaimDrawer({
  claim,
  run,
  onClose,
}: {
  claim: RenderedClaim;
  run: Run;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"code" | "output" | "run">("code");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard unavailable */
    }
  };

  const codeLines = (run.code || "").split("\n");

  return (
    <>
      <div className="overlay" onClick={onClose} />
      <aside className="drawer" role="dialog" aria-modal="true" aria-label="Metric receipt">
        <div className="drawer-head">
          <div>
            <div className="eyebrow">Receipt · metric</div>
            <div className="drawer-title">{claim.label}</div>
            <div className="receipt-id" style={{ marginTop: 4 }}>
              {claim.id}
            </div>
          </div>
          <button className="drawer-close" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="drawer-body">
          <div className="notice good" style={{ marginTop: 0 }}>
            <div className="notice-title">Provenance</div>
            This value was read from the metric ledger written by the sandboxed script.
            The prose above cannot contain a number that is not in this ledger.
          </div>
          <dl className="kv" style={{ marginTop: 16 }}>
            <dt>Value</dt>
            <dd style={{ fontSize: 15 }}>{claim.formatted}</dd>
            <dt>Kind</dt>
            <dd>{claim.kind}</dd>
            <dt>Run</dt>
            <dd>{run.id}</dd>
            <dt>Isolation</dt>
            <dd>{isolationLabel(run.isolation)}</dd>
            <dt>Sandbox</dt>
            <dd>{run.sandbox_id || "—"}</dd>
            <dt>Exit / time</dt>
            <dd>
              {run.exit_code ?? "—"} · {duration(run.duration_ms)}
            </dd>
            <dt>Teardown</dt>
            <dd>container force-removed after the task</dd>
            <dt>Started</dt>
            <dd>{formatDate(run.created_at)}</dd>
          </dl>

          <div className="tabs">
            <button className={`tab ${tab === "code" ? "active" : ""}`} onClick={() => setTab("code")}>
              Executed python
            </button>
            <button className={`tab ${tab === "output" ? "active" : ""}`} onClick={() => setTab("output")}>
              stdout / stderr
            </button>
            <button className={`tab ${tab === "run" ? "active" : ""}`} onClick={() => setTab("run")}>
              Log
            </button>
          </div>

          {tab === "code" && (
            <>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 8 }}>
                <span className="receipt-id">script.py · {codeLines.length} lines</span>
                <button className="receipt-btn" onClick={() => copy(run.code || "")}>
                  {copied ? "Copied" : "Copy"}
                </button>
              </div>
              <pre className="code">
                {codeLines.map((line, i) => (
                  <div key={i}>
                    <span className="ln">{i + 1}</span>
                    {line}
                  </div>
                ))}
              </pre>
            </>
          )}

          {tab === "output" && (
            <>
              <div className="eyebrow" style={{ marginBottom: 6 }}>
                stdout
              </div>
              <pre className="code" style={{ maxHeight: 200 }}>
                {run.stdout?.trim() || "— no stdout —"}
              </pre>
              <div className="eyebrow" style={{ margin: "14px 0 6px" }}>
                stderr
              </div>
              <pre className="code" style={{ maxHeight: 160 }}>
                {run.stderr?.trim() || "— no stderr —"}
              </pre>
            </>
          )}

          {tab === "run" && (
            <pre className="code" style={{ maxHeight: 420 }}>
              {run.log.map((l, i) => (
                <div key={i}>
                  <span className="ln">{i + 1}</span>
                  {l.at.slice(11, 19)} [{l.level}] {l.text}
                </div>
              ))}
            </pre>
          )}
        </div>
      </aside>
    </>
  );
}

import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { artifactUrl, api, ApiError } from "../lib/api";
import type { Report, RenderedClaim } from "../lib/types";
import { duration, formatDate, isolationLabel, shortHash } from "../lib/format";
import { Markdown } from "./Markdown";
import { ClaimDrawer } from "./ClaimDrawer";

const SECTIONS = [
  ["changed", "What changed"],
  ["exhibit", "Exhibit"],
  ["note", "Analyst note"],
  ["receipts", "Receipts"],
  ["containment", "Containment"],
] as const;

export function ReportView({ report }: { report: Report }) {
  const [claim, setClaim] = useState<RenderedClaim | null>(null);
  const [active, setActive] = useState<string>("changed");
  const [copied, setCopied] = useState(false);
  const [replayBusy, setReplayBusy] = useState(false);
  const [replayError, setReplayError] = useState("");
  const fileRef = useRef<HTMLInputElement | null>(null);
  const navigate = useNavigate();

  const { run, dataset } = report;
  const clean = run.clean_report;
  const claimMap = useMemo(() => {
    const map: Record<string, RenderedClaim> = {};
    for (const c of report.claims) map[c.id] = c;
    return map;
  }, [report.claims]);

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
        if (visible?.target?.id) setActive(visible.target.id);
      },
      { rootMargin: "-15% 0px -65% 0px", threshold: [0, 0.2, 0.6] }
    );
    for (const [id] of SECTIONS) {
      const el = document.getElementById(id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, [report.run.id]);

  const copyLink = async () => {
    try {
      await navigator.clipboard.writeText(window.location.href);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch {
      /* ignore */
    }
  };

  const replaySample = async () => {
    setReplayBusy(true);
    setReplayError("");
    try {
      const { run_id } = await api.replay(run.id, { sample_key: "orders_next" });
      navigate(`/r/${run_id}`);
    } catch (e) {
      setReplayError(e instanceof ApiError ? e.message : "replay failed");
    } finally {
      setReplayBusy(false);
    }
  };

  const replayFile = async (file?: File) => {
    if (!file) return;
    setReplayBusy(true);
    setReplayError("");
    try {
      const { dataset: next } = await api.upload(file);
      const { run_id } = await api.replay(run.id, { dataset_id: next.id });
      navigate(`/r/${run_id}`);
    } catch (e) {
      setReplayError(e instanceof ApiError ? e.message : "replay failed");
    } finally {
      setReplayBusy(false);
    }
  };

  const sampleReplayable = dataset.filename.includes("northwind");

  return (
    <div className="report-grid">
      <aside className="rail">
        <div className="rail-title">Contents</div>
        <ol>
          {SECTIONS.map(([id, label], i) => (
            <li key={id}>
              <a href={`#${id}`} className={active === id ? "active" : ""}>
                <span>{String(i + 1).padStart(2, "0")}</span>
                <span>{label}</span>
              </a>
            </li>
          ))}
        </ol>
        <div style={{ marginTop: 18 }}>
          <button className="receipt-btn" onClick={copyLink}>
            {copied ? "Link copied" : "Copy share link"}
          </button>
        </div>
      </aside>

      <article style={{ minWidth: 0 }}>
        <header className="report-head">
          <div className="report-kicker">
            <span className="eyebrow">Verified report</span>
            {run.mode === "replay" && <span className="spec">deterministic replay</span>}
            {report.replayed_from && (
              <Link to={`/r/${report.replayed_from}`} className="eyebrow" style={{ textDecoration: "underline" }}>
                original run
              </Link>
            )}
          </div>
          <h1 className="report-title">{run.plan?.title || "Analysis"}</h1>
          <p className="report-summary">{run.plan?.summary}</p>
          <div className="byline">
            <span>
              <b>{dataset.filename}</b>
            </span>
            <span>sha256 {shortHash(dataset.sha256, 12)}</span>
            <span>{clean ? `${clean.rows_in} → ${clean.rows_out} rows` : `${dataset.rows} rows`}</span>
            <span>
              run <b>{shortHash(run.id, 8)}</b>
            </span>
            <span>{isolationLabel(run.isolation)}</span>
            <span>{formatDate(run.created_at)}</span>
          </div>
          <div className={`stamp ${report.unreceipted > 0 ? "bad" : ""}`}>
            {report.unreceipted > 0 ? (
              <>
                {report.unreceipted} unverified
                <br />
                claim(s)
              </>
            ) : (
              <>
                All claims
                <br />
                receipted
              </>
            )}
          </div>
        </header>

        {report.replayed_from && (
          <div className="notice good" style={{ marginTop: 18 }}>
            <div className="notice-title">No model in this run</div>
            The exact pipeline from run {shortHash(report.replayed_from, 8)} was re-executed on a new
            file. The script and the narrative template were reused; only the data changed.
          </div>
        )}

        <section className="report-section" id="changed">
          <div className="rs-head">
            <span className="rs-num">01</span>
            <h2 className="rs-title">What changed</h2>
            <span className="rs-note">audited in Rust, not by the model</span>
          </div>

          {clean ? (
            <>
              <div className="clean-grid">
                <div className="clean-cell">
                  <div className="clean-value">{clean.rows_in}</div>
                  <div className="clean-label">rows in</div>
                </div>
                <div className="clean-cell">
                  <div className="clean-value">{clean.rows_out}</div>
                  <div className="clean-label">rows out</div>
                </div>
                <div className="clean-cell">
                  <div className="clean-value">{clean.duplicates_removed}</div>
                  <div className="clean-label">duplicates removed</div>
                </div>
                <div className="clean-cell">
                  <div className="clean-value">{clean.columns.length}</div>
                  <div className="clean-label">columns repaired</div>
                </div>
              </div>
              {clean.columns.length > 0 ? (
                <table className="tbl">
                  <thead>
                    <tr>
                      <th>Column</th>
                      <th>Before</th>
                      <th>After</th>
                      <th className="num">Fixes</th>
                      <th>Rule</th>
                    </tr>
                  </thead>
                  <tbody>
                    {clean.columns.map((c) => (
                      <tr key={c.name}>
                        <td className="col-name">{c.name}</td>
                        <td className="before">{c.before}</td>
                        <td>
                          <span className="fix-arrow">→</span> {c.after}
                        </td>
                        <td className="num">{c.fixes}</td>
                        <td className="muted small">{c.rule}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : (
                <div className="muted small">No column-level repairs were required.</div>
              )}
              {clean.rows_in - clean.rows_out > 0 && (
                <div className="eyebrow" style={{ marginTop: 10 }}>
                  Rows removed: {clean.rows_in - clean.rows_out} · of which{" "}
                  {clean.duplicates_removed} exact duplicate(s)
                </div>
              )}
              <div className="eyebrow" style={{ marginTop: 12 }}>
                Residual-dirt audit: categorical aliases, numeric parse rate and date parse rate are
                re-checked independently after the script runs.
              </div>
            </>
          ) : (
            <div className="muted">This run did not emit a cleaning report.</div>
          )}
        </section>

        <section className="report-section" id="exhibit">
          <div className="rs-head">
            <span className="rs-num">02</span>
            <h2 className="rs-title">Exhibit</h2>
            <span className="rs-note">
              {run.exit_code === 0 ? `exit 0 · ${duration(run.duration_ms)}` : "no chart"}
            </span>
          </div>
          <figure className="exhibit" style={{ margin: 0 }}>
            <img
              src={artifactUrl(run.id, "chart.png")}
              alt={`Chart generated by the sandboxed analysis: ${run.plan?.chart || "result"}`}
              loading="lazy"
            />
            <figcaption className="caption">
              <span>{run.plan?.chart || "Generated by the executed script."}</span>
              <span>
                <a href={artifactUrl(run.id, "chart.png")} download>
                  PNG ↓
                </a>
                {" · "}
                <a href={artifactUrl(run.id, "cleaned.csv")}>cleaned.csv ↓</a>
              </span>
            </figcaption>
          </figure>
        </section>

        <section className="report-section" id="note">
          <div className="rs-head">
            <span className="rs-num">03</span>
            <h2 className="rs-title">Analyst note</h2>
            <span className="rs-note">
              {report.receipted} receipts · {report.unreceipted} unverified
            </span>
          </div>

          <Markdown markdown={report.markdown} onClaim={(id) => setClaim(claimMap[id] || null)} />

          {report.unreceipted > 0 ? (
            <div className="notice bad">
              <div className="notice-title">The verifier held back the stamp</div>
              <ul style={{ margin: "4px 0 0", paddingLeft: 18 }}>
                {run.claims.map((f, i) => (
                  <li key={i}>{f.detail}</li>
                ))}
              </ul>
            </div>
          ) : (
            <div className="notice good">
              <div className="notice-title">Claim audit passed</div>
              Every number and every name in the note resolves to a value produced by the sandboxed
              script. Click any underlined value to open its receipt.
            </div>
          )}
        </section>

        <section className="report-section" id="receipts">
          <div className="rs-head">
            <span className="rs-num">04</span>
            <h2 className="rs-title">Receipts</h2>
            <span className="rs-note">the ledger the prose is not allowed to exceed</span>
          </div>
          <table className="tbl receipt-table">
            <thead>
              <tr>
                <th>Metric</th>
                <th>Value</th>
                <th>Kind</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {report.claims.map((c) => (
                <tr key={c.id}>
                  <td>
                    <div>{c.label}</div>
                    <div className="receipt-id">{c.id}</div>
                  </td>
                  <td className="receipt-value">{c.formatted}</td>
                  <td className="kind">{c.kind}</td>
                  <td style={{ textAlign: "right" }}>
                    <button className="receipt-btn" onClick={() => setClaim(c)}>
                      Open receipt
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="eyebrow" style={{ marginTop: 12 }}>
            Run {run.id} · sandbox {run.sandbox_id || "—"} · {isolationLabel(run.isolation)} ·{" "}
            {run.exit_code === 0 ? "exit 0" : `exit ${run.exit_code}`} · container removed
          </div>
        </section>

        <section className="report-section" id="containment">
          <div className="rs-head">
            <span className="rs-num">05</span>
            <h2 className="rs-title">Containment</h2>
          </div>
          <div className="containment-strip">
            <div>
              <h4>{isolationLabel(run.isolation)} · network off · root filesystem read-only</h4>
              <div className="muted small" style={{ marginTop: 4 }}>
                No credentials were mounted into the sandbox. The container was force-removed after
                the task. {run.oom_killed ? "The run hit the memory cap. " : ""}
                {run.attempt > 1 ? `The agent repaired itself ${run.attempt - 1} time(s), visibly.` : ""}
              </div>
            </div>
            <Link to="/drill" className="btn">
              Run the containment drill
            </Link>
          </div>

          {run.mode === "ask" && (
            <div className="replay" style={{ marginTop: 16 }}>
              <div>
                <h4>Replay this pipeline on next month's export</h4>
                <p>
                  No model in the loop. The same script and the same note template are re-executed on
                  a new file, then re-audited. This is the retention loop: build the pipeline once,
                  reuse it every month.
                </p>
              </div>
              <div className="replay-actions">
                {sampleReplayable && (
                  <button className="btn" onClick={replaySample} disabled={replayBusy}>
                    {replayBusy ? "Replaying…" : "Use sample export"}
                  </button>
                )}
                <button className="btn btn-primary" onClick={() => fileRef.current?.click()} disabled={replayBusy}>
                  Upload CSV
                </button>
                <input
                  ref={fileRef}
                  type="file"
                  accept=".csv,.tsv,.txt,text/csv"
                  style={{ display: "none" }}
                  onChange={(e) => replayFile(e.target.files?.[0])}
                />
              </div>
            </div>
          )}
          {replayError && <div className="notice bad">{replayError}</div>}
        </section>
      </article>

      {claim && <ClaimDrawer claim={claim} run={run} onClose={() => setClaim(null)} />}
    </div>
  );
}

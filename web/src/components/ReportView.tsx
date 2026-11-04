import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { artifactUrl, api, ApiError, bundleUrl } from "../lib/api";
import type { ClaimFlag, Report, RenderedClaim } from "../lib/types";
import {
  duration,
  formatDate,
  formatMetricValue,
  isolationLabel,
  shortHash,
} from "../lib/format";
import { Markdown } from "./Markdown";
import { ClaimDrawer } from "./ClaimDrawer";

export function ReportView({ report }: { report: Report }) {
  const [claim, setClaim] = useState<RenderedClaim | null>(null);
  const [copied, setCopied] = useState(false);
  const [replayBusy, setReplayBusy] = useState(false);
  const [replayError, setReplayError] = useState("");
  const [challenge, setChallenge] = useState<{
    markdown: string;
    flags: ClaimFlag[];
    note: string;
  } | null>(null);
  const [challengeBusy, setChallengeBusy] = useState(false);
  const [challengeError, setChallengeError] = useState("");
  const [active, setActive] = useState("definition");
  const fileRef = useRef<HTMLInputElement | null>(null);
  const navigate = useNavigate();

  const { run, dataset } = report;
  const clean = run.clean_report;
  const definition = run.definition;
  const consensus = run.consensus;
  const chartAudit = run.chart_audit;
  const comparison = run.comparison;

  const sections = useMemo(() => {
    const list: { id: string; label: string }[] = [
      { id: "definition", label: "Definition" },
    ];
    if (comparison) list.push({ id: "comparison", label: "Compared" });
    list.push(
      { id: "changed", label: "What changed" },
      { id: "exhibit", label: "Exhibit" },
      { id: "note", label: "Analyst note" },
      { id: "receipts", label: "Receipts" }
    );
    if (consensus) list.push({ id: "check", label: "Independent check" });
    list.push({ id: "containment", label: "Containment" });
    return list;
  }, [consensus, comparison]);

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
    for (const s of sections) {
      const el = document.getElementById(s.id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, [run.id, sections]);

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

  const runChallenge = async () => {
    setChallengeBusy(true);
    setChallengeError("");
    try {
      const res = await api.challenge(run.id);
      setChallenge(res);
    } catch (e) {
      setChallengeError(e instanceof ApiError ? e.message : "challenge failed");
    } finally {
      setChallengeBusy(false);
    }
  };

  const sampleReplayable = dataset.filename.includes("northwind");
  const sectionNo = (id: string) =>
    String(sections.findIndex((s) => s.id === id) + 1).padStart(2, "0");
  const stampBad = report.unreceipted > 0 || consensus?.status === "disagree";
  const stampWarn = !stampBad && consensus?.status === "checker_failed";
  const stampText = stampBad
    ? report.unreceipted > 0
      ? `${report.unreceipted} unverified\nclaim(s)`
      : "Executions\ndisagree"
    : stampWarn
      ? "Claims receipted\nsingle execution"
      : consensus?.status === "agree"
        ? "All claims receipted\n2 executions agree"
        : "All claims\nreceipted";

  return (
    <div className="report-grid">
      <aside className="rail">
        <div className="rail-title">Contents</div>
        <ol>
          {sections.map((s, i) => (
            <li key={s.id}>
              <a href={`#${s.id}`} className={active === s.id ? "active" : ""}>
                <span>{String(i + 1).padStart(2, "0")}</span>
                <span>{s.label}</span>
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
              <Link
                to={`/r/${report.replayed_from}`}
                className="eyebrow"
                style={{ textDecoration: "underline" }}
              >
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
          <div className="trust-row">
            <span className={`trust ${report.unreceipted === 0 ? "good" : "bad"}`}>
              {report.receipted} claims receipted
            </span>
            <span
              className={`trust ${
                consensus?.status === "agree"
                  ? "good"
                  : consensus?.status === "disagree"
                    ? "bad"
                    : consensus?.status === "checker_failed"
                      ? "warn"
                      : ""
              }`}
            >
              {consensus?.status === "agree"
                ? "2 executions agree"
                : consensus?.status === "disagree"
                  ? "executions disagree"
                  : consensus?.status === "checker_failed"
                    ? "single execution"
                    : run.mode === "replay"
                      ? "deterministic replay"
                      : "no independent check"}
            </span>
            <span className={`trust ${chartAudit?.passed ? "good" : "warn"}`}>
              {chartAudit?.passed ? "chart data checked" : "chart data unverified"}
            </span>
            {definition && <span className="trust">definition locked</span>}
          </div>
          <div className={`stamp ${stampBad ? "bad" : stampWarn ? "warn" : ""}`}>
            {stampText.split("\n").map((line) => (
              <span key={line}>
                {line}
                <br />
              </span>
            ))}
          </div>
        </header>

        {report.replayed_from && (
          <div className="notice good" style={{ marginTop: 18 }}>
            <div className="notice-title">No model in this run</div>
            The exact pipeline from run {shortHash(report.replayed_from, 8)} was re-executed on a new
            file. The script, the definition and the note template were reused; only the data changed.
          </div>
        )}

        <section className="report-section" id="definition">
          <div className="rs-head">
            <span className="rs-num">{sectionNo("definition")}</span>
            <h2 className="rs-title">Definition</h2>
            <span className="rs-note">approved before execution · stored with the run</span>
          </div>
          {definition ? (
            <>
              <div className="definition-summary">{definition.summary}</div>
              <table className="tbl">
                <thead>
                  <tr>
                    <th>Assumption</th>
                    <th>Approved value</th>
                    <th>Why it matters</th>
                  </tr>
                </thead>
                <tbody>
                  {definition.assumptions.map((a) => (
                    <tr key={a.key}>
                      <td>
                        <div className="col-name">{a.label}</div>
                        <div className="receipt-id">{a.key}</div>
                      </td>
                      <td>
                        <span className="spec">{a.value}</span>
                      </td>
                      <td className="muted small">{a.why}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="eyebrow" style={{ marginTop: 12 }}>
                Integrity {shortHash(report.integrity_hash, 28)} ·{" "}
                <a href={bundleUrl(run.id)} download>
                  Download assurance bundle
                </a>
              </div>
            </>
          ) : (
            <div className="muted">
              This legacy report predates the definition lock. Run the analysis again to create a
              replayable, definition-locked pipeline.
            </div>
          )}
        </section>

        {comparison && (
          <section className="report-section" id="comparison">
            <div className="rs-head">
              <span className="rs-num">{sectionNo("comparison")}</span>
              <h2 className="rs-title">Compared with the previous export</h2>
              <span className="rs-note">computed in Rust from two execution ledgers</span>
            </div>
            <div className="comparison">
              <div className="comparison-head">
                <h4>
                  {comparison.previous_rows ?? "—"} → {comparison.current_rows ?? "—"} rows
                </h4>
                <span className="eyebrow">
                  source {shortHash(comparison.source_run_id, 8)} ·{" "}
                  <Link to={`/r/${comparison.source_run_id}`}>open original</Link>
                </span>
              </div>
              <table className="tbl">
                <thead>
                  <tr>
                    <th>Metric</th>
                    <th className="num">Previous</th>
                    <th className="num">Current</th>
                    <th className="num">Change</th>
                  </tr>
                </thead>
                <tbody>
                  {comparison.metrics.map((m) => {
                    const isText = m.kind === "text";
                    const deltaClass =
                      isText || (m.delta_pct ?? 0) === 0
                        ? "delta-flat"
                        : (m.delta_pct ?? 0) > 0
                          ? "delta-pos"
                          : "delta-neg";
                    return (
                      <tr key={m.id}>
                        <td>
                          <div>{m.label}</div>
                          <div className="receipt-id">{m.id}</div>
                        </td>
                        <td className="num">
                          {isText
                            ? m.previous_text || "—"
                            : formatMetricValue(m.previous, m.kind)}
                        </td>
                        <td className="num">
                          {isText ? m.current_text || "—" : formatMetricValue(m.current, m.kind)}
                        </td>
                        <td className={`num ${deltaClass}`}>
                          {isText ? (
                            "changed"
                          ) : m.delta_pct == null ? (
                            "—"
                          ) : (
                            <>
                              {m.delta_pct > 0 ? "+" : ""}
                              {m.delta_pct}%
                            </>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        )}

        <section className="report-section" id="changed">
          <div className="rs-head">
            <span className="rs-num">{sectionNo("changed")}</span>
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
              {clean.rows_in - clean.rows_out > 0 && (
                <div className="eyebrow" style={{ marginBottom: 10 }}>
                  Rows removed: {clean.rows_in - clean.rows_out} · of which{" "}
                  {clean.duplicates_removed} exact duplicate(s)
                </div>
              )}
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
              {run.chart_data && (
                <div className="eyebrow" style={{ marginTop: 8 }}>
                  Metrics used {run.chart_data.rows_used} of {clean.rows_out} cleaned rows · the
                  rest were excluded by the approved definition
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
            <span className="rs-num">{sectionNo("exhibit")}</span>
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
          {chartAudit && (
            <details style={{ marginTop: 10 }}>
              <summary className="eyebrow" style={{ cursor: "pointer" }}>
                Chart data audit · {chartAudit.checks.filter((c) => c.passed).length}/
                {chartAudit.checks.length} checks ·{" "}
                {chartAudit.passed ? "series matches the ledger" : "flagged"}
              </summary>
              <div className="checks">
                {chartAudit.checks.map((c) => (
                  <div className="check-row" key={c.name}>
                    <span className={`check-mark ${c.passed ? "" : "fail"}`}>
                      {c.passed ? "✓" : "×"}
                    </span>
                    <span>{c.name}</span>
                    <span className="check-detail">{c.detail}</span>
                  </div>
                ))}
              </div>
            </details>
          )}
        </section>

        <section className="report-section" id="note">
          <div className="rs-head">
            <span className="rs-num">{sectionNo("note")}</span>
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
            <span className="rs-num">{sectionNo("receipts")}</span>
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
          <div className="eyebrow" style={{ marginTop: 6 }}>
            Integrity hash {report.integrity_hash}
          </div>
        </section>

        {consensus && (
          <section className="report-section" id="check">
            <div className="rs-head">
              <span className="rs-num">{sectionNo("check")}</span>
              <h2 className="rs-title">Independent check</h2>
              <span className="rs-note">
                {consensus.model} · separate sandbox · no access to the analyst script
              </span>
            </div>

            {consensus.status === "agree" ? (
              <div className="panel-good">
                <div className="panel-title-2">Two independent implementations agree.</div>
                <p className="small" style={{ marginTop: 4 }}>
                  A second model computed the same metrics from the approved definition and the raw
                  file. Every delta is within tolerance. This is corroboration, not just provenance.
                </p>
              </div>
            ) : consensus.status === "disagree" ? (
              <div className="panel-bad">
                <div className="panel-title-2">The two implementations disagree.</div>
                <p className="small" style={{ marginTop: 4 }}>
                  The report is flagged and the verified stamp is withheld. A disagreement is a real
                  signal, not something to smooth over.
                </p>
              </div>
            ) : consensus.status === "checker_failed" ? (
              <div className="panel-warn">
                <div className="panel-title-2">The independent check did not complete.</div>
                <p className="small" style={{ marginTop: 4 }}>
                  The primary report still has provenance, but only one implementation produced it.
                  The checker error is shown below.
                </p>
              </div>
            ) : (
              <div className="panel-warn">
                <div className="panel-title-2">No independent check was run.</div>
              </div>
            )}

            {consensus.deltas.length > 0 && (
              <table className="tbl" style={{ marginTop: 12 }}>
                <thead>
                  <tr>
                    <th>Metric</th>
                    <th>Primary execution</th>
                    <th>Independent execution</th>
                    <th className="num">Difference</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {consensus.deltas.map((d) => (
                    <tr key={d.id}>
                      <td>
                        <div>{d.label}</div>
                        <div className="receipt-id">{d.id}</div>
                      </td>
                      <td className="receipt-value">{d.primary}</td>
                      <td className="receipt-value">{d.checker}</td>
                      <td className="num">{d.diff_pct}%</td>
                      <td style={{ textAlign: "right" }}>
                        <span className={`trust ${d.agree ? "good" : "bad"}`}>
                          {d.agree ? "match" : "differs"}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <details style={{ marginTop: 14 }}>
              <summary className="eyebrow" style={{ cursor: "pointer" }}>
                Checker sandbox {consensus.checker_sandbox || "—"} · exit{" "}
                {consensus.checker_exit ?? "—"} · view checker code
              </summary>
              <pre className="code" style={{ marginTop: 8, maxHeight: 320 }}>
                {consensus.checker_code || "— no checker code —"}
              </pre>
              {consensus.checker_stderr && (
                <pre className="code" style={{ maxHeight: 160, marginTop: 8 }}>
                  {consensus.checker_stderr}
                </pre>
              )}
            </details>
          </section>
        )}

        <section className="report-section" id="containment">
          <div className="rs-head">
            <span className="rs-num">{sectionNo("containment")}</span>
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

          <div className="replay" style={{ marginTop: 16 }}>
            <div>
              <h4>Try to make it lie</h4>
              <p>
                Inject a fake number and an unknown metric into this report's note, then run the
                same verifier that stamped the real report. The strip should lose its stamp.
              </p>
            </div>
            <div className="replay-actions">
              <button className="btn" onClick={runChallenge} disabled={challengeBusy}>
                {challengeBusy ? "Checking…" : "Run adversarial check"}
              </button>
              <a className="btn" href={bundleUrl(run.id)} download>
                Assurance bundle
              </a>
            </div>
          </div>
          {challengeError && <div className="notice bad">{challengeError}</div>}
          {challenge && (
            <div className="tamper">
              <div className="comparison-head">
                <div>
                  <div className="notice-title">Tampered note · verifier response</div>
                  <div className="muted small">{challenge.note}</div>
                </div>
                <span className="stamp bad" style={{ position: "static", transform: "rotate(-2deg)" }}>
                  Stamp withheld
                </span>
              </div>
              <Markdown
                markdown={challenge.markdown}
                onClaim={(id) => setClaim(claimMap[id] || null)}
              />
              <div className="checks">
                {challenge.flags.map((f, i) => (
                  <div className="check-row" key={i}>
                    <span className="check-mark fail">×</span>
                    <span>{f.kind.replace(/_/g, " ")}</span>
                    <span className="check-detail">{f.detail}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {run.mode === "ask" && (
            <div className="replay" style={{ marginTop: 16 }}>
              <div>
                <h4>Replay this pipeline on a new export</h4>
                <p>
                  No model in the loop. The same definition, script and note template are
                  re-executed on a new file, then re-audited. This is the retention loop: build the
                  close once, reuse it every month.
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

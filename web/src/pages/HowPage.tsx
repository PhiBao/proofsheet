import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../lib/api";
import type { Health, Proof } from "../lib/types";

function ArchitectureDiagram() {
  return (
    <svg viewBox="0 0 900 400" role="img" aria-label="Proofsheet architecture">
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#17150F" />
        </marker>
        <marker id="arrowGreen" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#1E6B4C" />
        </marker>
      </defs>

      <rect x="14" y="132" width="138" height="112" fill="#FBF8F0" stroke="#17150F" />
      <text x="83" y="158" textAnchor="middle" fontFamily="var(--mono)" fontSize="10" letterSpacing="1.4" fill="#6B6555">BROWSER</text>
      <text x="83" y="182" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">React document</text>
      <text x="83" y="200" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">upload · ask · receipts</text>
      <text x="83" y="224" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="#8E8776">no model keys</text>

      <rect x="214" y="38" width="268" height="300" fill="#FBF8F0" stroke="#17150F" />
      <text x="348" y="62" textAnchor="middle" fontFamily="var(--mono)" fontSize="9.5" letterSpacing="1.4" fill="#6B6555">RUST CONTROL PLANE · AXUM</text>
      {[
        ["Rust CSV profiler", "deterministic dirt map"],
        ["Planner client", "plan → python script"],
        ["Sandbox orchestrator", "create · exec · destroy"],
        ["Claim verifier", "placeholders · literal-digit scan"],
        ["SQLite + artifacts", "runs · metrics · PNG · CSV"],
      ].map(([title, sub], i) => (
        <g key={title}>
          <rect x="232" y={80 + i * 48} width="232" height="38" fill="#F5F0E4" stroke="#D9D1BE" />
          <text x="244" y={97 + i * 48} fontFamily="var(--sans)" fontSize="12" fontWeight="500" fill="#17150F">{title}</text>
          <text x="244" y={110 + i * 48} fontFamily="var(--mono)" fontSize="8.5" fill="#8E8776">{sub}</text>
        </g>
      ))}
      <text x="348" y="326" textAnchor="middle" fontFamily="var(--mono)" fontSize="8.5" fill="#8E8776">one VM · systemd · no keys in the sandbox</text>

      <rect x="566" y="38" width="318" height="112" fill="#FBF8F0" stroke="#17150F" />
      <text x="725" y="62" textAnchor="middle" fontFamily="var(--mono)" fontSize="9.5" letterSpacing="1.2" fill="#6B6555">VULTR SERVERLESS INFERENCE</text>
      <text x="725" y="86" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">deepseek-v4.1-flash · qwen3.8-27b</text>
      <text x="725" y="106" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="#8E8776">OpenAI-compatible · all LLM calls</text>
      <text x="725" y="128" textAnchor="middle" fontFamily="var(--mono)" fontSize="8.5" fill="#1E6B4C">plan · code · note repair</text>

      <rect x="566" y="194" width="318" height="186" fill="#F3F7F1" stroke="#1E6B4C" strokeDasharray="5 4" />
      <text x="725" y="218" textAnchor="middle" fontFamily="var(--mono)" fontSize="9.5" letterSpacing="1.2" fill="#1E6B4C">SANDBOX BOUNDARY · DISPOSABLE</text>
      <text x="725" y="243" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">gVisor / Docker · read-only root</text>
      <text x="725" y="263" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">no network · no secrets</text>
      <text x="725" y="283" textAnchor="middle" fontFamily="var(--sans)" fontSize="12" fill="#17150F">time · memory · pid caps</text>
      <text x="725" y="307" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="#8E8776">python3 + pandas + matplotlib</text>
      <text x="725" y="330" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="#8E8776">/work/input.csv → cleaned.csv · chart.png</text>
      <text x="725" y="350" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" fill="#1E6B4C">destroyed after every task →</text>

      <line x1="152" y1="188" x2="212" y2="188" stroke="#17150F" markerEnd="url(#arrow)" />
      <line x1="214" y1="150" x2="160" y2="150" stroke="#17150F" markerEnd="url(#arrow)" />
      <text x="183" y="142" textAnchor="middle" fontFamily="var(--mono)" fontSize="8" fill="#6B6555">report</text>
      <line x1="348" y1="38" x2="600" y2="36" stroke="#17150F" markerEnd="url(#arrow)" />
      <line x1="640" y1="36" x2="400" y2="38" stroke="#17150F" markerEnd="url(#arrow)" />
      <text x="505" y="28" textAnchor="middle" fontFamily="var(--mono)" fontSize="8" fill="#6B6555">prompts / completions</text>
      <line x1="482" y1="230" x2="560" y2="264" stroke="#1E6B4C" markerEnd="url(#arrowGreen)" />
      <line x1="566" y1="290" x2="486" y2="260" stroke="#1E6B4C" markerEnd="url(#arrowGreen)" />
      <text x="523" y="248" textAnchor="middle" fontFamily="var(--mono)" fontSize="8" fill="#1E6B4C">script + data</text>
      <text x="523" y="308" textAnchor="middle" fontFamily="var(--mono)" fontSize="8" fill="#1E6B4C">ledger + artifacts</text>
    </svg>
  );
}

function VerificationDiagram() {
  return (
    <svg viewBox="0 0 900 250" role="img" aria-label="Claim verification flow">
      <defs>
        <marker id="a2" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#17150F" />
        </marker>
        <marker id="a2g" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#1E6B4C" />
        </marker>
      </defs>

      {[
        { x: 10, t: "SANDBOX EXECUTION", s: ["script writes metrics.json", "numbers + names"] },
        { x: 192, t: "MODEL NOTE", s: ["{{m:total_revenue}}", "no digits allowed"] },
        { x: 374, t: "RUST RENDER", s: ["placeholder → ledger", "each value gets a chip"] },
        { x: 556, t: "AUDIT", s: ["literal digits?", "unknown metric ids?"] },
        { x: 738, t: "STAMP", s: ["0 flags → verified", "else: withheld"] },
      ].map((b) => (
        <g key={b.t}>
          <rect x={b.x} y="52" width="152" height="94" fill="#FBF8F0" stroke="#17150F" />
          <text x={b.x + 76} y="76" textAnchor="middle" fontFamily="var(--mono)" fontSize="8.5" letterSpacing="1.1" fill="#6B6555">{b.t}</text>
          <text x={b.x + 76} y="102" textAnchor="middle" fontFamily="var(--sans)" fontSize="11.5" fill="#17150F">{b.s[0]}</text>
          <text x={b.x + 76} y="121" textAnchor="middle" fontFamily="var(--mono)" fontSize="8.5" fill="#8E8776">{b.s[1]}</text>
        </g>
      ))}
      {[162, 344, 526, 708].map((x) => (
        <line key={x} x1={x} y1="99" x2={x + 28} y2="99" stroke="#17150F" markerEnd="url(#a2)" />
      ))}
      <path d="M 632 146 L 632 190 L 268 190 L 268 150" fill="none" stroke="#1E6B4C" markerEnd="url(#a2g)" />
      <text x="450" y="208" textAnchor="middle" fontFamily="var(--mono)" fontSize="8.5" fill="#1E6B4C">repair loop · max 2 rewrites · the failure is visible</text>
      <text x="450" y="34" textAnchor="middle" fontFamily="var(--mono)" fontSize="9" letterSpacing="1.2" fill="#6B6555">THE REPORT CANNOT CONTAIN A NUMBER THE SANDBOX DID NOT PRODUCE</text>
    </svg>
  );
}

export function HowPage() {
  const [health, setHealth] = useState<Health | null>(null);
  const [proof, setProof] = useState<Proof | null>(null);

  useEffect(() => {
    api.health().then(setHealth).catch(() => undefined);
    api.proof().then(setProof).catch(() => undefined);
  }, []);

  return (
    <div className="wrap">
      <div className="how-head">
        <div className="eyebrow">Architecture · verification · honesty</div>
        <h1 className="how-title">
          The analyst that shows its work.
        </h1>
        <p className="drill-lede">
          Generic data tools ask you to trust a number. Proofsheet makes the number carry a
          receipt: the executed code, the sandbox run, the isolation level and the teardown. The
          model is allowed to plan and write code. It is not allowed to author a figure.
        </p>
      </div>

      <div className="how-grid">
        <div>
          <h2>The problem is not chart generation</h2>
          <p>
            Every AI data tool can produce a chart. The hard part is defending the number in a
            board deck when someone asks where it came from. Analysts spend most of their time
            cleaning the export, then re-checking the output. The failure mode is vivid: a model
            can invent a plausible figure inside a chart and no one notices until it is expensive.
          </p>
          <p>
            Proofsheet starts from a different question: what would it take to trust the number
            without re-doing the work? The answer is not a better prompt. It is a control plane
            that separates <em>planning</em>, <em>execution</em> and <em>claim rendering</em>, and
            refuses to let the last one invent anything.
          </p>

          <h2>The verification model</h2>
          <p>
            Every run produces a metric ledger from the sandbox: named values with a kind, written
            by the executed script. The analyst note is generated separately and may only cite
            numbers and names as <code>{"{{m:metric_id}}"}</code> placeholders. Rust renders those
            placeholders from the ledger and audits the result for literal digits or unknown ids.
            If the audit fails, the model rewrites the note. If it still fails, the report keeps
            the flag and withholds the verified stamp.
          </p>
          <figure className="diagram" style={{ margin: 0 }}>
            <VerificationDiagram />
            <figcaption>Claim verification: the prose is downstream of execution, not parallel to it.</figcaption>
          </figure>

          <h2>Four trust layers</h2>
          <div className="steps" style={{ gridTemplateColumns: "1fr 1fr", marginTop: 14 }}>
            <div className="step" style={{ borderLeft: 0, paddingLeft: 0 }}>
              <h3 style={{ fontSize: 20 }}>1. Definition lock</h3>
              <p>
                Before execution, the planner proposes the business rules and the user approves
                them: shipping in or out, which statuses count, how missing values are handled,
                how ambiguous dates are read. The approved definition is stored with the run,
                included in the receipt, and reused on replay.
              </p>
              <h3 style={{ fontSize: 20, marginTop: 16 }}>2. Execution ledger</h3>
              <p>
                The sandbox writes named metrics. The note may only cite them through
                placeholders, so the prose cannot invent a figure. Every claim links to the code,
                stdout, isolation level and teardown.
              </p>
            </div>
            <div className="step">
              <h3 style={{ fontSize: 20 }}>3. Chart audit</h3>
              <p>
                The chart emits its plotted series alongside the PNG. Rust checks that a summed
                series matches the ledger, that the top category matches a receipted name, that
                the peak month matches, and that every approved assumption is recorded.
              </p>
              <h3 style={{ fontSize: 20, marginTop: 16 }}>4. Independent execution</h3>
              <p>
                A second model, which never sees the analyst script, recomputes the same metrics
                from the approved definition in a separate sandbox. Agreement produces the
                two-execution stamp; disagreement withholds it.
              </p>
            </div>
          </div>

          <h2>Architecture</h2>
          <p>
            The backend is a single Rust binary (Axum + Tokio + SQLx) on a Vultr Cloud Compute
            instance. It profiles the CSV, calls Vultr Serverless Inference with the
            OpenAI-compatible API, dispatches the generated script to a disposable sandbox, reads
            the artifacts, verifies the note, and serves the React document and the public report.
            Secrets live only in the Rust process. The sandbox receives code and data, never
            credentials.
          </p>
          <figure className="diagram">
            <ArchitectureDiagram />
            <figcaption>Control plane, inference and the sandbox boundary. All inference is Vultr Serverless Inference.</figcaption>
          </figure>

          <h2>Containment is a product surface</h2>
          <p>
            Isolation is not a footnote. Each run reports the boundary it actually used, and the
            drill page runs hostile commands live. The host keeps a canary file whose hash is
            checked before and after every step.
          </p>
          <table className="honesty" style={{ marginTop: 12 }}>
            <tbody>
              <tr>
                <td>process</td>
                <td>Separate container, read-only root filesystem, only a per-run workdir mounted.</td>
              </tr>
              <tr>
                <td>network</td>
                <td><code>--network none</code>; outbound connections fail inside the namespace.</td>
              </tr>
              <tr>
                <td>secrets</td>
                <td>No Vultr key, inference key or app secret is passed into the sandbox.</td>
              </tr>
              <tr>
                <td>limits</td>
                <td>Wall-clock cap, memory cap, CPU cap, pid cap, dropped capabilities.</td>
              </tr>
              <tr>
                <td>lifecycle</td>
                <td>Create → execute → collect → force-remove. Nothing is reused.</td>
              </tr>
              <tr>
                <td>dirt</td>
                <td>After execution, Rust re-profiles the cleaned file: residual aliases, numeric parse rate, date parse rate.</td>
              </tr>
            </tbody>
          </table>

          <h2>What is real, and what is not</h2>
          <table className="honesty">
            <tbody>
              <tr>
                <td>real</td>
                <td>CSV profiling in Rust; Vultr inference calls; pandas execution in a sandbox; chart PNG; cleaned CSV; metric ledger; claim audit; containment commands; host canary.</td>
              </tr>
              <tr>
                <td>real</td>
                <td>Deterministic replay: the saved script and template are re-executed with no model in the loop.</td>
              </tr>
              <tr>
                <td>honest</td>
                <td>The isolation label is whatever the host actually provides (gVisor or hardened Docker). It is never upgraded in the copy.</td>
              </tr>
              <tr>
                <td>not yet</td>
                <td>Authentication, multi-tenant isolation, billing, object storage, one microVM per task. The demo is single-tenant and reports are unlisted, not private.</td>
              </tr>
              <tr>
                <td>not claimed</td>
                <td>The verifier guarantees provenance, not that the analysis plan is the best possible one. A wrong plan still produces a receipted wrong answer; the code is visible so a human can catch it.</td>
              </tr>
            </tbody>
          </table>

          <h2>Why a business, not a demo</h2>
          <p>
            The buyer is an ops, finance or founder-analyst at a 5–200 person company with a
            recurring export. The value anchor is hours of cleaning plus the cost of a wrong number
            in a decision document. The pricing hypothesis is $29–99 per analyst seat per month, or
            $199–499 per team, with the saved pipeline as the retention loop: the second month is
            one click, and switching cost accumulates with every saved pipeline.
          </p>
          <p>
            Distribution is the output itself: a verified report is a shareable artifact with a
            public URL, and every share carries the receipt model into a room where someone else
            has the same Monday export. The wedge is narrow on purpose — CSV first, then the same
            control plane for every recurring data task.
          </p>

          <h2>Limitations and next</h2>
          <ul>
            <li>Best on tabular CSV up to the upload cap. Exotic schemas may need one visible repair attempt.</li>
            <li>Code generation is probabilistic; the verifier proves provenance, not analytical taste.</li>
            <li>Production should use one microVM per task (Microsandbox / Firecracker) and object storage for artifacts; the interface already reports the boundary per run.</li>
            <li>Next: scheduled replays, a pipeline library, row-level lineage on the chart, and team workspaces.</li>
          </ul>
        </div>

        <aside>
          <div className="aside-card">
            <div className="eyebrow">Live on this host</div>
            <dl className="kv" style={{ marginTop: 10, gridTemplateColumns: "86px 1fr" }}>
              <dt>Sandbox</dt>
              <dd>{health?.sandbox.label || "—"}</dd>
              <dt>Ready</dt>
              <dd>{health?.sandbox.ready ? "yes" : "no"}</dd>
              <dt>Region</dt>
              <dd>{health?.region || "—"}</dd>
              <dt>Provider</dt>
              <dd>{health?.models.provider || "Vultr"}</dd>
              <dt>Planner</dt>
              <dd>{health?.models.plan || "—"}</dd>
              <dt>Code</dt>
              <dd>{health?.models.code || "—"}</dd>
              <dt>Verifier</dt>
              <dd>
                {health ? `${health.verifier.passed}/${health.verifier.total}` : "—"} self-tests
              </dd>
              <dt>Golden</dt>
              <dd>
                {proof ? `${proof.golden.passed}/${proof.golden.total}` : "—"} product checks
              </dd>
            </dl>
          </div>

          <div className="aside-card">
            <div className="eyebrow">Golden checks · {proof ? `${proof.golden.passed}/${proof.golden.total}` : "…"}</div>
            <div style={{ marginTop: 10, maxHeight: 280, overflow: "auto" }}>
              {proof?.golden.checks.map((c) => (
                <div
                  key={c.name}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 10,
                    padding: "6px 0",
                    borderBottom: "1px solid var(--rule)",
                    fontSize: 11.5,
                  }}
                >
                  <span>{c.name}</span>
                  <span style={{ color: c.passed ? "var(--green)" : "var(--red)", fontFamily: "var(--mono)", fontSize: 9 }}>
                    {c.passed ? "pass" : "fail"}
                  </span>
                </div>
              ))}
              {!proof && <div className="muted small">Loading…</div>}
            </div>
          </div>

          <div className="aside-card">
            <div className="eyebrow">Fixture checks</div>
            <div style={{ marginTop: 10 }}>
              {proof?.fixtures.map((f) => (
                <div
                  key={f.id}
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 10,
                    padding: "7px 0",
                    borderBottom: "1px solid var(--rule)",
                    fontSize: 12,
                  }}
                >
                  <span className="mono" style={{ fontSize: 10 }}>
                    {f.filename}
                  </span>
                  <span className={f.passed ? "mono" : "mono"} style={{ color: f.passed ? "var(--green)" : "var(--red)" }}>
                    {f.passed ? "pass" : "fail"}
                  </span>
                </div>
              ))}
              {!proof && <div className="muted small">Loading…</div>}
            </div>
            <div className="muted small" style={{ marginTop: 10 }}>
              Computed on every request by re-profiling the bundled dirty fixtures.
            </div>
          </div>

          <div className="aside-card">
            <div className="eyebrow">Hackathon fit</div>
            <p className="muted small" style={{ marginTop: 8 }}>
              Vultr Agent Rush · Blast Radius Zero · Pattern A (sandboxed code execution). Backend
              on a Vultr VM; every LLM call through Vultr Serverless Inference; containment
              demonstrated live.
            </p>
            <Link to="/drill" className="link" style={{ display: "inline-block", marginTop: 8 }}>
              Run the drill →
            </Link>
          </div>
        </aside>
      </div>
    </div>
  );
}

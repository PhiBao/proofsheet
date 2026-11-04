import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { api, ApiError } from "../lib/api";
import type { Health, Proof, Sample } from "../lib/types";
import { Uploader } from "../components/Uploader";
import { isolationLabel } from "../lib/format";

export function Landing() {
  const navigate = useNavigate();
  const [samples, setSamples] = useState<Sample[]>([]);
  const [health, setHealth] = useState<Health | null>(null);
  const [proof, setProof] = useState<Proof | null>(null);
  const [busyKey, setBusyKey] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api.samples().then((r) => setSamples(r.samples)).catch(() => undefined);
    api.health().then(setHealth).catch(() => undefined);
    api.proof().then(setProof).catch(() => undefined);
  }, []);

  const openSample = async (key: string) => {
    setBusyKey(key);
    setError("");
    try {
      const { dataset } = await api.useSample(key);
      navigate(`/d/${dataset.id}`);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "could not open that sample");
    } finally {
      setBusyKey("");
    }
  };

  const goldenPassed = proof?.golden?.passed ?? 0;
  const goldenTotal = proof?.golden?.total ?? 0;

  return (
    <>
      <div className="wrap">
        <section className="hero">
          <div className="hero-rail">
            <div className="num">01 — Intake</div>
          </div>
          <div>
            <h1 className="display">
              Numbers you can <em>defend.</em>
            </h1>
            <p className="lede">
              Drop a messy CSV. Proofsheet writes the analysis code, runs it in a disposable
              sandbox on Vultr, and returns a cleaned file, an executed chart, and a report where
              every number and every name is receipted to the execution that produced it.
            </p>
            <div style={{ marginTop: 28 }}>
              <Uploader onUploaded={(d) => navigate(`/d/${d.id}`)} />
            </div>
            <div className="hero-actions">
              <Link to="/how" className="link">
                How the receipts work
              </Link>
              <span className="hero-note">
                No sign-up. The file never leaves this Vultr VM.
              </span>
            </div>
          </div>
        </section>

        <section className="section">
          <div className="section-head">
            <h2 className="section-title">Try a real dirty export</h2>
            <span className="eyebrow">Plant the dirt, watch it get cleaned</span>
          </div>
          {error && <div className="upload-error">{error}</div>}
          <div className="sample-list">
            {samples
              .filter((s) => s.key !== "orders_next")
              .map((s) => (
                <button
                  key={s.key}
                  className="sample-row"
                  onClick={() => openSample(s.key)}
                  disabled={!!busyKey}
                >
                  <span>
                    <span className="sample-name">{s.description}</span>
                    <span className="sample-file">{s.filename}</span>
                  </span>
                  <span className="sample-cell">{s.rows ?? "—"} rows</span>
                  <span className="sample-cell hide">{s.cols ?? "—"} cols</span>
                  <span className="dirt">
                    <span className="dirt-bar">
                      <i style={{ width: `${Math.max(6, s.dirty_score ?? 0)}%` }} />
                    </span>
                    {s.dirty_score ?? 0}/100 dirty
                  </span>
                  <span className="arrow">{busyKey === s.key ? "…" : "→"}</span>
                </button>
              ))}
            {samples.length === 0 && <div className="muted small">Loading samples…</div>}
          </div>
        </section>

        <section className="section">
          <div className="stats">
            <div className="stat">
              <div className="stat-value">
                {health ? isolationLabel(health.sandbox.isolation) : "—"}
                <small>{health?.region}</small>
              </div>
              <div className="stat-label">Sandbox boundary</div>
            </div>
            <div className="stat">
              <div className="stat-value">
                {health ? `${health.verifier.passed}/${health.verifier.total}` : "—"}
              </div>
              <div className="stat-label">Verifier self-test</div>
            </div>
            <div className="stat">
              <div className="stat-value">
                {goldenTotal ? `${goldenPassed}/${goldenTotal}` : "—"}
              </div>
              <div className="stat-label">Golden product checks passing</div>
            </div>
          </div>
        </section>

        <section className="section">
          <div className="section-head">
            <h2 className="section-title">A receipt, not a vibe.</h2>
            <span className="eyebrow">Three steps</span>
          </div>
          <div className="steps">
            <div className="step">
              <div className="step-num">01</div>
              <h3>Profile, in Rust.</h3>
              <p>
                The file is parsed and profiled before any model sees it: duplicate rows, mixed
                date formats, currency text, percent strings, nulls, ambiguous aliases.
              </p>
            </div>
            <div className="step">
              <div className="step-num">02</div>
              <h3>Lock the definition. Then execute.</h3>
              <p>
                Before any code runs, you approve the business rules: shipping, status scope,
                missing values, date order. The model writes the script; a separate control plane
                runs it with no network, no secrets, a read-only root, and hard caps.
              </p>
            </div>
            <div className="step">
              <div className="step-num">03</div>
              <h3>Receipt every claim. Check it twice.</h3>
              <p>
                The note may only cite placeholders resolved from the execution ledger. A second,
                independent implementation recomputes the metrics and the chart series. Disagreement
                withholds the stamp.
              </p>
            </div>
          </div>
        </section>
      </div>

      <section className="band">
        <div className="band-inner">
          <div>
            <h2>
              Blast radius <em>zero.</em>
            </h2>
            <p>
              The drill runs an actual <code>rm -rf</code>, a process bomb, a runaway loop, a
              credential scan and an outbound connection against a fresh sandbox — then checks the
              host canary. It is not a recording.
            </p>
            <Link to="/drill" className="btn">
              Run the containment drill
            </Link>
          </div>
          <div className="band-list">
            <div className="band-item">
              <span>Process isolation</span>
              <span>{health?.sandbox.label || "gVisor / Docker"}</span>
            </div>
            <div className="band-item">
              <span>Secret hygiene</span>
              <span>no keys in the sandbox</span>
            </div>
            <div className="band-item">
              <span>Resource limits</span>
              <span>time · memory · pids</span>
            </div>
            <div className="band-item">
              <span>Lifecycle</span>
              <span>destroy after every task</span>
            </div>
            <div className="band-item">
              <span>Inference</span>
              <span>Vultr Serverless Inference</span>
            </div>
          </div>
        </div>
      </section>

      <div className="wrap">
        <section className="section">
          <div className="section-head">
            <h2 className="section-title">Built for the Monday export.</h2>
            <span className="eyebrow">Who it is for</span>
          </div>
          <div className="steps" style={{ gridTemplateColumns: "1fr 1fr" }}>
            <div className="step" style={{ borderLeft: 0, paddingLeft: 0 }}>
              <p style={{ marginTop: 0 }}>
                An ops, finance or founder-analyst at a small company gets a messy export every week
                or month. They burn hours cleaning it, then cannot fully defend the numbers they put
                in front of a boss or an investor.
              </p>
              <p>
                Generic AI data tools assert results. Proofsheet shows the executed code, the run,
                the isolation level and the teardown — and refuses to stamp a claim that execution
                did not produce.
              </p>
            </div>
            <div className="step">
              <p style={{ marginTop: 0 }}>
                The first report takes a minute. The second month is one click: the saved pipeline
                replays on the new file with no model in the loop.
              </p>
              <p className="muted">
                The wedge is trust, not chart generation. The product is the verification layer
                around execution.
              </p>
            </div>
          </div>
        </section>
      </div>
    </>
  );
}

import type { Definition, Plan } from "../lib/types";

export function DefinitionPanel({
  plan,
  definition,
  onChange,
  onRun,
  onCancel,
  busy,
  error,
}: {
  plan: Plan;
  definition: Definition;
  onChange: (d: Definition) => void;
  onRun: () => void;
  onCancel: () => void;
  busy: boolean;
  error: string;
}) {
  const update = (key: string, value: string) => {
    onChange({
      ...definition,
      assumptions: definition.assumptions.map((a) =>
        a.key === key ? { ...a, value } : a
      ),
    });
  };

  return (
    <div className="definition">
      <div className="definition-head">
        <div>
          <div className="eyebrow">Definition lock · approval required</div>
          <h3 className="definition-title">Approve the business definition.</h3>
          <p className="definition-sub">
            The model may plan and write code. It cannot change these rules after approval.
            Every execution, replay and receipt uses this definition.
          </p>
        </div>
        <div className="definition-plan">
          <div className="mono small muted">PLAN</div>
          <div className="definition-plan-title">{plan.title}</div>
          <div className="muted small">{plan.summary}</div>
          <div className="metric-specs">
            {plan.metrics.map((m) => (
              <span className="spec" key={m.id}>
                {m.id} · {m.kind}
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="definition-summary">{definition.summary}</div>

      <div className="assumptions">
        {definition.assumptions.map((a) => (
          <div className="assumption" key={a.key}>
            <div className="assumption-copy">
              <div className="assumption-label">{a.label}</div>
              <div className="assumption-key">{a.key}</div>
              {a.why && <div className="assumption-why">{a.why}</div>}
            </div>
            <div className="assumption-control">
              {a.kind === "bool" ? (
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={a.value === "true"}
                    onChange={(e) => update(a.key, e.target.checked ? "true" : "false")}
                  />
                  <span>{a.value === "true" ? "included" : "excluded"}</span>
                </label>
              ) : a.kind === "enum" && a.options.length > 0 ? (
                <select
                  className="select"
                  value={a.value}
                  onChange={(e) => update(a.key, e.target.value)}
                >
                  {a.options.map((o) => (
                    <option key={o} value={o}>
                      {o}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  className="input"
                  value={a.value}
                  onChange={(e) => update(a.key, e.target.value)}
                />
              )}
            </div>
          </div>
        ))}
      </div>

      {error && <div className="upload-error">{error}</div>}

      <div className="definition-foot">
        <div className="eyebrow">Approved before execution · stored with the run</div>
        <div className="definition-actions">
          <button className="btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={onRun} disabled={busy}>
            {busy ? "Starting…" : "Approve & run"}
          </button>
        </div>
      </div>
    </div>
  );
}

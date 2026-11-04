import type { Profile } from "../lib/types";

export function ProfileTable({ profile }: { profile: Profile }) {
  const dirty = profile.issues.filter((i) => i.severity !== "info").length;

  return (
    <div>
      <div className="panel-head">
        <div className="panel-title">Profile · deterministic, in Rust</div>
        <div className="eyebrow">
          {profile.rows} rows · {profile.columns.length} cols · {profile.duplicate_rows} dupes
        </div>
      </div>

      <table className="tbl">
        <thead>
          <tr>
            <th>Column</th>
            <th>Kind</th>
            <th className="num">Nulls</th>
            <th className="num">Unique</th>
            <th>Detected dirt</th>
          </tr>
        </thead>
        <tbody>
          {profile.columns.map((c) => (
            <tr key={c.name}>
              <td className="col-name">{c.name}</td>
              <td className="kind">{c.kind}</td>
              <td className="num">
                {c.nulls}
                {c.null_pct > 0 ? <span className="muted small"> · {c.null_pct}%</span> : null}
              </td>
              <td className="num">{c.unique}</td>
              <td className="col-issues">
                {c.issues.length ? c.issues.slice(0, 2).join(" · ") : <span className="muted">clean</span>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {dirty > 0 && (
        <div className="issue-list" style={{ marginTop: 18 }}>
          {profile.issues
            .filter((i) => i.severity !== "info")
            .slice(0, 8)
            .map((issue, i) => (
              <div className="issue" key={i}>
                <span className={`sev sev-${issue.severity}`}>{issue.severity}</span>
                <span className="issue-col">{issue.column || "dataset"}</span>
                <span>{issue.detail}</span>
              </div>
            ))}
        </div>
      )}

      <details style={{ marginTop: 18 }}>
        <summary
          style={{
            cursor: "pointer",
            fontFamily: "var(--mono)",
            fontSize: 10,
            letterSpacing: "0.12em",
            textTransform: "uppercase",
            color: "var(--muted)",
          }}
        >
          Raw preview — the dirt judges can see
        </summary>
        <div style={{ overflowX: "auto", marginTop: 10 }}>
          <table className="tbl">
            <thead>
              <tr>
                {profile.preview[0]?.map((_, i) => (
                  <th key={i}>{profile.columns[i]?.name || `col ${i + 1}`}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {profile.preview.map((row, i) => (
                <tr key={i}>
                  {row.map((cell, j) => (
                    <td key={j} style={{ fontFamily: "var(--mono)", fontSize: 11, whiteSpace: "nowrap" }}>
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="eyebrow" style={{ marginTop: 8 }}>
          Preview truncated to {profile.preview.length} rows × {profile.preview[0]?.length || 0} columns
        </div>
      </details>
    </div>
  );
}

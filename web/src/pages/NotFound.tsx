import { Link } from "react-router-dom";

export function NotFound() {
  return (
    <div className="wrap">
      <div className="page-head">
        <div className="eyebrow">404 — not found</div>
        <h1 className="page-title">That page left no receipt.</h1>
        <p className="page-sub">The address does not resolve to a report on this server.</p>
      </div>
      <div className="section">
        <Link to="/" className="btn btn-primary">
          Back to intake
        </Link>
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api, ApiError } from "../lib/api";
import type { Report, Run } from "../lib/types";
import { RunCard } from "../components/RunCard";
import { ReportView } from "../components/ReportView";
import { formatDate } from "../lib/format";

export function ReportPage() {
  const { id } = useParams<{ id: string }>();
  const [run, setRun] = useState<Run | null>(null);
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!id) return;
    setRun(null);
    setReport(null);
    setError("");
    api
      .run(id)
      .then(async ({ run }) => {
        setRun(run);
        if (run.status === "complete") {
          const { report } = await api.report(id);
          setReport(report);
        }
      })
      .catch((e) => setError(e instanceof ApiError ? e.message : "could not load this run"));
  }, [id]);

  if (error) {
    return (
      <div className="wrap">
        <div className="page-head">
          <div className="eyebrow">Error</div>
          <h1 className="page-title">No receipt for that address.</h1>
          <p className="page-sub">{error}</p>
        </div>
        <div className="section">
          <Link to="/" className="btn">
            Back to intake
          </Link>
        </div>
      </div>
    );
  }

  if (report) {
    return (
      <div className="wrap">
        <div className="page-head" style={{ paddingBottom: 18 }}>
          <div className="breadcrumb">
            <Link to="/">Intake</Link>
            <span>/</span>
            <Link to={`/d/${report.dataset.id}`}>{report.dataset.filename}</Link>
            <span>/</span>
            <span>Report</span>
          </div>
        </div>
        <ReportView report={report} />
      </div>
    );
  }

  if (!run) {
    return (
      <div className="wrap">
        <div className="page-head">
          <div className="eyebrow">Loading</div>
          <h1 className="page-title">Fetching the run…</h1>
        </div>
      </div>
    );
  }

  return (
    <div className="wrap">
      <div className="page-head">
        <div className="breadcrumb">
          <Link to="/">Intake</Link>
          <span>/</span>
          <Link to={`/d/${run.dataset_id}`}>Dataset</Link>
          <span>/</span>
          <span>Live run</span>
        </div>
        <h1 className="page-title">Analysis in progress</h1>
        <div className="page-sub">
          {run.question} · started {formatDate(run.created_at)}
        </div>
      </div>
      <div style={{ maxWidth: 860, paddingTop: 26 }}>
        <RunCard
          runId={run.id}
          onComplete={async (finished) => {
            if (finished.status === "complete") {
              try {
                const { report } = await api.report(finished.id);
                setReport(report);
              } catch {
                /* still settling */
              }
            }
          }}
        />
      </div>
    </div>
  );
}

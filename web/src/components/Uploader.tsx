import { useRef, useState } from "react";
import { api, ApiError } from "../lib/api";
import type { Dataset } from "../lib/types";

export function Uploader({
  onUploaded,
  compact = false,
}: {
  onUploaded: (dataset: Dataset) => void;
  compact?: boolean;
}) {
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement | null>(null);

  async function handleFile(file?: File) {
    if (!file) return;
    setBusy(true);
    setError("");
    try {
      const { dataset } = await api.upload(file);
      onUploaded(dataset);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : "upload failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <div
        className={`drop ${dragging ? "dragging" : ""}`}
        onDragOver={(e) => {
          e.preventDefault();
          setDragging(true);
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDragging(false);
          handleFile(e.dataTransfer.files?.[0]);
        }}
      >
        <div>
          <div className="drop-title">
            {compact ? "Upload another export." : "Drop a messy CSV."}
          </div>
          <div className="drop-sub">
            {busy
              ? "Uploading and profiling in Rust…"
              : "Up to 12 MB. Profiled on the server before any model sees it."}
          </div>
        </div>
        <div className="drop-actions">
          <button className="btn btn-primary" onClick={() => inputRef.current?.click()} disabled={busy}>
            {busy ? "Working…" : "Choose file"}
          </button>
          <button
            className="btn"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              setError("");
              try {
                const { dataset } = await api.useSample("orders");
                onUploaded(dataset);
              } catch (e) {
                setError(e instanceof ApiError ? e.message : "could not load sample");
              } finally {
                setBusy(false);
              }
            }}
          >
            Use sample
          </button>
        </div>
        <input
          ref={inputRef}
          type="file"
          accept=".csv,.tsv,.txt,text/csv"
          onChange={(e) => handleFile(e.target.files?.[0])}
        />
      </div>
      {error && <div className="upload-error">{error}</div>}
    </div>
  );
}

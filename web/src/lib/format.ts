export function shortHash(hash: string, n = 10): string {
  return hash.slice(0, n);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function elapsed(iso: string): string {
  const start = new Date(iso).getTime();
  const secs = Math.max(0, Math.floor((Date.now() - start) / 1000));
  if (secs < 60) return `${secs}s`;
  return `${Math.floor(secs / 60)}m ${secs % 60}s`;
}

export function duration(ms: number | null): string {
  if (ms == null) return "—";
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

export function isolationLabel(code: string | null | undefined): string {
  switch (code) {
    case "gvisor":
      return "gVisor";
    case "microvm":
      return "microVM";
    case "docker":
      return "Docker";
    default:
      return code || "unknown";
  }
}

export function statusLabel(status: string): string {
  const map: Record<string, string> = {
    queued: "Queued",
    planning: "Planning",
    coding: "Writing code",
    executing: "Executing",
    repairing: "Repairing",
    verifying: "Verifying claims",
    complete: "Complete",
    failed: "Failed",
  };
  return map[status] || status;
}

export function isActive(status: string): boolean {
  return !["complete", "failed"].includes(status);
}

export function downloadText(name: string, text: string, mime = "text/plain") {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

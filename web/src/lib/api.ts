import type {
  Dataset,
  Drill,
  Health,
  Proof,
  Report,
  Run,
  RunSummary,
  Sample,
} from "./types";

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, init);
  const text = await res.text();
  let body: unknown = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = null;
  }
  if (!res.ok) {
    const message =
      body && typeof body === "object" && "error" in body
        ? String((body as { error: unknown }).error)
        : `request failed (${res.status})`;
    throw new ApiError(message, res.status);
  }
  return body as T;
}

export const api = {
  health: () => request<Health>("/health"),
  proof: () => request<Proof>("/proof"),
  samples: () => request<{ samples: Sample[] }>("/samples"),

  useSample: (key: string) =>
    request<{ dataset: Dataset }>(`/samples/${key}`, { method: "POST" }),

  upload: async (file: File) => {
    const form = new FormData();
    form.append("file", file);
    return request<{ dataset: Dataset }>("/datasets", {
      method: "POST",
      body: form,
    });
  },

  dataset: (id: string) =>
    request<{ dataset: Dataset; runs: RunSummary[] }>(`/datasets/${id}`),

  plan: (id: string, question: string) =>
    request<{ plan: import("./types").Plan; definition: import("./types").Definition; question: string }>(
      `/datasets/${id}/plan`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ question }),
      }
    ),

  startRun: (
    id: string,
    body: {
      question: string;
      plan: import("./types").Plan;
      definition: import("./types").Definition;
    }
  ) =>
    request<{ run_id: string }>(`/datasets/${id}/run`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }),

  ask: (id: string, question: string) =>
    request<{ run_id: string }>(`/datasets/${id}/ask`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ question }),
    }),

  run: (id: string) => request<{ run: Run }>(`/runs/${id}`),

  report: (id: string) => request<{ report: Report }>(`/runs/${id}/report`),

  replay: (id: string, body: { dataset_id?: string; sample_key?: string }) =>
    request<{ run_id: string }>(`/runs/${id}/replay`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }),

  startDrill: () =>
    request<{ drill_id: string }>("/drills", { method: "POST" }),

  drill: (id: string) => request<{ drill: Drill }>(`/drills/${id}`),

  latestDrill: () => request<{ drill: Drill | null }>("/drills/latest"),

  challenge: (id: string) =>
    request<{
      markdown: string;
      flags: import("./types").ClaimFlag[];
      passed: boolean;
      note: string;
    }>(`/runs/${id}/challenge`, { method: "POST" }),
};

export function artifactUrl(runId: string, file: string): string {
  return `/api/artifacts/${runId}/${file}`;
}

export function bundleUrl(runId: string): string {
  return `/api/runs/${runId}/bundle`;
}

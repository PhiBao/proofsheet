# Lablab submission copy — Proofsheet

## Project title

Proofsheet — the analyst that shows its work

## Short description

Upload a messy CSV. Proofsheet executes the analysis in a disposable gVisor sandbox
on Vultr and returns a cleaned file, a real chart, and a report where every number
and every name is receipted to the execution that produced it.

## Long description

### The problem

Every AI data tool can make a chart. The hard part is defending the number when
someone asks where it came from. Analysts spend most of their time cleaning exports,
then re-checking AI output. Models can invent a plausible figure inside a chart and no
one notices until it is expensive.

### What Proofsheet does

Proofsheet separates planning, execution, and claim rendering.

1. A Rust profiler maps the dirt in the file before any model sees it: duplicate rows,
   five date formats, currency text, percent strings, nulls, casing chaos.
2. Vultr Serverless Inference plans the analysis and writes a pandas script.
3. A Rust control plane runs that script in a fresh gVisor sandbox with no network,
   no secrets, a read-only root, and hard time/memory/pid caps.
4. The sandbox writes a metric ledger. The analyst note may only cite values as
   placeholders; Rust renders them from the ledger and audits for literal digits or
   unknown ids. If the audit fails, the model repairs the note; if it still fails, the
   report keeps the flag and withholds the verified stamp.
5. A saved pipeline can replay on a new export with no model in the loop.

### Containment (Blast Radius Zero)

The containment page runs hostile code live, one fresh sandbox per step: a real
`rm -rf --no-preserve-root /` (absorbed by the read-only root), a fork storm (stopped
by the pids limit), a runaway loop (killed at the wall-clock cap), a credential scan
(no keys in the sandbox), an outbound connection (`--network none`), and a host
canary probe (path not mounted). The host canary is byte-identical afterwards.

### Stack

- Backend: Rust (Axum, Tokio, SQLx, Reqwest) on a Vultr Cloud Compute VM in `ewr`.
- Inference: Vultr Serverless Inference, OpenAI-compatible:
  `deepseek-v4.1-flash-1017` (plan/repair), `qwen3.8-27b` (code).
- Sandbox: Docker + gVisor (`runsc`), prebuilt python/pandas/matplotlib image.
- Frontend: React + TypeScript + Vite, custom editorial design system.

### What is real

Real CSV profiling, real Vultr inference, real gVisor execution, real chart, real
ledger, real claim audit, real containment commands, real host canary. No mocked core
path, no hard-coded answer, no simulated sandbox verdict.

### Links

- Live demo: http://149.28.225.246
- Repository: https://github.com/PhiBao/proofsheet
- Demo video: https://github.com/PhiBao/proofsheet/raw/main/docs/demo.mp4
- Example report: http://149.28.225.246/r/8b3dc46d-6892-47d1-b3f3-cf3c5e545912
- Containment drill: http://149.28.225.246/drill

## Tags

Agentic AI, AI Agents, Data Analysis, Sandboxing, gVisor, Rust, Vultr, Serverless
Inference, Verification, Developer Tools, CSV, Trust & Safety

## Track / pattern

Blast Radius Zero — Pattern A: sandboxed code execution.

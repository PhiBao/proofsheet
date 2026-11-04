# Lablab submission — Proofsheet

## Submission title

**Proofsheet — the analyst that shows its work** *(44 characters)*

## Demo video link

https://youtu.be/bdNI-FLgZxI

## Short description

**Drop a messy CSV. Proofsheet locks the business definition, executes the analysis in a disposable sandbox on Vultr, and returns a report where every number and every name carries a receipt.** *(189 characters)*

## Long description

Proofsheet is the analyst that shows its work.

Every AI data tool can make a chart. The hard part is defending the number: which definition was used, which rows were dropped, whether the model invented a figure. Proofsheet turns a messy CSV into an audit-ready report.

The user uploads an export, asks a question in plain English, and approves the business definition before any code runs: which orders count, whether shipping is revenue, how missing values, discounts, and ambiguous dates are handled. The definition is stored with the run, shown on every report, and reused on replay.

A Rust control plane profiles the file, calls Vultr Serverless Inference for a plan and a pandas script, and executes it in a disposable gVisor sandbox with no network, no secrets, a read-only root, and hard caps. A second model, which never sees the first script, recomputes the same metrics in a separate sandbox; both import the same deterministic parsing contract. Agreement produces the two-execution stamp. The note may only cite placeholders resolved from the execution ledger, and a chart audit checks the series, top category, peak month, assumptions and rows used. A live “try to make it lie” check injects a fake claim, and the stamp is withheld.

The result: every claim opens the exact code, stdout, sandbox and isolation level that produced it. A saved pipeline replays on a new export with no model in the loop, and Rust computes the period comparison. The production run: 522 rows in, 506 out, 333 used, 6/6 claims receipted, 5/5 independent agreement, +3.0% net revenue on replay.

Stack: Rust (Axum, Tokio, SQLx) on a Vultr Cloud Compute VM, Vultr Serverless Inference, gVisor, React + TypeScript. Track: Blast Radius Zero, Pattern A.

Live: http://149.28.225.246
Report: http://149.28.225.246/r/7263a5f0-e0be-47e1-9f6c-f29e23d3ffd8
Source: https://github.com/PhiBao/proofsheet

*(1,885 characters)*

## Tags

Agentic AI · AI Agents · Data Analysis · Sandboxing · gVisor · Rust · Vultr · Serverless Inference · Verification · Developer Tools · CSV · Trust & Safety

## Track / pattern

**Blast Radius Zero — Pattern A: sandboxed code execution.**

## Additional information

```
Track: Blast Radius Zero — Pattern A (sandboxed code execution).

Built with Rust (Axum, Tokio, SQLx), Vultr Serverless Inference, gVisor and React/TypeScript, deployed on a Vultr Cloud Compute VM in EWR.

Judges can verify the claims in about two minutes:
• Verified report — definition lock, chart, 6/6 receipted claims, 0 unverified, 5/5 independent agreement: http://149.28.225.246/r/7263a5f0-e0be-47e1-9f6c-f29e23d3ffd8
• Try to make it lie — inject a fake claim; the verifier withholds the stamp: same report, "Run adversarial check"
• Containment drill — 6/6 steps, host canary intact: http://149.28.225.246/drill
• Deterministic replay — 506 → 506 rows, +3.0% net revenue, no model calls: http://149.28.225.246/r/e45ea4dc-feea-476c-b854-fe1ea1e955a5

No login required. Source: https://github.com/PhiBao/proofsheet
Demo video: https://youtu.be/bdNI-FLgZxI
```

*(863 characters.)*

## Assets

| Asset | File |
| --- | --- |
| 16:9 cover image | `docs/cover.png` |
| Slide deck (PDF, 10 pages) | `docs/proofsheet-deck.pdf` |
| Slide presentation video (3:13) | `docs/slide-presentation.mp4` |
| Product demo video (2:09) | https://youtu.be/bdNI-FLgZxI |
| Product demo upload file | `docs/demo.mp4` |
| Product demo subtitles | `docs/demo.srt` |
| Slide video subtitles | `docs/slide-presentation.srt` |
| Live demo | http://149.28.225.246 |
| Repository | https://github.com/PhiBao/proofsheet |

## Upload order

1. Cover image: `docs/cover.png`
2. Slide presentation: `docs/proofsheet-deck.pdf`
3. Video presentation: `docs/slide-presentation.mp4`
4. Demo video: paste https://youtu.be/bdNI-FLgZxI (upload `docs/demo.mp4` only if the form requires a file).
5. Paste the title, short description, and long description above.
6. Add the tags, choose **Blast Radius Zero / Pattern A**, and paste the additional information above.

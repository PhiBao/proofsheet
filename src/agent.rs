use crate::app::AppState;
use crate::inference::{require_json, truncate, InferenceClient};
use crate::models::*;
use crate::profile::parse_number;
use crate::sandbox::SandboxSpec;
use crate::store;
use crate::verify;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const PLAN_SYSTEM: &str = r#"You are Proofsheet's planning engine. You receive a dataset profile and an analyst's question.
Produce a rigorous, minimal plan that a pandas script can execute deterministically.
Reply with ONE JSON object and nothing else, matching exactly:
{
  "title": "short report title",
  "summary": "one sentence describing the analysis",
  "cleaning": ["specific cleaning operation", "..."],
  "analysis": ["specific computation", "..."],
  "metrics": [{"id": "snake_case_id", "label": "Human label", "kind": "currency|number|percent|count|text"}],
  "chart": "what the chart shows"
}
Rules:
- 2 to 5 metrics. Each metric must be directly computable from the columns in the profile.
- Use "currency" for money, "percent" for rates expressed as a fraction (0.234 = 23.4%), "count" for row/order counts, "number" otherwise.
- Use "text" for a named entity the answer depends on, such as the top country or region. Its value is a string.
- Metric ids are lowercase snake_case, unique, and start with a letter.
- Cleaning must address the actual issues in the profile (duplicates, mixed dates, currency text, nulls, casing).
- Do not invent columns. Do not include prose outside the JSON."#;

const CODE_SYSTEM: &str = r#"You are Proofsheet's code generator. Output ONE Python code block (```python ... ```) and nothing else.
The script runs in an offline Linux sandbox as an unprivileged user, working directory /work.
Available: python3, pandas, numpy, matplotlib. No network, no pip, no shell commands.
It must exit 0, print progress lines, and finish by printing PROOFSHEET_OK.
Never access the network or any path outside /work. Never print secrets."#;

const NOTES_SYSTEM: &str = r#"You are Proofsheet's verification editor. You rewrite an analyst note so that every number is a receipted metric placeholder.
Rules:
- Output ONLY the rewritten Markdown. No preamble, no fences.
- Keep the same structure and meaning.
- The note must contain ZERO digit characters (0-9) outside placeholders. No dates, no quarters, no counts written as digits.
- Every number AND every named entity (country, region, product) must be written as {{m:<metric_id>}} using an id from the provided ledger.
- Never invent a number or a metric id."#;

pub async fn execute_ask(state: Arc<AppState>, run_id: String) {
    if let Err(e) = ask_inner(&state, &run_id).await {
        fail_run(&state, &run_id, &format!("{e:#}")).await;
    }
}

async fn ask_inner(state: &AppState, run_id: &str) -> Result<()> {
    let mut run = store::get_run(&state.db, run_id)
        .await?
        .ok_or_else(|| anyhow!("run not found"))?;
    let dataset = store::get_dataset(&state.db, &run.dataset_id)
        .await?
        .ok_or_else(|| anyhow!("dataset not found"))?;

    let input_path = state
        .cfg
        .data_dir
        .join("uploads")
        .join(format!("{}.csv", dataset.id));
    if !input_path.exists() {
        bail!("the uploaded file is missing on disk");
    }

    // ---------- Plan ----------
    run.status = "planning".into();
    push_log(state, &mut run, "info", &format!("Planner: {}", state.cfg.model_plan)).await?;
    let plan_prompt = plan_prompt(&dataset, &run.question);
    let completion = state
        .inference
        .chat(&state.cfg.model_plan, PLAN_SYSTEM, &plan_prompt, 0.1, 3200)
        .await
        .context("planning call failed")?;
    push_log(
        state,
        &mut run,
        "meta",
        &format!(
            "{} tokens in / {} out · {}",
            completion.prompt_tokens, completion.completion_tokens, completion.model
        ),
    )
    .await?;

    let plan = parse_plan(&completion.text).or_else(|first_err| {
        // one synchronous retry is handled by the caller loop below
        Err(first_err)
    });
    let plan = match plan {
        Ok(p) => p,
        Err(e) => {
            push_log(state, &mut run, "warn", &format!("Planner returned invalid JSON: {e}")).await?;
            let retry_prompt = format!(
                "{}\n\nYour previous reply was invalid: {e}\nReturn only the JSON object.",
                plan_prompt
            );
            let c = state
                .inference
                .chat(&state.cfg.model_plan, PLAN_SYSTEM, &retry_prompt, 0.0, 3200)
                .await?;
            parse_plan(&c.text)?
        }
    };
    run.plan = Some(plan.clone());
    run.status = "coding".into();
    push_log(
        state,
        &mut run,
        "ok",
        &format!("Plan: {} ({} metrics)", plan.title, plan.metrics.len()),
    )
    .await?;

    // ---------- Code + execute + repair ----------
    let mut code = generate_code(state, &mut run, &dataset, &plan).await?;
    let workdir = state.cfg.data_dir.join("work").join(run_id);
    std::fs::create_dir_all(&workdir)?;

    let mut last_error = String::new();
    let attempts = state.cfg.max_repair_attempts.max(1);

    for attempt in 1..=attempts {
        run.attempt = attempt as i64;
        run.code = Some(code.clone());
        run.status = if attempt == 1 { "executing".into() } else { "repairing".into() };
        push_log(
            state,
            &mut run,
            "info",
            &format!("Sandbox run · attempt {attempt}/{attempts}"),
        )
        .await?;

        // Fresh workspace per attempt: lifecycle discipline, no state carry-over.
        let attempt_dir = workdir.join(format!("attempt-{attempt}"));
        if attempt_dir.exists() {
            let _ = std::fs::remove_dir_all(&attempt_dir);
        }
        std::fs::create_dir_all(&attempt_dir)?;
        std::fs::copy(&input_path, attempt_dir.join("input.csv"))?;
        std::fs::write(attempt_dir.join("script.py"), &code)?;

        let spec = SandboxSpec {
            label: format!("ask-{run_id}-{attempt}"),
            workdir: attempt_dir.clone(),
            command: vec!["python3".into(), "/work/script.py".into()],
            timeout: Duration::from_secs(state.cfg.sandbox_timeout_secs),
            memory: None,
            cpus: None,
            network: false,
            read_only_workdir: false,
        };
        let result = state.sandbox.run(spec).await?;
        run.stdout = result.stdout.clone();
        run.stderr = result.stderr.clone();
        run.exit_code = result.exit_code;
        run.duration_ms = Some(result.duration_ms);
        run.sandbox_id = Some(result.sandbox_id.clone());
        run.isolation = Some(result.isolation.clone());
        run.oom_killed = result.oom_killed;

        push_log(
            state,
            &mut run,
            if result.exit_code == Some(0) { "ok" } else { "warn" },
            &format!(
                "{} exited {:?} in {} ms · destroyed={}",
                result.sandbox_id, result.exit_code, result.duration_ms, result.destroyed
            ),
        )
        .await?;

        if result.timed_out || result.exit_code != Some(0) || result.oom_killed {
            last_error = if result.oom_killed {
                "sandbox was OOM-killed (memory cap)".to_string()
            } else if result.timed_out {
                format!("sandbox exceeded the {}s wall-clock limit", state.cfg.sandbox_timeout_secs)
            } else {
                format!(
                    "script exited {:?}. stderr: {}",
                    result.exit_code,
                    truncate(result.stderr.trim(), 1400)
                )
            };
            push_log(state, &mut run, "error", &last_error).await?;
            if attempt < attempts {
                code = repair_code(state, &mut run, &dataset, &plan, &code, &last_error).await?;
                continue;
            }
            break;
        }

        // ---------- Artifacts ----------
        match read_artifacts(&attempt_dir, &plan, &dataset.profile) {
            Ok((metrics, template, clean_report)) => {
                run.metrics = metrics;
                run.clean_report = Some(clean_report);
                run.template = Some(template.clone());
                run.status = "verifying".into();
                let metric_count = run.metrics.len();
                push_log(
                    state,
                    &mut run,
                    "ok",
                    &format!("Execution produced {metric_count} receipted metrics"),
                )
                .await?;

                let mut flags = verify::verify_template(&template, &run.metrics);
                let mut final_template = template;
                let mut verifications = 0;
                while !flags.is_empty() && verifications < 2 {
                    push_log(
                        state,
                        &mut run,
                        "warn",
                        &format!(
                            "Verifier found {} unreceipted claim(s); rewriting the note",
                            flags.len()
                        ),
                    )
                    .await?;
                    final_template =
                        repair_template(state, &mut run, &final_template, &flags).await?;
                    flags = verify::verify_template(&final_template, &run.metrics);
                    verifications += 1;
                }
                run.template = Some(final_template.clone());
                run.claims = flags.clone();
                let (rendered, receipted) = verify::render(&final_template, &run.metrics);
                run.notes_markdown = Some(rendered);

                if flags.is_empty() {
                    push_log(
                        state,
                        &mut run,
                        "ok",
                        &format!("Verified: {receipted} claim(s), 0 unverified numbers"),
                    )
                    .await?;
                } else {
                    push_log(
                        state,
                        &mut run,
                        "warn",
                        &format!("{} claim(s) remain unverified and are flagged in the report", flags.len()),
                    )
                    .await?;
                }

                copy_artifacts(state, run_id, &attempt_dir).await?;
                run.status = "complete".into();
                run.completed_at = Some(Utc::now().to_rfc3339());
                run.error = None;
                push_log(state, &mut run, "ok", "Report complete").await?;
                store::save_run(&state.db, &run).await?;
                return Ok(());
            }
            Err(e) => {
                last_error = format!("missing or invalid artifacts: {e}");
                push_log(state, &mut run, "error", &last_error).await?;
                if attempt < attempts {
                    code = repair_code(state, &mut run, &dataset, &plan, &code, &last_error).await?;
                    continue;
                }
                break;
            }
        }
    }

    run.status = "failed".into();
    run.error = Some(last_error.clone());
    run.completed_at = Some(Utc::now().to_rfc3339());
    push_log(state, &mut run, "error", "Run failed after all repair attempts").await?;
    store::save_run(&state.db, &run).await?;
    Ok(())
}

async fn generate_code(
    state: &AppState,
    run: &mut Run,
    dataset: &Dataset,
    plan: &Plan,
) -> Result<String> {
    push_log(
        state,
        run,
        "info",
        &format!("Code generator: {}", state.cfg.model_code),
    )
    .await?;
    let prompt = code_prompt(dataset, plan);
    let completion = state
        .inference
        .chat(&state.cfg.model_code, CODE_SYSTEM, &prompt, 0.1, 9000)
        .await
        .context("code generation failed")?;
    push_log(
        state,
        run,
        "meta",
        &format!(
            "{} tokens in / {} out · {}",
            completion.prompt_tokens, completion.completion_tokens, completion.model
        ),
    )
    .await?;
    InferenceClient::extract_python(&completion.text)
        .ok_or_else(|| anyhow!("the model did not return a python code block"))
}

async fn repair_code(
    state: &AppState,
    run: &mut Run,
    dataset: &Dataset,
    plan: &Plan,
    code: &str,
    error: &str,
) -> Result<String> {
    push_log(state, run, "info", "Requesting a repair from the code model").await?;
    let prompt = format!(
        "The script failed inside the sandbox.\n\nFAILURE:\n{error}\n\nPREVIOUS SCRIPT:\n```python\n{code}\n```\n\nDATASET PROFILE:\n{}\n\nPLAN:\n{}\n\nFix the script. Keep the artifact contract identical. Output only the corrected python code block.",
        compact_profile(dataset),
        serde_json::to_string(plan)?
    );
    let completion = state
        .inference
        .chat(&state.cfg.model_code, CODE_SYSTEM, &prompt, 0.15, 9000)
        .await
        .context("repair call failed")?;
    push_log(
        state,
        run,
        "meta",
        &format!("{} tokens out · {}", completion.completion_tokens, completion.model),
    )
    .await?;
    InferenceClient::extract_python(&completion.text)
        .ok_or_else(|| anyhow!("the repair did not return a python code block"))
}

async fn repair_template(
    state: &AppState,
    run: &mut Run,
    template: &str,
    flags: &[ClaimFlag],
) -> Result<String> {
    let ledger: Vec<Value> = run
        .metrics
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "label": m.label,
                "kind": m.kind,
                "value": m.value,
                "formatted": verify::format_value(m)
            })
        })
        .collect();
    let violations: Vec<String> = flags.iter().map(|f| f.detail.clone()).collect();
    let prompt = format!(
        "METRIC LEDGER (the only allowed numbers):\n{}\n\nCURRENT NOTE:\n{}\n\nVIOLATIONS:\n- {}\n\nRewrite the note. Zero digit characters outside placeholders. Every number must be a placeholder from the ledger.",
        serde_json::to_string_pretty(&ledger)?,
        template,
        violations.join("\n- ")
    );
    let completion = state
        .inference
        .chat(&state.cfg.model_repair, NOTES_SYSTEM, &prompt, 0.2, 2600)
        .await
        .context("template repair failed")?;
    push_log(
        state,
        run,
        "meta",
        &format!("{} tokens out · {}", completion.completion_tokens, completion.model),
    )
    .await?;
    Ok(strip_markdown_fences(completion.text.trim()))
}

fn strip_markdown_fences(s: &str) -> String {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```markdown") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = s.strip_prefix("```md") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim_end_matches("```").trim().to_string();
    }
    s.to_string()
}

fn read_artifacts(
    dir: &PathBuf,
    plan: &Plan,
    profile: &Profile,
) -> Result<(Vec<Metric>, String, CleanReport)> {
    let metrics_raw = std::fs::read_to_string(dir.join("metrics.json"))
        .context("metrics.json was not written")?;
    let metrics = parse_metrics(&metrics_raw, plan)?;

    let template = std::fs::read_to_string(dir.join("notes.md"))
        .context("notes.md was not written")?;
    if template.trim().is_empty() {
        bail!("notes.md is empty");
    }

    let cleaned_path = dir.join("cleaned.csv");
    let cleaned = std::fs::metadata(&cleaned_path).context("cleaned.csv was not written")?;
    if cleaned.len() == 0 {
        bail!("cleaned.csv is empty");
    }
    let clean_report = crate::profile::build_clean_report(profile, &cleaned_path)
        .context("could not re-profile the cleaned file")?;
    let chart = std::fs::metadata(dir.join("chart.png")).context("chart.png was not written")?;
    if chart.len() < 5000 {
        bail!("chart.png is missing or implausibly small");
    }

    // The orchestrator independently re-profiles the cleaned file. If the dirt
    // described in the profile is still there, the run is repaired, not trusted.
    let residual = crate::profile::audit_cleaned(&dir.join("cleaned.csv"), profile);
    if !residual.is_empty() {
        bail!("cleaned.csv still fails the residual-dirt audit: {}", residual.join("; "));
    }

    Ok((metrics, template, clean_report))
}

fn parse_metrics(raw: &str, plan: &Plan) -> Result<Vec<Metric>> {
    let value: Value = serde_json::from_str(raw).context("metrics.json is not valid JSON")?;
    let arr = value
        .get("metrics")
        .and_then(|m| m.as_array())
        .or_else(|| value.as_array())
        .ok_or_else(|| anyhow!("metrics.json must contain a 'metrics' array"))?;

    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .map(slug)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("a metric has no valid id"))?;
        let label = item
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();
        let kind = item
            .get("kind")
            .and_then(|v| v.as_str())
            .filter(|k| ["currency", "number", "percent", "count", "text"].contains(k))
            .unwrap_or("number")
            .to_string();

        if kind == "text" {
            let text = match item.get("value") {
                Some(Value::String(s)) => Some(s.trim().to_string()),
                _ => item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string()),
            }
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("text metric '{id}' does not have a string value"))?;
            out.push(Metric {
                id,
                label,
                kind,
                value: 0.0,
                text: Some(text),
            });
            continue;
        }

        let value = match item.get("value") {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => parse_number(s),
            _ => None,
        }
        .ok_or_else(|| anyhow!("metric '{id}' does not have a numeric value"))?;
        if !value.is_finite() {
            bail!("metric '{id}' is not finite");
        }
        if kind == "percent" && value.abs() > 1.5 {
            bail!("metric '{id}' is a percent but looks like a whole number ({value}); percent values must be fractions such as 0.234");
        }
        out.push(Metric {
            id,
            label,
            kind,
            value,
            text: None,
        });
    }
    if out.is_empty() {
        bail!("metrics.json contained no metrics");
    }
    // Every planned metric must be present and receipted.
    for spec in &plan.metrics {
        if !out.iter().any(|m| m.id == spec.id) {
            bail!("metric '{}' from the plan was not produced by execution", spec.id);
        }
    }
    Ok(out)
}

fn parse_plan(text: &str) -> Result<Plan> {
    let v = require_json(text)?;
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("Data analysis")
        .to_string();
    let summary = v
        .get("summary")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let strings = |key: &str| -> Vec<String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut metrics: Vec<MetricSpec> = Vec::new();
    if let Some(arr) = v.get("metrics").and_then(|x| x.as_array()) {
        for m in arr {
            let id = m
                .get("id")
                .and_then(|x| x.as_str())
                .map(slug)
                .unwrap_or_default();
            if id.is_empty() || metrics.iter().any(|x| x.id == id) {
                continue;
            }
            let kind = m
                .get("kind")
                .and_then(|x| x.as_str())
                .filter(|k| ["currency", "number", "percent", "count", "text"].contains(k))
                .unwrap_or("number")
                .to_string();
            metrics.push(MetricSpec {
                id: id.clone(),
                label: m
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                kind,
            });
        }
    }
    if metrics.is_empty() {
        bail!("plan contains no metrics");
    }
    if metrics.len() > 6 {
        metrics.truncate(6);
    }
    Ok(Plan {
        title,
        summary,
        cleaning: strings("cleaning"),
        analysis: strings("analysis"),
        metrics,
        chart: v
            .get("chart")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

fn slug(s: &str) -> String {
    let mut out: String = s
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("m_{out}")
    } else {
        out.chars().take(48).collect()
    }
}

fn compact_profile(d: &Dataset) -> String {
    let cols: Vec<Value> = d
        .profile
        .columns
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "kind": c.kind,
                "nulls": c.nulls,
                "unique": c.unique,
                "sample": c.sample,
                "values": c.values,
                "issues": c.issues
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "filename": d.filename,
        "delimiter": d.profile.delimiter,
        "rows": d.profile.rows,
        "duplicate_rows": d.profile.duplicate_rows,
        "columns": cols,
        "preview": d.profile.preview
    }))
    .unwrap_or_default()
}

fn plan_prompt(d: &Dataset, question: &str) -> String {
    format!(
        "DATASET PROFILE:\n{}\n\nANALYST QUESTION:\n{}\n\nReturn the plan JSON.",
        compact_profile(d),
        question
    )
}

fn code_prompt(d: &Dataset, plan: &Plan) -> String {
    let preview = d
        .profile
        .preview
        .iter()
        .take(6)
        .map(|r| r.join(" | "))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r###"DATASET PROFILE (computed in Rust, authoritative):
{profile}

FIRST ROWS (raw, note the dirt):
{preview}

PLAN TO EXECUTE:
{plan}

ARTIFACT CONTRACT — write all four files into /work:
1. cleaned.csv — cleaned dataset, UTF-8 with header, no index. Preserve rows unless the plan says dedupe/drop invalid.
2. metrics.json — exactly {{"metrics":[{{"id":"<plan id>","label":"<label>","kind":"currency|number|percent|count|text","value":<number or string>}}]}}
   - exactly the plan's metric ids, each once; numbers must be JSON numbers and finite; percent as a fraction (0.234).
   - a "text" metric's value is a JSON string (for example the name of the top country).
3. chart.png — matplotlib, Agg, figsize=(7,4), dpi=200, white facecolor, tight_layout, readable labels, informative title. No chartjunk.
4. notes.md — Markdown note, max 200 words, sections "## What the data says", "## What to do", "## Caveats".
   - EVERY number must be a placeholder: {{{{m:<metric_id>}}}}
   - EVERY named entity that comes from the data (country, region, product) must also be a placeholder.
   - ZERO digit characters (0-9) outside placeholders. Write "the second quarter", never "Q2". No dates as digits.
   - Mention the cleaning performed and one honest caveat. The caveat must describe what your script ACTUALLY does (exclude vs fill); do not claim an operation you did not implement. Never invent a number.

SCRIPT RULES:
- Read with pandas.read_csv('/work/input.csv', dtype=str, keep_default_na=False, encoding='utf-8-sig').
- Parse numbers explicitly: strip $ € £ ¥ ₹ and thousands separators, treat (123) as negative, strip % when appropriate.
- Parse dates with an explicit ordered list of formats (strptime). Do not rely on dateutil inference. Count unparseable rows.
- Deduplicate exact rows before aggregating when duplicates exist.
- If a value required for a metric is missing or unparseable, EXCLUDE that row from the metric and print the excluded count. Do not silently impute zeros or ones.
- Sort periods chronologically. Coerce unexpected values with errors='coerce' and count them.
- If a text column has a `values` list, map variants that differ only by case, whitespace or punctuation to ONE canonical label.
- After cleaning, the orchestrator re-profiles cleaned.csv in Rust. The run FAILS if a categorical column still contains case/punctuation aliases, if a numeric column is still under 95% parseable, or if a date column still contains unparseable values. Clean until the audit passes.
- matplotlib.use('Agg') before pyplot.
- Print a few progress lines, then PROOFSHEET_OK.

Output only the python code block."###,
        profile = compact_profile(d),
        preview = preview,
        plan = serde_json::to_string_pretty(plan).unwrap_or_default()
    )
}

async fn read_canary(path: &std::path::Path) -> bool {
    tokio::fs::read_to_string(path).await.is_ok()
}

async fn push_log(state: &AppState, run: &mut Run, level: &str, text: &str) -> Result<()> {
    run.log.push(LogLine {
        at: Utc::now().to_rfc3339(),
        level: level.to_string(),
        text: text.to_string(),
    });
    if run.log.len() > 120 {
        let drain = run.log.len() - 120;
        run.log.drain(0..drain);
    }
    store::save_run(&state.db, run).await?;
    Ok(())
}

async fn fail_run(state: &AppState, run_id: &str, error: &str) {
    if let Ok(Some(mut run)) = store::get_run(&state.db, run_id).await {
        run.status = "failed".into();
        run.error = Some(error.to_string());
        run.completed_at = Some(Utc::now().to_rfc3339());
        let _ = push_log(state, &mut run, "error", &truncate(error, 1200)).await;
        let _ = read_canary(&state.canary_path).await;
    }
}

async fn copy_artifacts(state: &AppState, run_id: &str, from: &std::path::Path) -> Result<()> {
    let dest = store::artifact_dir(&state.cfg.data_dir, run_id).await?;
    for name in [
        "cleaned.csv",
        "chart.png",
        "metrics.json",
        "notes.md",
        "clean_report.json",
        "script.py",
    ] {
        let src = from.join(name);
        if src.exists() {
            std::fs::copy(&src, dest.join(name))?;
        }
    }
    Ok(())
}

/// Deterministic replay: no model in the loop. Reuse the exact script and template.
pub async fn execute_replay(state: Arc<AppState>, run_id: String) {
    if let Err(e) = replay_inner(&state, &run_id).await {
        fail_run(&state, &run_id, &format!("{e:#}")).await;
    }
}

async fn replay_inner(state: &AppState, run_id: &str) -> Result<()> {
    let mut run = store::get_run(&state.db, run_id)
        .await?
        .ok_or_else(|| anyhow!("run not found"))?;
    let source_id = run
        .replayed_from
        .clone()
        .ok_or_else(|| anyhow!("replay has no source"))?;
    let source = store::get_run(&state.db, &source_id)
        .await?
        .ok_or_else(|| anyhow!("source run not found"))?;
    let dataset = store::get_dataset(&state.db, &run.dataset_id)
        .await?
        .ok_or_else(|| anyhow!("dataset not found"))?;

    let code = source
        .code
        .clone()
        .ok_or_else(|| anyhow!("source run has no code"))?;
    let plan = source
        .plan
        .clone()
        .ok_or_else(|| anyhow!("source run has no plan"))?;
    let template = source
        .template
        .clone()
        .ok_or_else(|| anyhow!("source run has no template"))?;

    run.plan = Some(plan.clone());
    run.code = Some(code.clone());
    run.template = Some(template.clone());
    run.status = "executing".into();
    run.attempt = 1;
    push_log(
        state,
        &mut run,
        "info",
        &format!("Deterministic replay of {source_id} · no model in the loop"),
    )
    .await?;

    let input_path = state
        .cfg
        .data_dir
        .join("uploads")
        .join(format!("{}.csv", dataset.id));
    let workdir = state.cfg.data_dir.join("work").join(run_id).join("attempt-1");
    if workdir.exists() {
        let _ = std::fs::remove_dir_all(&workdir);
    }
    std::fs::create_dir_all(&workdir)?;
    std::fs::copy(&input_path, workdir.join("input.csv"))?;
    std::fs::write(workdir.join("script.py"), &code)?;

    let spec = SandboxSpec {
        label: format!("replay-{run_id}"),
        workdir: workdir.clone(),
        command: vec!["python3".into(), "/work/script.py".into()],
        timeout: Duration::from_secs(state.cfg.sandbox_timeout_secs),
        memory: None,
        cpus: None,
        network: false,
        read_only_workdir: false,
    };
    let result = state.sandbox.run(spec).await?;
    run.stdout = result.stdout.clone();
    run.stderr = result.stderr.clone();
    run.exit_code = result.exit_code;
    run.duration_ms = Some(result.duration_ms);
    run.sandbox_id = Some(result.sandbox_id.clone());
    run.isolation = Some(result.isolation.clone());
    push_log(
        state,
        &mut run,
        if result.exit_code == Some(0) { "ok" } else { "error" },
        &format!("Replay exited {:?}", result.exit_code),
    )
    .await?;

    if result.exit_code != Some(0) || result.timed_out {
        bail!(
            "the saved pipeline no longer matches this file (schema drift or dirty values). stderr: {}",
            truncate(result.stderr.trim(), 900)
        );
    }

    let (metrics, _artifact_template, clean_report) = read_artifacts(&workdir, &plan, &dataset.profile)?;
    run.metrics = metrics;
    run.clean_report = Some(clean_report);
    // The repaired template from the original run is authoritative: the generated
    // script may still emit the pre-repair note, and replay must not resurrect it.
    run.template = Some(template.clone());
    let flags = verify::verify_template(&template, &run.metrics);
    run.claims = flags;
    let (rendered, receipted) = verify::render(&template, &run.metrics);
    run.notes_markdown = Some(rendered);
    let flag_count = run.claims.len();
    push_log(
        state,
        &mut run,
        "ok",
        &format!("Replay verified: {receipted} claims, {flag_count} flags"),
    )
    .await?;

    copy_artifacts(state, run_id, &workdir).await?;
    run.status = "complete".into();
    run.completed_at = Some(Utc::now().to_rfc3339());
    store::save_run(&state.db, &run).await?;
    Ok(())
}

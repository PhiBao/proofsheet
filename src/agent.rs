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
Produce a rigorous, minimal plan that a pandas script can execute deterministically, plus the business definition the user must approve before execution.
Reply with ONE JSON object and nothing else, matching exactly:
{
  "title": "short report title",
  "summary": "one sentence describing the analysis",
  "cleaning": ["specific cleaning operation", "..."],
  "analysis": ["specific computation", "..."],
  "metrics": [{"id": "snake_case_id", "label": "Human label", "kind": "currency|number|percent|count|text"}],
  "chart": "what the chart shows",
  "definition": {
    "summary": "one sentence stating the business definition",
    "assumptions": [
      {"key": "include_shipping", "label": "Include shipping in net revenue", "value": "false", "kind": "bool", "options": [], "why": "shipping is a pass-through, not revenue"},
      {"key": "status_scope", "label": "Orders included", "value": "completed,shipped", "kind": "enum", "options": ["completed,shipped", "all", "completed"], "why": "cancelled and refunded orders are not revenue"},
      {"key": "missing_values", "label": "Missing values", "value": "exclude", "kind": "enum", "options": ["exclude", "fill_zero"], "why": "a row without a metric input cannot be counted"},
      {"key": "date_order", "label": "Ambiguous dates", "value": "day_first", "kind": "enum", "options": ["day_first", "month_first", "auto"], "why": "day-first matches the export convention"}
    ]
  }
}
Rules:
- 2 to 5 metrics. Each metric is a SINGLE SCALAR FACT, one value. Never put a time series or a list in a metric; the chart_data series carries the trend.
- If the question asks for a monthly trend, include scalar metrics such as total revenue, peak month (text), peak-month revenue, and top country; do not create a "monthly revenue" scalar metric.
- Each metric must be directly computable from the columns in the profile.
- Use "currency" for money, "percent" for rates expressed as a fraction (0.234 = 23.4%), "count" for row/order counts, "number" otherwise.
- Use "text" for a named entity the answer depends on, such as the top country or region. Its value is a string.
- Metric ids are lowercase snake_case, unique, and start with a letter.
- Cleaning must address the actual issues in the profile (duplicates, mixed dates, currency text, nulls, casing).
- The definition is shown to the user for approval before any code runs. 3 to 6 assumptions, only choices that materially change the answer.
- State formulas precisely, for example `quantity * unit_price * (1 - discount)`, not "minus discount".
- Assumption keys are snake_case, values are strings. kind is bool | enum | text. An enum must have 2 to 4 options and its value must be one of them.
- Always state assumptions for missing values, ambiguous dates, duplicate rows, and any filter/status scope the metrics depend on.
- Do not invent columns. Do not include prose outside the JSON."#;

const CONTRACT: &str = include_str!("sandbox/contract.py");

const CHECKER_SYSTEM: &str = r#"You are Proofsheet's independent verifier. You receive a dataset profile, an approved business definition, and a metric contract. You do NOT see the analyst's implementation, and you must not ask for it.
Write ONE Python code block (```python ... ```) and nothing else.
The script runs in an offline Linux sandbox as an unprivileged user, working directory /work.
Read /work/input.csv and recompute the metrics independently, applying the approved definition exactly.
Import `contract` from /work and use its parsers (`contract.parse_money`, `contract.parse_quantity`, `contract.parse_discount`, `contract.parse_date`, `contract.normalize_status`, `contract.normalize_country`). Do not reimplement parsing.
For dates, call `contract.parse_date(value)`; the approved precedence is configured. Do not regex-match date formats and do not use `dayfirst=True` inference.
Available: python3, pandas, numpy. No network, no pip.
Write /work/check_metrics.json exactly:
{"metrics":[{"id":"<metric id>","label":"<label>","kind":"currency|number|percent|count|text","value":<number or string>}]}
Include every metric id in the contract exactly once. Percent values are fractions (0.234 for 23.4%). Text metrics are strings.
Do not write a chart. Do not read any other file. Print progress lines and finish with PROOFSHEET_OK.
Never access the network or any path outside /work. Never print secrets."#;

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

/// Execute a run whose plan and definition were already approved.
pub async fn execute_prepared(state: Arc<AppState>, run_id: String) {
    if let Err(e) = prepared_inner(&state, &run_id).await {
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

    if run.plan.is_none() || run.definition.is_none() {
        run.status = "planning".into();
        push_log(state, &mut run, "info", &format!("Planner: {}", state.cfg.model_plan)).await?;
        let (plan, definition) = generate_plan(state, &dataset, &run.question).await?;
        run.plan = Some(plan.clone());
        run.definition = Some(definition);
        store::save_run(&state.db, &run).await?;
        let assumption_count = run
            .definition
            .as_ref()
            .map(|d| d.assumptions.len())
            .unwrap_or(0);
        push_log(
            state,
            &mut run,
            "ok",
            &format!(
                "Plan: {} ({} metrics, {} assumptions)",
                plan.title,
                plan.metrics.len(),
                assumption_count
            ),
        )
        .await?;
    }

    execute_prepared_inner(state, &mut run, &dataset).await
}

async fn prepared_inner(state: &AppState, run_id: &str) -> Result<()> {
    let mut run = store::get_run(&state.db, run_id)
        .await?
        .ok_or_else(|| anyhow!("run not found"))?;
    let dataset = store::get_dataset(&state.db, &run.dataset_id)
        .await?
        .ok_or_else(|| anyhow!("dataset not found"))?;
    if run.plan.is_none() || run.definition.is_none() {
        bail!("this run has no approved plan and definition");
    }
    execute_prepared_inner(state, &mut run, &dataset).await
}

async fn execute_prepared_inner(
    state: &AppState,
    run: &mut Run,
    dataset: &Dataset,
) -> Result<()> {
    if state.run_slots.available_permits() == 0 {
        run.status = "queued".into();
        push_log(state, run, "info", "Waiting for a sandbox slot").await?;
    }
    let _slot = state
        .run_slots
        .acquire()
        .await
        .context("could not acquire a sandbox slot")?;

    let plan = run.plan.clone().ok_or_else(|| anyhow!("run has no plan"))?;
    let definition = run
        .definition
        .clone()
        .ok_or_else(|| anyhow!("run has no approved definition"))?;
    let run_id = run.id.clone();

    let input_path = state
        .cfg
        .data_dir
        .join("uploads")
        .join(format!("{}.csv", dataset.id));
    if !input_path.exists() {
        bail!("the uploaded file is missing on disk");
    }

    run.status = "coding".into();
    let mut code = generate_code(state, run, dataset, &plan, &definition).await?;
    let workdir = state.cfg.data_dir.join("work").join(&run_id);
    std::fs::create_dir_all(&workdir)?;

    let mut last_error = String::new();
    let attempts = state.cfg.max_repair_attempts.max(1);

    for attempt in 1..=attempts {
        run.attempt = attempt as i64;
        run.code = Some(code.clone());
        run.status = if attempt == 1 { "executing".into() } else { "repairing".into() };
        push_log(
            state,
            run,
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
        std::fs::write(attempt_dir.join("contract.py"), CONTRACT)?;
        std::fs::write(attempt_dir.join("contract_config.py"), contract_config(&definition))?;
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
            run,
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
            push_log(state, run, "error", &last_error).await?;
            if attempt < attempts {
                code = repair_code(state, run, dataset, &plan, &definition, &code, &last_error).await?;
                continue;
            }
            break;
        }

        // ---------- Artifacts ----------
        match read_artifacts(&attempt_dir, &plan, &dataset.profile, &definition) {
            Ok((metrics, template, clean_report, chart_data, chart_audit)) => {
                run.metrics = metrics;
                run.clean_report = Some(clean_report);
                run.chart_data = Some(chart_data);
                run.chart_audit = Some(chart_audit.clone());
                run.template = Some(template.clone());
                run.status = "verifying".into();
                let metric_count = run.metrics.len();
                push_log(
                    state,
                    run,
                    "ok",
                    &format!("Execution produced {metric_count} receipted metrics"),
                )
                .await?;
                push_log(
                    state,
                    run,
                    if chart_audit.passed { "ok" } else { "warn" },
                    &format!(
                        "Chart audit: {}/{} checks passed",
                        chart_audit.checks.iter().filter(|c| c.passed).count(),
                        chart_audit.checks.len()
                    ),
                )
                .await?;

                let mut flags = verify::verify_template(&template, &run.metrics);
                let mut final_template = template;
                let mut verifications = 0;
                while !flags.is_empty() && verifications < 2 {
                    push_log(
                        state,
                        run,
                        "warn",
                        &format!(
                            "Verifier found {} unreceipted claim(s); rewriting the note",
                            flags.len()
                        ),
                    )
                    .await?;
                    final_template =
                        repair_template(state, run, &final_template, &flags).await?;
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
                        run,
                        "ok",
                        &format!("Verified: {receipted} claim(s), 0 unverified numbers"),
                    )
                    .await?;
                } else {
                    push_log(
                        state,
                        run,
                        "warn",
                        &format!("{} claim(s) remain unverified and are flagged in the report", flags.len()),
                    )
                    .await?;
                }

                // ---------- Independent cross-check ----------
                run.status = "checking".into();
                push_log(state, run, "info", "Starting the independent checker execution").await?;
                let consensus =
                    run_consensus(state, run, dataset, &plan, &definition).await?;
                match consensus.status.as_str() {
                    "agree" => push_log(
                        state,
                        run,
                        "ok",
                        "Independent implementation agrees with the primary ledger",
                    )
                    .await?,
                    "disagree" => push_log(
                        state,
                        run,
                        "warn",
                        "Independent implementation disagrees; the report will be flagged",
                    )
                    .await?,
                    other => push_log(
                        state,
                        run,
                        "warn",
                        &format!("Independent check did not complete ({other})"),
                    )
                    .await?,
                }
                run.consensus = Some(consensus);

                copy_artifacts(state, &run_id, &attempt_dir).await?;
                run.status = "complete".into();
                run.completed_at = Some(Utc::now().to_rfc3339());
                run.error = None;
                push_log(state, run, "ok", "Report complete").await?;
                store::save_run(&state.db, run).await?;
                return Ok(());
            }
            Err(e) => {
                last_error = format!("missing or invalid artifacts: {e}");
                push_log(state, run, "error", &last_error).await?;
                if attempt < attempts {
                    code = repair_code(state, run, dataset, &plan, &definition, &code, &last_error).await?;
                    continue;
                }
                break;
            }
        }
    }

    run.status = "failed".into();
    run.error = Some(last_error.clone());
    run.completed_at = Some(Utc::now().to_rfc3339());
    push_log(state, run, "error", "Run failed after all repair attempts").await?;
    store::save_run(&state.db, run).await?;
    Ok(())
}

/// Plan a question without executing it. The result is shown to the user for approval.
pub async fn generate_plan(
    state: &AppState,
    dataset: &Dataset,
    question: &str,
) -> Result<(Plan, Definition)> {
    let plan_prompt = plan_prompt(dataset, question);
    let completion = state
        .inference
        .chat(&state.cfg.model_plan, PLAN_SYSTEM, &plan_prompt, 0.1, 3600)
        .await
        .context("planning call failed")?;
    match parse_plan(&completion.text) {
        Ok((plan, definition)) => Ok((plan, trim_definition(definition, &dataset.profile))),
        Err(e) => {
            let retry_prompt = format!(
                "{}\n\nYour previous reply was invalid: {e}\nReturn only the JSON object.",
                plan_prompt
            );
            let c = state
                .inference
                .chat(&state.cfg.model_plan, PLAN_SYSTEM, &retry_prompt, 0.0, 3600)
                .await
                .context("planning retry failed")?;
            parse_plan(&c.text)
                .map(|(plan, definition)| (plan, trim_definition(definition, &dataset.profile)))
                .map_err(|e2| anyhow!("planner returned invalid JSON: {e2}"))
        }
    }
}

async fn generate_code(
    state: &AppState,
    run: &mut Run,
    dataset: &Dataset,
    plan: &Plan,
    definition: &Definition,
) -> Result<String> {
    push_log(
        state,
        run,
        "info",
        &format!("Code generator: {}", state.cfg.model_code),
    )
    .await?;
    let prompt = code_prompt(dataset, plan, definition);
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
    definition: &Definition,
    code: &str,
    error: &str,
) -> Result<String> {
    push_log(state, run, "info", "Requesting a repair from the code model").await?;
    let prompt = format!(
        "The script failed or produced a degenerate result inside the sandbox.\n\nFAILURE:\n{error}\n\nPREVIOUS SCRIPT:\n```python\n{code}\n```\n\nDATASET PROFILE:\n{}\n\nPLAN:\n{}\n\nAPPROVED DEFINITION (binding):\n{}\n\nFix the root cause. Do not add a guard that returns zero or N/A; inspect the actual values and fix the parser or filter. `pd.Timestamp` does not accept a `format` argument. Keep the artifact contract identical. Output only the corrected python code block.",
        compact_profile(dataset),
        serde_json::to_string(plan)?,
        definition_prompt(definition)
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
    definition: &Definition,
) -> Result<(Vec<Metric>, String, CleanReport, ChartData, ChartAudit)> {
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

    let chart_raw = std::fs::read_to_string(dir.join("chart_data.json"))
        .context("chart_data.json was not written")?;
    let chart_data: ChartData =
        serde_json::from_str(&chart_raw).context("chart_data.json is invalid")?;
    let chart_audit = audit_chart(&chart_data, &metrics, definition, clean_report.rows_out);
    if !chart_audit.passed {
        let failed: Vec<String> = chart_audit
            .checks
            .iter()
            .filter(|c| !c.passed)
            .map(|c| c.detail.clone())
            .collect();
        bail!("chart data fails the ledger audit: {}", failed.join("; "));
    }

    // The orchestrator independently re-profiles the cleaned file. If the dirt
    // described in the profile is still there, the run is repaired, not trusted.
    let residual = crate::profile::audit_cleaned(&dir.join("cleaned.csv"), profile);
    if !residual.is_empty() {
        bail!("cleaned.csv still fails the residual-dirt audit: {}", residual.join("; "));
    }

    Ok((metrics, template, clean_report, chart_data, chart_audit))
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

    // A run that "succeeds" with an empty analysis is not a success. This is the
    // gate that stops a plausible-looking zero report from being stamped.
    let numeric: Vec<f64> = out
        .iter()
        .filter(|m| m.kind != "text")
        .map(|m| m.value)
        .collect();
    if !numeric.is_empty() && numeric.iter().all(|v| v.abs() < 1e-9) {
        bail!("execution produced only zero-valued metrics; a parser or filter dropped the data");
    }
    for m in out.iter().filter(|m| m.kind == "text") {
        let t = m.text.as_deref().unwrap_or("").trim().to_lowercase();
        if t.is_empty()
            || ["n/a", "na", "none", "null", "unknown", "nan", "-"]
                .contains(&t.as_str())
        {
            bail!("text metric '{}' has no usable value", m.id);
        }
    }
    // Every planned metric must be present and receipted.
    for spec in &plan.metrics {
        if !out.iter().any(|m| m.id == spec.id) {
            bail!("metric '{}' from the plan was not produced by execution", spec.id);
        }
    }
    Ok(out)
}

fn parse_plan(text: &str) -> Result<(Plan, Definition)> {
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
    let definition = parse_definition(v.get("definition"));
    Ok((
        Plan {
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
        },
        definition,
    ))
}

fn default_assumptions() -> Vec<Assumption> {
    vec![
        Assumption {
            key: "missing_values".into(),
            label: "Missing values".into(),
            value: "exclude".into(),
            kind: "enum".into(),
            options: vec!["exclude".into(), "fill_zero".into()],
            why: "A row without a metric input cannot be counted without inventing data.".into(),
        },
        Assumption {
            key: "date_order".into(),
            label: "Ambiguous dates".into(),
            value: "auto".into(),
            kind: "enum".into(),
            options: vec!["auto".into(), "day_first".into(), "month_first".into()],
            why: "Day/month order changes monthly totals when both values are under thirteen.".into(),
        },
        Assumption {
            key: "duplicates".into(),
            label: "Duplicate rows".into(),
            value: "remove_exact".into(),
            kind: "enum".into(),
            options: vec!["remove_exact".into(), "keep".into()],
            why: "Exact duplicates double-count totals if left in.".into(),
        },
        Assumption {
            key: "discount_parsing".into(),
            label: "Discount values".into(),
            value: "percent_to_fraction".into(),
            kind: "enum".into(),
            options: vec!["percent_to_fraction".into(), "as_decimal".into()],
            why: "A value like 10% is 0.10, while 0.10 is already a fraction. Ambiguity here changes revenue.".into(),
        },
    ]
}

pub fn parse_definition(v: Option<&Value>) -> Definition {
    let mut summary = String::new();
    let mut assumptions: Vec<Assumption> = Vec::new();
    if let Some(obj) = v {
        summary = obj
            .get("summary")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if let Some(arr) = obj.get("assumptions").and_then(|x| x.as_array()) {
            for a in arr.iter().take(8) {
                let key = a
                    .get("key")
                    .and_then(|x| x.as_str())
                    .map(slug)
                    .unwrap_or_default();
                if key.is_empty() || assumptions.iter().any(|x| x.key == key) {
                    continue;
                }
                let mut kind = a
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .filter(|k| ["bool", "enum", "text"].contains(k))
                    .unwrap_or("text")
                    .to_string();
                let options: Vec<String> = a
                    .get("options")
                    .and_then(|x| x.as_array())
                    .map(|o| {
                        o.iter()
                            .filter_map(|s| s.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .take(4)
                            .collect()
                    })
                    .unwrap_or_default();
                let mut value = a
                    .get("value")
                    .map(|x| match x {
                        Value::String(s) => s.trim().to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                if kind == "bool" {
                    value = if value.eq_ignore_ascii_case("true") {
                        "true".into()
                    } else {
                        "false".into()
                    };
                } else if kind == "enum" {
                    if options.is_empty() {
                        kind = "text".into();
                    } else if !options.iter().any(|o| o.eq_ignore_ascii_case(&value)) {
                        value = options[0].clone();
                    } else {
                        value = options
                            .iter()
                            .find(|o| o.eq_ignore_ascii_case(&value))
                            .cloned()
                            .unwrap_or(value);
                    }
                }
                if value.is_empty() {
                    value = options.first().cloned().unwrap_or_else(|| "—".into());
                }
                let label = a
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&key)
                    .trim()
                    .to_string();
                assumptions.push(Assumption {
                    key,
                    label,
                    value,
                    kind,
                    options,
                    why: a
                        .get("why")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                });
            }
        }
    }
    // Merge the generic defaults only when the planner did not already cover
    // the same decision under a different key.
    let covers = |key: &str, assumptions: &[Assumption]| {
        assumptions.iter().any(|a| {
            a.key == key
                || (key == "duplicates" && a.key.contains("duplicat"))
                || (key == "missing_values" && a.key.contains("missing"))
                || (key == "date_order" && a.key.contains("date"))
                || (key == "currency" && a.key.contains("currency"))
        })
    };
    let mut added_defaults: Vec<String> = Vec::new();
    for d in default_assumptions() {
        if !covers(&d.key, &assumptions) {
            added_defaults.push(d.key.clone());
            assumptions.push(d);
        }
    }
    // Keep the approval panel focused.
    while assumptions.len() > 8 {
        let pos = assumptions
            .iter()
            .rposition(|a| added_defaults.contains(&a.key))
            .or_else(|| assumptions.len().checked_sub(1));
        let Some(pos) = pos else { break };
        assumptions.remove(pos);
    }
    if summary.is_empty() {
        summary = "Approved business assumptions for this analysis.".into();
    }
    Definition {
        summary,
        assumptions,
    }
}

fn trim_definition(mut definition: Definition, profile: &Profile) -> Definition {
    // Date precedence is derived from the profiler, not guessed by a model.
    // Ambiguous slash/dash dates must be resolved identically by both executions.
    if !definition.assumptions.iter().any(|a| a.key == "date_parsing") {
        if let Some(date_col) = profile.columns.iter().find(|c| !c.date_formats.is_empty()) {
            definition.assumptions.push(Assumption {
                key: "date_parsing".into(),
                label: "Date format precedence".into(),
                value: date_col.date_formats.join(" > "),
                kind: "text".into(),
                options: vec![],
                why: "The first matching format wins. This is the only permitted date parser order for both executions.".into(),
            });
        }
    }
    let names: Vec<String> = profile
        .columns
        .iter()
        .map(|c| c.name.to_lowercase())
        .collect();
    let has = |needle: &str| names.iter().any(|n| n.contains(needle));
    let has_shipping = has("shipping") || has("freight");
    let has_discount = has("discount");
    let has_status = has("status") || has("state");
    definition.assumptions.retain(|a| match a.key.as_str() {
        "include_shipping" => has_shipping,
        "discount_parsing" | "discount_missing" | "discount_values" => has_discount,
        "status_scope" | "order_status" => has_status,
        _ => true,
    });
    if definition.assumptions.is_empty() {
        definition.assumptions = default_assumptions();
    }
    while definition.assumptions.len() > 8 {
        definition.assumptions.pop();
    }
    definition
}

fn contract_config(definition: &Definition) -> String {
    let precedence: Vec<String> = definition
        .assumptions
        .iter()
        .find(|a| a.key == "date_parsing")
        .map(|a| {
            a.value
                .split('>')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let discount = definition
        .assumptions
        .iter()
        .find(|a| a.key == "discount_parsing")
        .map(|a| a.value.clone())
        .unwrap_or_else(|| "percent_to_fraction".into());
    format!(
        "DATE_PRECEDENCE = {}\nDISCOUNT_MODE = {:?}\n",
        serde_json::to_string(&precedence).unwrap_or_else(|_| "[]".into()),
        discount
    )
}

fn definition_prompt(definition: &Definition) -> String {
    let mut out = definition.summary.clone();
    for a in &definition.assumptions {
        out.push_str(&format!(
            "\n- {} = {}  ({}; {})",
            a.key, a.value, a.label, a.why
        ));
    }
    out
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

fn redact_sample(column: &str, value: &str) -> String {
    let col = column.to_lowercase();
    if value.contains('@') || col.contains("email") {
        return "***@***".into();
    }
    if ["name", "customer", "address", "phone", "contact", "person"]
        .iter()
        .any(|k| col.contains(k))
    {
        let words: Vec<&str> = value.split_whitespace().collect();
        if words.len() >= 2 {
            return format!(
                "{}*** ***",
                words[0].chars().next().unwrap_or('*')
            );
        }
        return "***".into();
    }
    let digits = value.chars().filter(|c| c.is_ascii_digit()).count();
    let chars = value.chars().count().max(1);
    if digits >= 7 && digits * 100 / chars > 60 {
        return "***".into();
    }
    truncate(value, 32)
}

fn value_shape(c: &ColumnProfile) -> String {
    match c.kind.as_str() {
        "currency" => "money text with symbols and/or separators".into(),
        "percent" => "percentage text".into(),
        "date" => {
            if c.issues.iter().any(|i| i.contains("mixed date")) {
                "date text in several formats".into()
            } else {
                "date text".into()
            }
        }
        "number" => {
            if c.nulls > 0 {
                "numeric text with some empty/invalid values".into()
            } else {
                "numeric text".into()
            }
        }
        "text" if !c.values.is_empty() => {
            format!("categorical text, {} distinct values", c.unique)
        }
        "text" => "free text".into(),
        "empty" => "empty column".into(),
        other => other.into(),
    }
}

fn compact_profile(d: &Dataset) -> String {
    let cols: Vec<Value> = d
        .profile
        .columns
        .iter()
        .map(|c| {
            // Free-text samples are not needed to write the code and are the most
            // likely place for names, addresses or notes to leak. Omit them.
            let samples: Vec<String> = if c.kind == "text" && c.values.is_empty() {
                vec!["***".into()]
            } else {
                c.sample
                    .iter()
                    .take(3)
                    .map(|v| redact_sample(&c.name, v))
                    .collect()
            };
            json!({
                "name": c.name,
                "kind": c.kind,
                "shape": value_shape(c),
                "nulls": c.nulls,
                "unique": c.unique,
                "sample": samples,
                "values": c.values.iter().take(12).map(|v| redact_sample(&c.name, v)).collect::<Vec<_>>(),
                "issues": c.issues
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "filename": d.filename,
        "delimiter": d.profile.delimiter,
        "rows": d.profile.rows,
        "duplicate_rows": d.profile.duplicate_rows,
        "columns": cols
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

fn code_prompt(d: &Dataset, plan: &Plan, definition: &Definition) -> String {
    format!(
        r###"DATASET PROFILE (computed in Rust; samples are redacted):
{profile}

PLAN TO EXECUTE:
{plan}

APPROVED DEFINITION (binding — the user approved these before execution):
{definition}

ARTIFACT CONTRACT — write all five files into /work:
1. cleaned.csv — cleaned dataset, UTF-8 with header, no index. Preserve rows unless the definition says dedupe/drop invalid.
2. metrics.json — exactly {{"metrics":[{{"id":"<plan id>","label":"<label>","kind":"currency|number|percent|count|text","value":<number or string>}}]}}
   - exactly the plan's metric ids, each once; numbers must be JSON numbers and finite; percent as a fraction (0.234).
   - a "text" metric's value is a JSON string (for example the name of the top country).
3. chart.png — matplotlib, Agg, figsize=(7,4), dpi=200, white facecolor, tight_layout, readable labels, informative title. No chartjunk.
4. chart_data.json — exactly:
   {{"metric_id":"<metric id the chart represents>","aggregation":"sum|trend|breakdown","unit":"currency|number|percent|count","series":[{{"label":"<period or category>","value":<number>}}],"top_category":"<optional category>","rows_used":<number of rows that contributed to the metrics>,"applied_assumptions":["<key>=<value>", ...]}}
   - "sum" and "breakdown" mean the series values add up to the metric. "trend" is a time series that need not sum.
   - rows_used is the number of rows that actually contributed to the metrics after cleaning and definition filters.
   - applied_assumptions MUST list every approved assumption exactly as key=value.
   - the series must come from the same cleaned/aggregated data as the metrics; never draw numbers that are not in the ledger.
5. notes.md — Markdown note, max 200 words, sections "## What the data says", "## What to do", "## Caveats".
   - EVERY number must be a placeholder: {{{{m:<metric_id>}}}}
   - EVERY named entity that comes from the data (country, region, product) must also be a placeholder.
   - ZERO digit characters (0-9) outside placeholders. Write "the second quarter", never "Q2". No dates as digits.
   - Mention the cleaning performed and one honest caveat. The caveat must describe what your script ACTUALLY does (exclude vs fill); do not claim an operation you did not implement. Never invent a number.

SCRIPT RULES:
- Apply every approved definition assumption exactly. If the plan and the definition conflict, the DEFINITION wins.
- The sandbox provides `/work/contract.py` and `/work/contract_config.py`. Import `contract` and use it for ALL parsing:
  `contract.parse_money(value)`, `contract.parse_quantity(value)`, `contract.parse_discount(value)`,
  `contract.parse_date(value)`, `contract.normalize_status(value)`, `contract.normalize_country(value)`.
  The approved date precedence and discount mode are already configured in the sandbox.
  Do NOT build a precedence list, regex-match dates, or reimplement these parsers. Both executions share the contract.
- Read with pandas.read_csv('/work/input.csv', dtype=str, keep_default_na=False, encoding='utf-8-sig').
- Parse numbers with `contract.parse_money` and quantities with `contract.parse_quantity`.
- Discounts use `contract.parse_discount(value)` (the approved mode is configured). `10%` becomes 0.10; `0.10` is already 0.10. Never divide a plain decimal by 100.
- Parse dates with `contract.parse_date(value)` (the approved precedence is configured). Do NOT use `dayfirst=True`, `dateutil`, `pd.Timestamp(..., format=...)`, or regex format matching. Count unparseable rows.
- Deduplicate exact rows before aggregating when the definition says remove_exact.
- If a value required for a metric is missing or unparseable, follow the missing_values assumption exactly and print the excluded count.
- Do not require optional fields (for example shipping_cost) for a metric that does not need them. A filter that leaves zero rows is a bug, not a result.
- Sort periods chronologically. Coerce unexpected values with errors='coerce' and count them.
- Guard every value before calling `.strftime` or string methods: `if pd.isna(x): ...`. Never assume a date parsed; a single bad value must not crash the pipeline.
- If a text column has a `values` list, map variants that differ only by case, whitespace or punctuation to ONE canonical label.
- After cleaning, the orchestrator re-profiles cleaned.csv in Rust. The run FAILS if a categorical column still contains case/punctuation aliases, if a numeric column is still under 95% parseable, or if a date column still contains unparseable values. Clean until the audit passes.
- The chart data is audited against the ledger. If the series sums to the metric, it must match within tolerance; every assumption must appear in applied_assumptions, including the exact date_parsing precedence.
- Record `rows_used`: the number of rows that actually contributed to the metrics after cleaning and definition filters.
- matplotlib.use('Agg') before pyplot.
- Print a few progress lines, then PROOFSHEET_OK.

Output only the python code block."###,
        profile = compact_profile(d),
        plan = serde_json::to_string_pretty(plan).unwrap_or_default(),
        definition = definition_prompt(definition)
    )
}

pub fn audit_chart(
    chart: &ChartData,
    metrics: &[Metric],
    definition: &Definition,
    cleaned_rows: i64,
) -> ChartAudit {
    let mut checks: Vec<EvalCheck> = Vec::new();
    let mut check = |name: &str, passed: bool, detail: String| {
        checks.push(EvalCheck {
            name: name.into(),
            passed,
            detail,
        });
    };

    check(
        "chart has a series",
        !chart.series.is_empty(),
        format!("{} point(s)", chart.series.len()),
    );
    check(
        "series values are finite",
        chart.series.iter().all(|p| p.value.is_finite()),
        "all plotted values are numbers".into(),
    );
    check(
        "series labels are present",
        chart.series.iter().all(|p| !p.label.trim().is_empty()),
        "every point has a label".into(),
    );

    let metric = metrics.iter().find(|m| m.id == chart.metric_id);
    check(
        "rows used for metrics is plausible",
        chart.rows_used > 0 && chart.rows_used <= cleaned_rows,
        format!("{} row(s) used of {} cleaned", chart.rows_used, cleaned_rows),
    );
    check(
        "chart references an executed metric",
        metric.is_some(),
        if chart.metric_id.is_empty() {
            "metric_id is empty".into()
        } else {
            format!("metric_id={}", chart.metric_id)
        },
    );

    let applied: Vec<String> = chart
        .applied_assumptions
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    let missing: Vec<String> = definition
        .assumptions
        .iter()
        .filter(|a| {
            let expected = format!("{}={}", a.key, a.value).to_lowercase();
            !applied.iter().any(|s| s == &expected)
        })
        .map(|a| format!("{}={}", a.key, a.value))
        .collect();
    check(
        "every approved assumption is recorded",
        missing.is_empty(),
        if missing.is_empty() {
            format!("{} assumption(s)", definition.assumptions.len())
        } else {
            format!("missing: {}", missing.join(", "))
        },
    );

    if chart.aggregation == "sum" || chart.aggregation == "breakdown" {
        if let Some(m) = metric.filter(|m| m.kind != "text") {
            let sum: f64 = chart.series.iter().map(|p| p.value).sum();
            let tolerance = (m.value.abs() * 0.005).max(0.01);
            check(
                "series sums to the ledger metric",
                (sum - m.value).abs() <= tolerance,
                format!("series {sum:.2} vs metric {:.2}", m.value),
            );
        }
    }

    if let Some(top) = chart.top_category.as_deref().filter(|s| !s.trim().is_empty()) {
        let ok = metrics.iter().any(|m| {
            m.kind == "text"
                && m.text
                    .as_deref()
                    .map(|t| {
                        t.eq_ignore_ascii_case(top)
                            || t.to_lowercase().contains(&top.to_lowercase())
                            || top.to_lowercase().contains(&t.to_lowercase())
                    })
                    .unwrap_or(false)
        });
        check(
            "top category matches a ledger text metric",
            ok,
            format!("chart says {top}"),
        );
    }

    // If the ledger names a peak month and the chart contains that label, the
    // highest point must be the same month.
    for m in metrics.iter().filter(|m| m.kind == "text") {
        if let Some(t) = m.text.as_deref() {
            if let Some(ledger_month) = month_key(t) {
                if chart
                    .series
                    .iter()
                    .any(|p| month_key(&p.label).as_deref() == Some(ledger_month.as_str()))
                {
                    let peak = chart
                        .series
                        .iter()
                        .max_by(|a, b| {
                            a.value
                                .partial_cmp(&b.value)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .and_then(|p| month_key(&p.label));
                    check(
                        "chart peak matches the ledger month",
                        peak.as_deref() == Some(ledger_month.as_str()),
                        format!("ledger {ledger_month}, chart peak {:?}", peak),
                    );
                }
            }
        }
    }

    ChartAudit {
        passed: checks.iter().all(|c| c.passed),
        checks,
    }
}

fn month_key(s: &str) -> Option<String> {
    use chrono::Datelike;
    let t = s.trim();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return Some(format!("{:04}-{:02}", d.year(), d.month()));
    }
    for (fmt, input) in [
        ("%Y-%m-%d", format!("{t}-01")),
        ("%Y/%m/%d", format!("{t}/01")),
        ("%B %Y %d", format!("{t} 01")),
        ("%b %Y %d", format!("{t} 01")),
    ] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&input, fmt) {
            return Some(format!("{:04}-{:02}", d.year(), d.month()));
        }
    }
    None
}

fn text_equivalent(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();
    if let (Some(x), Some(y)) = (month_key(a), month_key(b)) {
        return x == y;
    }
    let norm = |s: &str| -> String {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    };
    let na = norm(a);
    let nb = norm(b);
    !na.is_empty()
        && !nb.is_empty()
        && (na == nb || (na.len() > 4 && nb.len() > 4 && (na.contains(&nb) || nb.contains(&na))))
}

fn checker_prompt(d: &Dataset, plan: &Plan, definition: &Definition) -> String {
    format!(
        "DATASET PROFILE (redacted samples):\\n{}\\n\\nMETRIC CONTRACT:\\n{}\\n\\nAPPROVED DEFINITION (binding):\\n{}\\n\\nWrite the independent checker script.",
        compact_profile(d),
        serde_json::to_string_pretty(plan).unwrap_or_default(),
        definition_prompt(definition)
    )
}

async fn run_consensus(
    state: &AppState,
    run: &mut Run,
    dataset: &Dataset,
    plan: &Plan,
    definition: &Definition,
) -> Result<Consensus> {
    if !state.cfg.consensus_enabled {
        return Ok(Consensus {
            status: "skipped".into(),
            model: state.cfg.model_checker.clone(),
            ..Default::default()
        });
    }
    let model = state.cfg.model_checker.clone();
    let prompt = checker_prompt(dataset, plan, definition);
    let completion = state
        .inference
        .chat(&model, CHECKER_SYSTEM, &prompt, 0.05, 9000)
        .await;
    let completion = match completion {
        Ok(c) => c,
        Err(e) => {
            push_log(state, run, "warn", &format!("Checker generation failed: {e}")).await?;
            return Ok(Consensus {
                status: "checker_failed".into(),
                model,
                checker_stderr: format!("{e:#}"),
                ..Default::default()
            });
        }
    };
    let code = match InferenceClient::extract_python(&completion.text) {
        Some(c) => c,
        None => {
            push_log(state, run, "warn", "Checker returned no python code").await?;
            return Ok(Consensus {
                status: "checker_failed".into(),
                model: completion.model,
                checker_stderr: "no python code block".into(),
                ..Default::default()
            });
        }
    };
    push_log(
        state,
        run,
        "meta",
        &format!("Checker: {} · {} tokens out", completion.model, completion.completion_tokens),
    )
    .await?;

    let dir = state
        .cfg
        .data_dir
        .join("work")
        .join(&run.id)
        .join("checker");
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    std::fs::create_dir_all(&dir)?;
    let input_path = state
        .cfg
        .data_dir
        .join("uploads")
        .join(format!("{}.csv", dataset.id));
    std::fs::copy(&input_path, dir.join("input.csv"))?;
    std::fs::write(dir.join("contract.py"), CONTRACT)?;
    std::fs::write(dir.join("contract_config.py"), contract_config(definition))?;
    std::fs::write(dir.join("script.py"), &code)?;

    let spec = SandboxSpec {
        label: format!("check-{}", run.id),
        workdir: dir.clone(),
        command: vec!["python3".into(), "/work/script.py".into()],
        timeout: Duration::from_secs(state.cfg.sandbox_timeout_secs),
        memory: None,
        cpus: None,
        network: false,
        read_only_workdir: false,
    };
    let result = state.sandbox.run(spec).await?;
    push_log(
        state,
        run,
        if result.exit_code == Some(0) { "ok" } else { "warn" },
        &format!(
            "checker {} exited {:?} in {} ms",
            result.sandbox_id, result.exit_code, result.duration_ms
        ),
    )
    .await?;

    let checker_raw = std::fs::read_to_string(dir.join("check_metrics.json")).ok();
    let checker_metrics = checker_raw
        .as_deref()
        .and_then(|raw| parse_metrics(raw, plan).ok());

    let mut consensus = Consensus {
        status: "checker_failed".into(),
        model: completion.model,
        checker_sandbox: Some(result.sandbox_id),
        checker_exit: result.exit_code,
        checker_code: Some(code),
        checker_stderr: truncate(result.stderr.trim(), 1200),
        ..Default::default()
    };

    let Some(checker_metrics) = checker_metrics else {
        consensus.checks.push(EvalCheck {
            name: "checker produced a valid ledger".into(),
            passed: false,
            detail: "check_metrics.json was missing or invalid".into(),
        });
        return Ok(consensus);
    };

    let mut deltas: Vec<MetricDelta> = Vec::new();
    for primary in &run.metrics {
        let Some(checker) = checker_metrics.iter().find(|m| m.id == primary.id) else {
            deltas.push(MetricDelta {
                id: primary.id.clone(),
                label: primary.label.clone(),
                primary: verify::format_value(primary),
                checker: "—".into(),
                diff_pct: 100.0,
                agree: false,
            });
            continue;
        };
        if primary.kind == "text" {
            let p = primary.text.as_deref().unwrap_or("");
            let c = checker.text.as_deref().unwrap_or("");
            let agree = text_equivalent(p, c);
            deltas.push(MetricDelta {
                id: primary.id.clone(),
                label: primary.label.clone(),
                primary: verify::format_value(primary),
                checker: verify::format_value(checker),
                diff_pct: if agree { 0.0 } else { 100.0 },
                agree,
            });
        } else {
            let diff = (primary.value - checker.value).abs();
            let pct = if primary.value.abs() > 1e-9 {
                diff / primary.value.abs() * 100.0
            } else if diff <= 0.01 {
                0.0
            } else {
                100.0
            };
            let agree = diff <= 0.01 || pct <= 0.5;
            deltas.push(MetricDelta {
                id: primary.id.clone(),
                label: primary.label.clone(),
                primary: verify::format_value(primary),
                checker: verify::format_value(checker),
                diff_pct: (pct * 10.0).round() / 10.0,
                agree,
            });
        }
    }

    let all_agree = !deltas.is_empty() && deltas.iter().all(|d| d.agree);
    consensus.checks.push(EvalCheck {
        name: "independent implementation agrees".into(),
        passed: all_agree,
        detail: format!(
            "{} of {} metrics within tolerance",
            deltas.iter().filter(|d| d.agree).count(),
            deltas.len()
        ),
    });
    consensus.status = if all_agree { "agree".into() } else { "disagree".into() };
    consensus.deltas = deltas;
    Ok(consensus)
}

fn build_comparison(source: &Run, current: &Run) -> Comparison {
    let mut metrics = Vec::new();
    for m in &current.metrics {
        let Some(prev) = source.metrics.iter().find(|p| p.id == m.id) else {
            continue;
        };
        if m.kind == "text" {
            metrics.push(ComparisonMetric {
                id: m.id.clone(),
                label: m.label.clone(),
                kind: m.kind.clone(),
                previous: 0.0,
                current: 0.0,
                delta: 0.0,
                delta_pct: None,
                previous_text: prev.text.clone(),
                current_text: m.text.clone(),
            });
        } else {
            let delta = m.value - prev.value;
            let pct = if prev.value.abs() > 1e-9 {
                Some(((delta / prev.value.abs()) * 1000.0).round() / 10.0)
            } else {
                None
            };
            metrics.push(ComparisonMetric {
                id: m.id.clone(),
                label: m.label.clone(),
                kind: m.kind.clone(),
                previous: prev.value,
                current: m.value,
                delta,
                delta_pct: pct,
                previous_text: None,
                current_text: None,
            });
        }
    }
    Comparison {
        source_run_id: source.id.clone(),
        source_created_at: source.created_at.clone(),
        previous_rows: source.clean_report.as_ref().map(|c| c.rows_out),
        current_rows: current.clean_report.as_ref().map(|c| c.rows_out),
        metrics,
    }
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
    if state.run_slots.available_permits() == 0 {
        // The run card is created before this task starts; wait transparently.
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let _slot = state
        .run_slots
        .acquire()
        .await
        .context("could not acquire a sandbox slot")?;
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
    let definition = source
        .definition
        .clone()
        .ok_or_else(|| anyhow!("this legacy run has no approved definition; run the analysis once to create a replayable pipeline"))?;

    run.plan = Some(plan.clone());
    run.definition = Some(definition.clone());
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
    std::fs::write(workdir.join("contract.py"), CONTRACT)?;
    std::fs::write(workdir.join("contract_config.py"), contract_config(&definition))?;
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

    let (metrics, _artifact_template, clean_report, chart_data, chart_audit) =
        read_artifacts(&workdir, &plan, &dataset.profile, &definition)?;
    run.metrics = metrics;
    run.clean_report = Some(clean_report);
    run.chart_data = Some(chart_data);
    run.chart_audit = Some(chart_audit);
    // The repaired template from the original run is authoritative: the generated
    // script may still emit the pre-repair note, and replay must not resurrect it.
    run.template = Some(template.clone());
    let flags = verify::verify_template(&template, &run.metrics);
    run.claims = flags;
    let (rendered, receipted) = verify::render(&template, &run.metrics);
    run.notes_markdown = Some(rendered);
    let flag_count = run.claims.len();
    run.comparison = Some(build_comparison(&source, &run));
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

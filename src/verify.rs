use crate::models::{ClaimFlag, Metric};
use regex::Regex;
use std::sync::OnceLock;

fn placeholder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{\{\s*m:([A-Za-z0-9_]+)\s*\}\}").unwrap())
}

fn number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[0-9][0-9.,]*[0-9]?%?|[0-9]").unwrap())
}

/// The core guarantee: a report may only contain numbers that the sandbox produced.
/// We enforce it structurally (placeholders) and audit for literal digits.
pub fn verify_template(template: &str, metrics: &[Metric]) -> Vec<ClaimFlag> {
    let mut flags = Vec::new();
    if template.trim().is_empty() {
        flags.push(ClaimFlag {
            kind: "empty_template".into(),
            detail: "The report body is empty.".into(),
            token: None,
        });
        return flags;
    }
    if metrics.is_empty() {
        flags.push(ClaimFlag {
            kind: "no_metrics".into(),
            detail: "Execution produced no metrics to cite.".into(),
            token: None,
        });
        return flags;
    }

    let ids: Vec<&str> = metrics.iter().map(|m| m.id.as_str()).collect();
    let mut referenced = 0usize;
    for cap in placeholder_re().captures_iter(template) {
        let id = &cap[1];
        if !ids.contains(&id) {
            flags.push(ClaimFlag {
                kind: "unknown_metric".into(),
                detail: format!("Placeholder references a metric that execution did not produce: {id}"),
                token: Some(cap[0].to_string()),
            });
        } else {
            referenced += 1;
        }
    }

    if referenced == 0 {
        flags.push(ClaimFlag {
            kind: "no_placeholders".into(),
            detail: "The report cites no executed metric. Every number must come from the metric ledger."
                .into(),
            token: None,
        });
    }

    // Strip placeholders, then look for any remaining digit characters.
    let stripped = placeholder_re().replace_all(template, " ⟨metric⟩ ");
    for m in number_re().find_iter(&stripped).take(10) {
        let token = m.as_str().to_string();
        let start = m.start().saturating_sub(34);
        let end = (m.end() + 34).min(stripped.len());
        let context = stripped[start..end].replace('\n', " ");
        flags.push(ClaimFlag {
            kind: "literal_number".into(),
            detail: format!("Unreceipted number: …{context}…"),
            token: Some(token),
        });
    }

    flags
}

/// Replace metric placeholders with render tokens the frontend turns into clickable receipts.
pub fn render(template: &str, metrics: &[Metric]) -> (String, usize) {
    let by_id: std::collections::HashMap<&str, &Metric> =
        metrics.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut receipted = 0usize;
    let rendered = placeholder_re()
        .replace_all(template, |caps: &regex::Captures| {
            let id = &caps[1];
            if let Some(metric) = by_id.get(id) {
                receipted += 1;
                format!("⟦{}|{}⟧", metric.id, format_value(metric))
            } else {
                format!("⟦unknown:{}|—⟧", id)
            }
        })
        .to_string();
    (rendered, receipted)
}

pub fn format_value(m: &Metric) -> String {
    if m.kind == "text" {
        m.text.clone().unwrap_or_else(|| "—".into())
    } else {
        format_metric(m.value, &m.kind)
    }
}

pub fn format_metric(value: f64, kind: &str) -> String {
    match kind {
        "currency" => {
            let sign = if value < 0.0 { "-" } else { "" };
            let v = value.abs();
            let body = if (v.fract()).abs() < 1e-9 {
                fixed(v, 0)
            } else {
                fixed(v, 2)
            };
            format!("{sign}${body}")
        }
        "percent" => {
            let pct = value * 100.0;
            format!("{pct:.1}%")
        }
        "count" => fixed(value.round(), 0),
        _ => {
            let abs = value.abs();
            if abs >= 1000.0 {
                if (value.fract()).abs() < 1e-9 {
                    fixed(value, 0)
                } else {
                    fixed(value, 2)
                }
            } else if (value.fract()).abs() < 1e-9 {
                format!("{value:.0}")
            } else {
                format!("{value:.2}")
            }
        }
    }
}

/// Rust's formatter has no thousands grouping, so do it explicitly.
fn fixed(value: f64, decimals: usize) -> String {
    let raw = format!("{:.*}", decimals, value.abs());
    let (int_part, frac) = raw.split_once('.').unwrap_or((raw.as_str(), ""));
    let mut grouped = String::new();
    for (i, ch) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let int_grouped: String = grouped.chars().rev().collect();
    let sign = if value < 0.0 { "-" } else { "" };
    if decimals > 0 {
        format!("{sign}{int_grouped}.{frac}")
    } else {
        format!("{sign}{int_grouped}")
    }
}

/// Deterministic self-test surfaced in /api/health and the proof page.
pub fn selftest() -> (usize, usize) {
    let metrics = vec![
        Metric {
            id: "revenue_total".into(),
            label: "Revenue".into(),
            kind: "currency".into(),
            value: 1234567.5,
            text: None,
        },
        Metric {
            id: "churn_rate".into(),
            label: "Churn".into(),
            kind: "percent".into(),
            value: 0.0234,
            text: None,
        },
    ];
    let mut passed = 0;
    let total = 6;

    if verify_template("Revenue was {{m:revenue_total}} this period.", &metrics).is_empty() {
        passed += 1;
    }
    if verify_template("Revenue was $1,234,567.50 this period.", &metrics)
        .iter()
        .any(|f| f.kind == "literal_number")
    {
        passed += 1;
    }
    if verify_template("{{m:made_up}}", &metrics)
        .iter()
        .any(|f| f.kind == "unknown_metric")
    {
        passed += 1;
    }
    if verify_template("", &metrics)
        .iter()
        .any(|f| f.kind == "empty_template")
    {
        passed += 1;
    }
    if verify_template("No numbers here.", &metrics)
        .iter()
        .any(|f| f.kind == "no_placeholders")
    {
        passed += 1;
    }
    let (rendered, n) = render("Revenue {{m:revenue_total}} at {{m:churn_rate}}.", &metrics);
    if n == 2 && rendered.contains("$1,234,567.50") && rendered.contains("2.3%") {
        passed += 1;
    }

    (passed, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, v: f64) -> Metric {
        Metric {
            id: id.into(),
            label: id.into(),
            kind: "number".into(),
            value: v,
            text: None,
        }
    }

    #[test]
    fn flags_literal_numbers() {
        let flags = verify_template("There were 42 orders.", &[m("orders", 42.0)]);
        assert!(flags.iter().any(|f| f.kind == "literal_number"));
    }

    #[test]
    fn accepts_placeholders() {
        let flags = verify_template("There were {{m:orders}} orders.", &[m("orders", 42.0)]);
        assert!(flags.is_empty(), "{flags:?}");
    }

    #[test]
    fn formats_currency() {
        assert_eq!(format_metric(1234.5, "currency"), "$1,234.50");
        assert_eq!(format_metric(-12.0, "currency"), "-$12");
        assert_eq!(format_metric(0.234, "percent"), "23.4%");
    }
}

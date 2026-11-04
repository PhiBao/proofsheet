use crate::agent;
use crate::models::*;
use crate::profile::profile_csv;
use crate::verify;

fn check(name: &str, passed: bool, detail: impl Into<String>) -> EvalCheck {
    EvalCheck {
        name: name.into(),
        passed,
        detail: detail.into(),
    }
}

fn assumption(key: &str, value: &str) -> Assumption {
    Assumption {
        key: key.into(),
        label: key.into(),
        value: value.into(),
        kind: "text".into(),
        options: vec![],
        why: String::new(),
    }
}

/// Deterministic checks that can run on every /api/proof request.
/// Deliberately no model calls: this is a regression suite, not a benchmark.
pub fn run() -> Vec<EvalCheck> {
    let mut out = Vec::new();

    let orders = include_str!("../fixtures/northwind_orders_messy.csv");
    match profile_csv(orders.as_bytes()) {
        Ok(p) => {
            out.push(check(
                "orders fixture: duplicate rows detected",
                p.duplicate_rows > 0,
                format!("{} duplicates", p.duplicate_rows),
            ));
            out.push(check(
                "orders fixture: mixed date formats detected",
                p.issues.iter().any(|i| i.kind == "mixed_dates"),
                "date formats vary across rows",
            ));
            out.push(check(
                "orders fixture: currency text detected",
                p.columns.iter().any(|c| c.kind == "currency"),
                "unit_price is currency text",
            ));
            out.push(check(
                "orders fixture: dirty score above ten",
                p.dirty_score > 10,
                format!("score {}", p.dirty_score),
            ));
        }
        Err(e) => out.push(check("orders fixture parses", false, e.to_string())),
    }

    let next = include_str!("../fixtures/northwind_orders_next_month.csv");
    match profile_csv(next.as_bytes()) {
        Ok(p) => {
            let dates = p.columns.iter().find(|c| c.name == "order_date");
            out.push(check(
                "replay fixture: all dates parse",
                dates
                    .map(|c| c.non_null == p.rows)
                    .unwrap_or(false),
                "every order_date is a valid date",
            ));
            out.push(check(
                "replay fixture: duplicates are real",
                p.duplicate_rows > 0,
                format!("{} exact duplicates", p.duplicate_rows),
            ));
        }
        Err(e) => out.push(check("replay fixture parses", false, e.to_string())),
    }

    let metric = Metric {
        id: "total_revenue".into(),
        label: "Total revenue".into(),
        kind: "currency".into(),
        value: 1000.0,
        text: None,
    };
    out.push(check(
        "verifier: valid placeholder passes",
        verify::verify_template("Revenue was {{m:total_revenue}}.", &[metric.clone()]).is_empty(),
        "placeholder resolves",
    ));
    out.push(check(
        "verifier: literal number is flagged",
        verify::verify_template("Revenue was 1000.", &[metric.clone()])
            .iter()
            .any(|f| f.kind == "literal_number"),
        "unreceipted 1000 caught",
    ));
    out.push(check(
        "verifier: unknown metric is flagged",
        verify::verify_template("{{m:made_up}}", &[metric.clone()])
            .iter()
            .any(|f| f.kind == "unknown_metric"),
        "unknown id caught",
    ));
    out.push(check(
        "verifier: note with no placeholder is flagged",
        verify::verify_template("The trend is good.", &[metric.clone()])
            .iter()
            .any(|f| f.kind == "no_placeholders"),
        "no receipts means no stamp",
    ));

    let definition = Definition {
        summary: "Test definition".into(),
        assumptions: vec![
            assumption("include_shipping", "false"),
            assumption("date_order", "day_first"),
        ],
    };
    let good_chart = ChartData {
        metric_id: "total_revenue".into(),
        aggregation: "sum".into(),
        unit: "currency".into(),
        series: vec![
            ChartPoint {
                label: "2024-01".into(),
                value: 600.0,
            },
            ChartPoint {
                label: "2024-02".into(),
                value: 400.0,
            },
        ],
        top_category: None,
        rows_used: 100,
        applied_assumptions: vec!["include_shipping=false".into(), "date_order=day_first".into()],
    };
    out.push(check(
        "chart audit: series sums to ledger",
        agent::audit_chart(&good_chart, &[metric.clone()], &definition, 100).passed,
        "600 + 400 = 1000",
    ));

    let mut bad_sum = good_chart.clone();
    bad_sum.series[1].value = 300.0;
    out.push(check(
        "chart audit: sum mismatch is rejected",
        !agent::audit_chart(&bad_sum, &[metric.clone()], &definition, 100).passed,
        "900 != 1000",
    ));

    let mut missing_assumption = good_chart.clone();
    missing_assumption.applied_assumptions = vec!["include_shipping=false".into()];
    out.push(check(
        "chart audit: missing assumption is rejected",
        !agent::audit_chart(&missing_assumption, &[metric.clone()], &definition, 100).passed,
        "date_order is not recorded",
    ));

    let defaults = agent::parse_definition(None);
    out.push(check(
        "definition: generic defaults are present",
        defaults.assumptions.len() >= 4,
        format!("{} assumptions", defaults.assumptions.len()),
    ));

    out.push(check(
        "formatting: currency has grouping and cents",
        verify::format_metric(1234.5, "currency") == "$1,234.50",
        verify::format_metric(1234.5, "currency"),
    ));

    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn golden_suite_passes() {
        let checks = super::run();
        let failed: Vec<_> = checks.iter().filter(|c| !c.passed).collect();
        assert!(failed.is_empty(), "failed checks: {failed:#?}");
    }
}

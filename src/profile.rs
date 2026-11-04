use crate::models::{CleanReport, ColumnFix, ColumnProfile, Issue, Profile};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_PREVIEW_ROWS: usize = 8;
const MAX_PREVIEW_COLS: usize = 9;

/// Deterministic, dependency-light CSV profiler. This runs in Rust (not the LLM)
/// so the "what is dirty" verdict is reproducible and can be trusted.
pub fn profile_csv(bytes: &[u8]) -> anyhow::Result<Profile> {
    let text = decode(bytes);
    let delimiter = detect_delimiter(&text);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for rec in reader.records() {
        match rec {
            Ok(r) => records.push(r.iter().map(|s| s.to_string()).collect()),
            Err(_) => continue,
        }
    }

    if records.is_empty() {
        anyhow::bail!("The file is empty.");
    }

    let header = records.remove(0);
    let rows = records;
    let col_count = header.len();

    let mut issues: Vec<Issue> = Vec::new();

    // Header hygiene
    let mut seen = HashMap::new();
    for (i, name) in header.iter().enumerate() {
        if name.trim().is_empty() {
            issues.push(Issue {
                severity: "error".into(),
                kind: "empty_header".into(),
                column: Some(format!("column {}", i + 1)),
                count: 1,
                detail: "A column has no name.".into(),
            });
        }
        if name != name.trim() {
            issues.push(Issue {
                severity: "warn".into(),
                kind: "header_whitespace".into(),
                column: Some(name.clone()),
                count: 1,
                detail: "Column name has leading or trailing whitespace.".into(),
            });
        }
        *seen.entry(name.trim().to_lowercase()).or_insert(0) += 1;
    }
    for (name, count) in seen {
        if count > 1 {
            issues.push(Issue {
                severity: "warn".into(),
                kind: "duplicate_header".into(),
                column: Some(name),
                count,
                detail: "Column name appears more than once.".into(),
            });
        }
    }

    // Ragged rows
    let ragged = rows.iter().filter(|r| r.len() != col_count).count() as i64;
    if ragged > 0 {
        issues.push(Issue {
            severity: "error".into(),
            kind: "ragged_rows".into(),
            column: None,
            count: ragged,
            detail: "Rows have a different number of fields than the header.".into(),
        });
    }

    // Duplicate rows (exact, on the raw values)
    let mut row_hashes: HashSet<String> = HashSet::new();
    let mut duplicate_rows = 0i64;
    for r in &rows {
        let key = r.join("\u{1f}");
        if !row_hashes.insert(key) {
            duplicate_rows += 1;
        }
    }
    if duplicate_rows > 0 {
        issues.push(Issue {
            severity: "warn".into(),
            kind: "duplicate_rows".into(),
            column: None,
            count: duplicate_rows,
            detail: "Exact duplicate rows will double-count totals if left in.".into(),
        });
    }

    let mut columns: Vec<ColumnProfile> = Vec::with_capacity(col_count);
    for (i, name) in header.iter().enumerate() {
        let raw: Vec<&str> = rows
            .iter()
            .map(|r| r.get(i).map(|s| s.as_str()).unwrap_or(""))
            .collect();
        columns.push(profile_column(name, &raw, rows.len()));
    }

    // Column issues get promoted into the global issue list too.
    for col in &columns {
        for msg in &col.issues {
            let (severity, kind, detail) = classify_column_issue(msg);
            issues.push(Issue {
                severity: severity.into(),
                kind: kind.into(),
                column: Some(col.name.clone()),
                count: 1,
                detail,
            });
        }
    }

    if duplicate_rows == 0 && issues.iter().all(|i| i.severity == "info") {
        issues.push(Issue {
            severity: "info".into(),
            kind: "clean_file".into(),
            column: None,
            count: 0,
            detail: "No structural dirt detected. Analysis can run directly.".into(),
        });
    }

    issues.sort_by_key(|i| match i.severity.as_str() {
        "error" => 0,
        "warn" => 1,
        _ => 2,
    });

    let preview: Vec<Vec<String>> = rows
        .iter()
        .take(MAX_PREVIEW_ROWS)
        .map(|r| {
            r.iter()
                .take(MAX_PREVIEW_COLS)
                .map(|s| truncate(s, 48))
                .collect()
        })
        .collect();

    let dirty_score = dirty_score(&columns, duplicate_rows, ragged, rows.len());

    Ok(Profile {
        delimiter: match delimiter {
            b',' => "comma".into(),
            b';' => "semicolon".into(),
            b'\t' => "tab".into(),
            b'|' => "pipe".into(),
            _ => "other".into(),
        },
        rows: rows.len() as i64,
        duplicate_rows,
        issues,
        columns,
        preview,
        dirty_score,
    })
}

fn profile_column(name: &str, raw: &[&str], total_rows: usize) -> ColumnProfile {
    let mut nulls = 0i64;
    let mut non_empty: Vec<&str> = Vec::new();
    let mut whitespace = 0i64;
    let mut unique: HashSet<String> = HashSet::new();
    let mut numbers = 0usize;
    let mut currency = 0usize;
    let mut percent = 0usize;
    let mut dates = 0usize;
    let mut date_formats: HashSet<&'static str> = HashSet::new();
    let mut ambiguous_dates = 0i64;
    let mut thousands = 0i64;
    let mut row_formats: Vec<Option<&'static str>> = vec![None; raw.len()];
    let mut numeric_ok: Vec<bool> = vec![false; raw.len()];
    let mut non_numeric = 0usize;
    let mut trimmed_counts: HashMap<String, usize> = HashMap::new();
    let mut dirty: HashSet<usize> = HashSet::new();

    for (idx, v) in raw.iter().enumerate() {
        if v.trim().is_empty() {
            nulls += 1;
            continue;
        }
        let t = v.trim();
        if *v != t {
            whitespace += 1;
            dirty.insert(idx);
        }
        non_empty.push(v);
        unique.insert(t.to_string());
        *trimmed_counts.entry(t.to_string()).or_insert(0) += 1;

        if has_currency(t) {
            currency += 1;
            dirty.insert(idx);
        }
        if t.ends_with('%') {
            percent += 1;
            dirty.insert(idx);
        }
        if t.contains(',') && parse_number(t).is_some() {
            thousands += 1;
            dirty.insert(idx);
        }
        if parse_number(t).is_some() {
            numbers += 1;
            numeric_ok[idx] = true;
        } else {
            non_numeric += 1;
        }
        let matches = matching_date_formats(t);
        if !matches.is_empty() {
            dates += 1;
            row_formats[idx] = Some(matches[0]);
            date_formats.insert(matches[0]);
            if matches.len() > 1 {
                ambiguous_dates += 1;
            }
        }
    }

    let non_null = non_empty.len() as i64;
    let denom = non_null.max(1) as f64;
    let mut kind = "text".to_string();
    if non_empty.is_empty() {
        kind = "empty".into();
    } else if percent as f64 / denom >= 0.9 {
        kind = "percent".into();
    } else if currency > 0 && numbers as f64 / denom >= 0.9 {
        kind = "currency".into();
    } else if numbers as f64 / denom >= 0.9 {
        kind = "number".into();
    } else if dates as f64 / denom >= 0.9 {
        kind = "date".into();
    }

    let lower: HashSet<String> = unique.iter().map(|s| s.to_lowercase()).collect();
    let casing_issue = kind == "text" && lower.len() < unique.len() && unique.len() <= 60;

    // Count values a correct cleaning pass must actually normalize.
    if matches!(kind.as_str(), "number" | "currency" | "percent") {
        for (idx, v) in raw.iter().enumerate() {
            if !v.trim().is_empty() && !numeric_ok[idx] {
                dirty.insert(idx);
            }
        }
    }
    if kind == "date" {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for f in row_formats.iter().flatten() {
            *counts.entry(*f).or_insert(0) += 1;
        }
        if let Some((primary, _)) = counts.into_iter().max_by_key(|(_, c)| *c) {
            for (idx, f) in row_formats.iter().enumerate() {
                if raw[idx].trim().is_empty() {
                    continue;
                }
                if *f != Some(primary) {
                    dirty.insert(idx);
                }
            }
        }
    }
    if casing_issue {
        if let Some((canonical, _)) = trimmed_counts.iter().max_by_key(|(_, c)| **c) {
            for (idx, v) in raw.iter().enumerate() {
                if v.trim().is_empty() {
                    continue;
                }
                if v.trim() != canonical {
                    dirty.insert(idx);
                }
            }
        }
    }
    let dirty_values = dirty.len() as i64;

    let mut issues: Vec<String> = Vec::new();
    if nulls > 0 {
        issues.push(format!("{nulls} empty value(s)"));
    }
    if whitespace > 0 {
        issues.push(format!("{whitespace} value(s) with stray whitespace"));
    }
    if currency > 0 && (kind == "number" || kind == "currency" || numbers > 0) {
        issues.push(format!("{currency} value(s) with currency symbols"));
    }
    if percent > 0 && kind != "percent" {
        issues.push(format!("{percent} value(s) stored as percent text"));
    }
    if thousands > 0 {
        issues.push(format!("{thousands} value(s) with thousands separators"));
    }
    if matches!(kind.as_str(), "number" | "currency" | "percent") && non_numeric > 0 {
        issues.push(format!("{non_numeric} non-numeric value(s)"));
    }
    if date_formats.len() > 1 {
        let mut fmts: Vec<&str> = date_formats.iter().cloned().collect();
        fmts.sort();
        issues.push(format!("mixed date formats ({})", fmts.join(", ")));
    }
    if ambiguous_dates > 0 {
        issues.push(format!(
            "{ambiguous_dates} ambiguous date(s) (day/month order unclear)"
        ));
    }
    if total_rows > 1 && unique.len() == 1 {
        issues.push("constant value in every row".into());
    }
    if casing_issue {
        issues.push("inconsistent casing or spacing (e.g. USA / usa)".into());
    }
    if kind == "empty" {
        issues.push("column is entirely empty".into());
    }

    let sample: Vec<String> = non_empty
        .iter()
        .take(4)
        .map(|s| truncate(s, 40))
        .collect();

    let mut values: Vec<String> = if unique.len() <= 30 && kind == "text" {
        unique.iter().cloned().collect()
    } else {
        Vec::new()
    };
    values.sort();
    values.truncate(18);

    ColumnProfile {
        name: name.trim().to_string(),
        kind,
        non_null,
        nulls,
        null_pct: ((nulls as f64 / total_rows.max(1) as f64) * 1000.0).round() / 10.0,
        unique: unique.len() as i64,
        sample,
        values,
        dirty_values,
        issues,
    }
}

fn classify_column_issue(msg: &str) -> (&'static str, &'static str, String) {
    if msg.contains("mixed date") {
        ("error", "mixed_dates", format!("Date column cannot be sorted or grouped until normalized: {msg}."))
    } else if msg.contains("ambiguous date") {
        ("warn", "ambiguous_dates", format!("Day/month order is not deterministic: {msg}."))
    } else if msg.contains("currency") {
        ("warn", "currency_text", format!("Numeric column contains currency symbols: {msg}."))
    } else if msg.contains("percent") {
        ("warn", "percent_text", format!("Percentages stored as text: {msg}."))
    } else if msg.contains("thousands") {
        ("warn", "thousands_separator", format!("Numbers are stored with separators: {msg}."))
    } else if msg.contains("whitespace") {
        ("warn", "whitespace", format!("Whitespace will break joins and grouping: {msg}."))
    } else if msg.contains("empty value") {
        ("warn", "nulls", format!("Missing data: {msg}."))
    } else if msg.contains("inconsistent casing") {
        ("warn", "casing", format!("Category labels are not normalized: {msg}."))
    } else if msg.contains("constant value") {
        ("info", "constant", format!("Constant column: {msg}."))
    } else if msg.contains("entirely empty") {
        ("error", "empty_column", "Column is entirely empty and will be dropped.".into())
    } else {
        ("info", "note", msg.to_string())
    }
}

fn has_currency(t: &str) -> bool {
    ["$", "€", "£", "¥", "₹"]
        .iter()
        .any(|sym| t.starts_with(sym))
}

/// The "what changed" table is derived here, not reported by the model.
/// Rows and dirty-value counts come from the raw profile; the after-state comes
/// from re-profiling the cleaned file with the same deterministic code.
pub fn build_clean_report(profile: &Profile, cleaned_path: &Path) -> anyhow::Result<CleanReport> {
    let bytes = std::fs::read(cleaned_path)?;
    let cleaned = profile_csv(&bytes)?;
    let rows_in = profile.rows;
    let rows_out = cleaned.rows;
    let removed = (rows_in - rows_out).max(0);
    let duplicates_removed = removed.min(profile.duplicate_rows);

    let mut columns = Vec::new();
    for col in &profile.columns {
        let after = cleaned.columns.iter().find(|c| c.name == col.name);
        let kind_changed = after
            .map(|a| a.kind != col.kind)
            .unwrap_or(col.kind != "empty");
        if col.dirty_values == 0 && !kind_changed {
            continue;
        }
        columns.push(ColumnFix {
            name: col.name.clone(),
            before: before_label(col),
            after: after_label(after),
            fixes: col.dirty_values,
            rule: rule_for(col),
        });
    }

    Ok(CleanReport {
        rows_in,
        rows_out,
        duplicates_removed,
        columns,
    })
}

fn before_label(c: &ColumnProfile) -> String {
    let has = |needle: &str| c.issues.iter().any(|i| i.contains(needle));
    if has("mixed date") || has("ambiguous date") {
        "mixed date text".into()
    } else if has("currency") {
        "currency text".into()
    } else if has("non-numeric") {
        "number with text values".into()
    } else if has("percent") {
        "percent text".into()
    } else if has("stray whitespace") {
        "text with stray whitespace".into()
    } else if has("inconsistent casing") {
        "mixed-case labels".into()
    } else if c.nulls > 0 {
        format!("{} with empty values", c.kind)
    } else {
        c.kind.clone()
    }
}

fn after_label(after: Option<&ColumnProfile>) -> String {
    match after {
        None => "column dropped".into(),
        Some(a) => {
            // Missing values are allowed to remain; they are not "residual dirt".
            let residual = a
                .issues
                .iter()
                .any(|i| !i.contains("empty value") && !i.contains("constant value"));
            if residual {
                return format!("{} (residual dirt)", a.kind);
            }
            match a.kind.as_str() {
                "date" => "date".into(),
                "number" => "number".into(),
                "text" => "canonical text".into(),
                "currency" => "currency".into(),
                "percent" => "fraction".into(),
                other => other.into(),
            }
        }
    }
}

fn rule_for(c: &ColumnProfile) -> String {
    let has = |needle: &str| c.issues.iter().any(|i| i.contains(needle));
    if has("mixed date") || has("ambiguous date") {
        "explicit strptime; ambiguous order resolved".into()
    } else if has("currency") {
        "symbols and separators stripped; coerced to number".into()
    } else if has("non-numeric") {
        "invalid values excluded from metrics".into()
    } else if has("percent") {
        "percent text parsed to a fraction".into()
    } else if has("inconsistent casing") {
        "aliases mapped to one canonical label".into()
    } else if has("stray whitespace") {
        "whitespace trimmed".into()
    } else if c.nulls > 0 {
        "empty values handled explicitly".into()
    } else {
        "normalized".into()
    }
}

fn dirty_score(columns: &[ColumnProfile], duplicates: i64, ragged: i64, rows: usize) -> u8 {
    let mut score = 0f64;
    score += (duplicates as f64 / rows.max(1) as f64 * 60.0).min(30.0);
    if ragged > 0 {
        score += 20.0;
    }
    let mut null_ratio = 0f64;
    for c in columns {
        if c.null_pct > 0.0 {
            null_ratio += c.null_pct / 100.0;
        }
        for i in &c.issues {
            if i.contains("mixed date") {
                score += 12.0;
            } else if i.contains("ambiguous date") {
                score += 6.0;
            } else if i.contains("currency") || i.contains("percent") || i.contains("thousands") {
                score += 4.0;
            } else if i.contains("whitespace") {
                score += 3.0;
            } else if i.contains("inconsistent casing") {
                score += 3.0;
            } else if i.contains("entirely empty") {
                score += 5.0;
            }
        }
    }
    if !columns.is_empty() {
        score += (null_ratio / columns.len() as f64) * 25.0;
    }
    (score.round() as u8).min(100)
}

fn detect_delimiter(text: &str) -> u8 {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let candidates = [(b',', ","), (b';', ";"), (b'\t', "\t"), (b'|', "|")];
    let mut best = (b',', 0usize);
    for (byte, s) in candidates {
        let count = line.matches(s).count();
        if count > best.1 {
            best = (byte, count);
        }
    }
    best.0
}

fn decode(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Independent post-execution audit of the cleaned file. The orchestrator — not
/// the model — decides whether the dirt described in the profile is actually gone.
pub fn audit_cleaned(path: &std::path::Path, profile: &Profile) -> Vec<String> {
    let mut problems = Vec::new();
    let mut rdr = match csv::Reader::from_path(path) {
        Ok(r) => r,
        Err(e) => return vec![format!("cleaned.csv cannot be read: {e}")],
    };
    let headers: Vec<String> = rdr
        .headers()
        .map(|h| h.iter().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records().flatten() {
        rows.push(rec.iter().map(|s| s.to_string()).collect());
        if rows.len() >= 20000 {
            break;
        }
    }

    for col in &profile.columns {
        let Some(idx) = headers.iter().position(|h| h == &col.name) else {
            continue;
        };
        let vals: Vec<&str> = rows
            .iter()
            .map(|r| r.get(idx).map(|s| s.as_str()).unwrap_or(""))
            .filter(|v| !v.trim().is_empty())
            .collect();
        if vals.is_empty() {
            if col.non_null > 0 {
                problems.push(format!(
                    "column '{}' is entirely empty after cleaning (it had {} values)",
                    col.name, col.non_null
                ));
            }
            continue;
        }

        if col.kind == "text" && !col.values.is_empty() {
            let mut groups: HashMap<String, HashSet<String>> = HashMap::new();
            for v in &vals {
                groups
                    .entry(norm_key(v))
                    .or_default()
                    .insert(v.trim().to_string());
            }
            for (_, set) in groups {
                if set.len() > 1 {
                    let mut list: Vec<String> = set.into_iter().collect();
                    list.sort();
                    problems.push(format!(
                        "column '{}' still contains aliases that differ only by case/punctuation: {}",
                        col.name,
                        list.join(", ")
                    ));
                }
            }
        }

        if col.kind == "currency" || col.kind == "number" {
            let ok = vals.iter().filter(|v| parse_number(v).is_some()).count();
            if ok * 100 / vals.len() < 95 {
                problems.push(format!(
                    "column '{}' is still not numeric after cleaning ({}% parse)",
                    col.name,
                    ok * 100 / vals.len()
                ));
            }
        }
        if col.kind == "percent" {
            let ok = vals
                .iter()
                .filter(|v| parse_number(v.trim().trim_end_matches('%')).is_some())
                .count();
            if ok * 100 / vals.len() < 95 {
                problems.push(format!(
                    "column '{}' is still not numeric after cleaning ({}% parse)",
                    col.name,
                    ok * 100 / vals.len()
                ));
            }
        }
        if col.kind == "date" {
            let ok = vals
                .iter()
                .filter(|v| !matching_date_formats(v).is_empty())
                .count();
            if ok * 100 / vals.len() < 95 {
                problems.push(format!(
                    "column '{}' still contains unparseable dates ({}% parse)",
                    col.name,
                    ok * 100 / vals.len()
                ));
            }
        }
    }
    problems
}

fn norm_key(s: &str) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for c in s.trim().to_lowercase().chars() {
        if c == '.' || c == ',' || c == '-' || c == '_' || c == '\'' {
            continue;
        }
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out.trim().to_string()
}

pub fn parse_number(s: &str) -> Option<f64> {
    let mut t = s.trim();
    if t.is_empty() {
        return None;
    }
    for sym in ["$", "€", "£", "¥", "₹"] {
        t = t.trim_start_matches(sym).trim();
    }
    let mut negative = false;
    if t.starts_with('(') && t.ends_with(')') {
        negative = true;
        t = &t[1..t.len() - 1];
    }
    let cleaned = t.replace([',', ' ', '\u{a0}'], "");
    let cleaned = cleaned.trim_end_matches('%');
    let v = cleaned.parse::<f64>().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(if negative { -v } else { v })
}

fn matching_date_formats(s: &str) -> Vec<&'static str> {
    const FORMATS: &[(&str, &str)] = &[
        ("%Y-%m-%d", "YYYY-MM-DD"),
        ("%Y/%m/%d", "YYYY/MM/DD"),
        ("%m/%d/%Y", "MM/DD/YYYY"),
        ("%d/%m/%Y", "DD/MM/YYYY"),
        ("%m-%d-%Y", "MM-DD-YYYY"),
        ("%d-%m-%Y", "DD-MM-YYYY"),
        ("%b %d, %Y", "Mon DD, YYYY"),
        ("%B %d, %Y", "Month DD, YYYY"),
        ("%d %b %Y", "DD Mon YYYY"),
        ("%d %B %Y", "DD Month YYYY"),
        ("%Y-%m-%dT%H:%M:%S", "ISO datetime"),
        ("%Y-%m-%d %H:%M:%S", "YYYY-MM-DD HH:MM:SS"),
        ("%m/%d/%y", "MM/DD/YY"),
        ("%d/%m/%y", "DD/MM/YY"),
    ];
    let t = s.trim();
    let mut out = Vec::new();
    for (fmt, label) in FORMATS {
        if NaiveDate::parse_from_str(t, fmt).is_ok() {
            out.push(*label);
        } else if fmt.contains("%H:%M:%S") {
            if chrono::NaiveDateTime::parse_from_str(t, fmt).is_ok() {
                out.push(*label);
            }
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_messy_numbers() {
        assert_eq!(parse_number("$1,234.50"), Some(1234.5));
        assert_eq!(parse_number("(45.00)"), Some(-45.0));
        assert_eq!(parse_number("12%"), Some(12.0));
        assert_eq!(parse_number("abc"), None);
    }

    #[test]
    fn finds_dupes_and_nulls() {
        let csv = "id,date,amount\n1,2024-01-02,$10.00\n1,2024-01-02,$10.00\n2,01/03/2024,\n";
        let p = profile_csv(csv.as_bytes()).unwrap();
        assert_eq!(p.rows, 3);
        assert_eq!(p.duplicate_rows, 1);
        assert_eq!(p.columns[2].nulls, 1);
        assert!(p.issues.iter().any(|i| i.kind == "mixed_dates"));
    }

    #[test]
    fn detects_currency() {
        let csv = "a,b\n$1.00,2\n$3.00,4\n";
        let p = profile_csv(csv.as_bytes()).unwrap();
        assert_eq!(p.columns[0].kind, "currency");
    }
}

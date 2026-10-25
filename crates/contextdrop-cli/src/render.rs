use std::collections::BTreeSet;

use contextdrop_core::{CompressionResult, TokenMeasurementKind};
use similar::TextDiff;

use crate::benchmark::CliBenchmarkReport;

pub fn human_summary(result: &CompressionResult) -> String {
    let metrics = &result.metrics;
    let classification = &result.classification;
    let measurement = if metrics.token_measurement.kind == TokenMeasurementKind::Exact {
        format!("{} exact", metrics.token_measurement.name)
    } else {
        format!("{} estimate", metrics.token_measurement.name)
    };
    let mut lines = Vec::new();
    lines.push(format!(
        "ContextDrop | {} | {} ({}, {:.0}%) | {} -> {} bytes ({:.1}% smaller)",
        result.strategy.as_str(),
        classification.kind.as_str(),
        if classification.automatic {
            "automatic"
        } else {
            "manual"
        },
        classification.confidence * 100.0,
        format_count(metrics.original_bytes),
        format_count(metrics.output_bytes),
        metrics.byte_reduction_percent
    ));
    lines.push(format!(
        "Tokens | {} -> {} ({:.1}% smaller) | {} | {:.1} ms",
        format_count(metrics.original_tokens),
        format_count(metrics.output_tokens),
        metrics.token_reduction_percent,
        measurement,
        metrics.processing_time_ms
    ));
    let preserved = if result.preservation.passed {
        format!(
            "OK | {}/{} protected occurrence(s) retained",
            result.preservation.retained_occurrences, result.preservation.protected_occurrences
        )
    } else {
        format!(
            "FAILED | {}/{} protected occurrence(s) retained",
            result.preservation.retained_occurrences, result.preservation.protected_occurrences
        )
    };
    lines.push(format!(
        "Preservation | {preserved} | fallback: {}",
        if result.fallback_used { "yes" } else { "no" }
    ));
    let omitted = omission_summary(result);
    if !omitted.is_empty() {
        lines.push(format!("Omitted | {omitted}"));
    }
    if let Some(cost) = &metrics.cost {
        lines.push(format!(
            "Cost estimate | {} {:.6} -> {:.6} | saved {:.6} (input tokens only; not a billing guarantee)",
            cost.currency,
            cost.original_input_cost,
            cost.compressed_input_cost,
            cost.input_cost_saved
        ));
    }
    if result.fallback_used {
        if let Some(reason) = &result.fallback_reason {
            lines.push(format!("Fallback reason | {reason}"));
        }
    }
    lines.join("\n")
}

fn omission_summary(result: &CompressionResult) -> String {
    let o = &result.omissions;
    let mut parts = Vec::new();
    if o.repeated_lines_omitted > 0 {
        parts.push(format!("{} repeated line(s)", o.repeated_lines_omitted));
    }
    if o.passing_test_lines_omitted > 0 {
        parts.push(format!(
            "{} passing test line(s)",
            o.passing_test_lines_omitted
        ));
    }
    if o.progress_lines_omitted > 0 {
        parts.push(format!("{} progress update(s)", o.progress_lines_omitted));
    }
    if o.normalized_log_lines_omitted > 0 {
        parts.push(format!(
            "{} normalized log line(s)",
            o.normalized_log_lines_omitted
        ));
    }
    if o.json_duplicate_items_omitted > 0 {
        parts.push(format!(
            "{} duplicate JSON record(s)",
            o.json_duplicate_items_omitted
        ));
    }
    if o.markers_inserted > 0 {
        parts.push(format!("{} marker(s) inserted", o.markers_inserted));
    }
    if parts.is_empty() {
        "No content omitted".into()
    } else {
        parts.join(", ")
    }
}

pub fn print_compress_diagnostics(result: &CompressionResult, quiet: bool) {
    if quiet {
        return;
    }
    for line in human_summary(result).lines() {
        crate::io_utils::stderr_line(line);
    }
    for warning in &result.warnings {
        crate::io_utils::stderr_line(&format!("warning: {warning}"));
    }
}

pub fn render_comparison_with_original(
    label: &str,
    original: &str,
    result: &CompressionResult,
    context_radius: usize,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("Input: {label}\n"));
    output.push_str(&human_summary(result));
    output.push('\n');
    for warning in &result.warnings {
        output.push_str(&format!("warning: {warning}\n"));
    }
    output.push('\n');
    if original == result.output {
        output.push_str("(no textual differences; original returned)\n");
        return output;
    }
    let diff = TextDiff::from_lines(original, &result.output);
    let rendered = diff
        .unified_diff()
        .context_radius(context_radius)
        .header("original", "contextdrop")
        .to_string();
    output.push_str(rendered.trim_end_matches('\n'));
    output.push('\n');
    output
}

pub fn render_benchmark(report: &CliBenchmarkReport) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "ContextDrop benchmark | engine {} | tokenizer {} | {} file(s)\n",
        report.engine_version,
        report.tokenizer,
        report.files.len()
    ));
    output.push('\n');

    for (index, file) in report.files.iter().enumerate() {
        let run = &file.run;
        output.push_str(&format!("[{}] {}\n", index + 1, run.case.name));
        output.push_str(&format!("    {}\n", file.path));
        let first = run.results.first();
        let kind = first
            .map(|result| result.compression.classification.kind.label())
            .unwrap_or("unknown");
        let automatic = first
            .map(|result| {
                if result.compression.classification.automatic {
                    "automatic"
                } else {
                    "manual"
                }
            })
            .unwrap_or("unknown");
        output.push_str(&format!(
            "    {} | {} | {kind}, {automatic} | {} bytes ({})\n",
            run.case.category,
            run.case.format.as_str(),
            format_count(run.case.original_bytes),
            format_bytes(run.case.original_bytes)
        ));

        let headers = [
            "strategy".to_string(),
            "bytes".to_string(),
            "saved".to_string(),
            "tokens".to_string(),
            "omitted".to_string(),
            "time".to_string(),
            "preserve".to_string(),
            "evidence".to_string(),
        ];
        let rows = run
            .results
            .iter()
            .map(|result| {
                let c = &result.compression;
                let evidence = if result.critical_checks.is_empty() {
                    "-".to_string()
                } else {
                    let passed = result
                        .critical_checks
                        .iter()
                        .filter(|check| check.passed)
                        .count();
                    format!("{passed}/{}", result.critical_checks.len())
                };
                vec![
                    result.strategy.as_str().to_string(),
                    format!(
                        "{} -> {}",
                        format_count(c.metrics.original_bytes),
                        format_count(c.metrics.output_bytes)
                    ),
                    format!("{:.1}%", c.metrics.byte_reduction_percent),
                    format!(
                        "{} -> {}",
                        format_count(c.metrics.original_tokens),
                        format_count(c.metrics.output_tokens)
                    ),
                    c.omissions.total_omitted().to_string(),
                    format!("{:.1} ms", c.metrics.processing_time_ms),
                    if c.preservation.passed {
                        "ok".into()
                    } else {
                        "FAIL".into()
                    },
                    evidence,
                ]
            })
            .collect::<Vec<_>>();

        let mut widths = headers.iter().map(String::len).collect::<Vec<_>>();
        for row in &rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.len());
            }
        }
        output.push_str(&format!(
            "    {:<width0$}  {:<width1$}  {:>width2$}  {:<width3$}  {:>width4$}  {:>width5$}  {:<width6$}  {:<width7$}\n",
            headers[0],
            headers[1],
            headers[2],
            headers[3],
            headers[4],
            headers[5],
            headers[6],
            headers[7],
            width0 = widths[0],
            width1 = widths[1],
            width2 = widths[2],
            width3 = widths[3],
            width4 = widths[4],
            width5 = widths[5],
            width6 = widths[6],
            width7 = widths[7],
        ));
        for row in rows {
            output.push_str(&format!(
                "    {:<width0$}  {:<width1$}  {:>width2$}  {:<width3$}  {:>width4$}  {:>width5$}  {:<width6$}  {:<width7$}\n",
                row[0],
                row[1],
                row[2],
                row[3],
                row[4],
                row[5],
                row[6],
                row[7],
                width0 = widths[0],
                width1 = widths[1],
                width2 = widths[2],
                width3 = widths[3],
                width4 = widths[4],
                width5 = widths[5],
                width6 = widths[6],
                width7 = widths[7],
            ));
        }

        let mut warnings = BTreeSet::new();
        for result in &run.results {
            for warning in &result.compression.warnings {
                warnings.insert(warning.clone());
            }
            for check in &result.critical_checks {
                if !check.passed {
                    warnings.insert(format!("{} ({})", check.detail, result.strategy.as_str()));
                }
            }
        }
        if !warnings.is_empty() {
            output.push('\n');
            for warning in warnings {
                output.push_str(&format!("    warning: {warning}\n"));
            }
        }
        output.push('\n');
    }

    let (passed, total) = report.evidence_totals();
    if total == 0 {
        output.push_str("Evidence assertions | none configured for these inputs\n");
    } else {
        output.push_str(&format!(
            "Evidence assertions | {passed}/{total} expected markers retained across all strategies\n"
        ));
    }
    output.push_str(&format!(
        "Result | {}\n",
        if report.all_checks_passed() {
            "PASS"
        } else {
            "FAIL (required evidence or preservation check failed)"
        }
    ));
    output
}

pub fn format_count(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator_is_stable() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
    }

    #[test]
    fn comparison_includes_real_unified_diff() {
        let input = format!("{}error: stop\n", "noise\n".repeat(12));
        let result =
            contextdrop_core::compress(&input, &contextdrop_core::CompressionConfig::default())
                .unwrap();
        assert!(!result.fallback_used);
        let rendered = render_comparison_with_original("sample.log", &input, &result, 3);
        assert!(rendered.contains("--- original"));
        assert!(rendered.contains("+++ contextdrop"));
        assert!(rendered.contains("error: stop"));
    }
}

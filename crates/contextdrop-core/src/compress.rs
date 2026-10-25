//! Orchestration for the deterministic compression pipeline.

use std::time::Instant;

use crate::classify::classify;
use crate::json::compress_json;
use crate::text::{compress_text, split_content_lines};
use crate::tokenizer::count;
use crate::types::{
    CompressionConfig, CompressionResult, CostEstimate, InputKind, Metrics, Omissions,
    PreservationCheck, Strategy,
};
use crate::validate::validate_output;
use crate::{Error, Result};

/// Compress text locally using the selected deterministic strategy.
///
/// No I/O, subprocess execution, network access, or model calls occur here.
pub fn compress(input: &str, config: &CompressionConfig) -> Result<CompressionResult> {
    let started = Instant::now();
    config.validate()?;
    if input.len() > config.max_input_bytes {
        return Err(Error::InputTooLarge {
            actual: input.len(),
            limit: config.max_input_bytes,
        });
    }

    let classification = classify(input, config.input_format);

    let attempted = match classification.kind {
        InputKind::Json => {
            let attempt = compress_json(input, config)?;
            Attempt {
                output: attempt.output,
                omissions: attempt.omissions,
                warnings: attempt.warnings,
            }
        }
        _ => {
            let attempt = compress_text(input, config, classification.kind);
            Attempt {
                output: attempt.output,
                omissions: attempt.omissions,
                warnings: attempt.warnings,
            }
        }
    };

    let validation = validate_output(
        input,
        &attempted.output,
        classification.kind,
        &config.preserve_patterns,
    );

    let candidate_changed = attempted.output != input;
    let mut fallback_reason: Option<String> = None;
    if !validation.passed {
        let failed_details = validation
            .checks
            .iter()
            .filter(|check| !check.passed)
            .map(|check| check.detail.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        fallback_reason = Some(format!(
            "Safety validation rejected the candidate transformation. {failed_details}"
        ));
    } else if candidate_changed && attempted.output.len() >= input.len() {
        fallback_reason =
            Some("The candidate transformation did not reduce the input byte size.".into());
    }

    let fallback_used = fallback_reason.is_some();
    let transformation_applied = fallback_reason.is_none() && candidate_changed;
    let output = if fallback_used || !candidate_changed {
        input.to_string()
    } else {
        attempted.output
    };

    let mut warnings = if fallback_used || !candidate_changed {
        Vec::new()
    } else {
        attempted.warnings
    };

    if let Some(reason) = &fallback_reason {
        warnings.push(format!(
            "{reason} The original input was returned unchanged."
        ));
    } else if !candidate_changed {
        if input.is_empty() {
            warnings.push("Input is empty; there was nothing to compress.".into());
        } else {
            warnings.push(
                "No qualifying deterministic redundancy was found; the original input was returned unchanged."
                    .into(),
            );
        }
    }

    if transformation_applied && config.strategy == Strategy::Aggressive {
        warnings.push(
            "Aggressive mode may omit useful non-protected context. Review the diff before using the output."
                .into(),
        );
    }

    if classification.automatic
        && classification.kind == InputKind::PlainText
        && classification.confidence < 0.7
        && !input.trim().is_empty()
    {
        warnings.push(
            "Automatic classification was inconclusive, so plain-text handling was used. Select a format manually to apply specialized parsing."
                .into(),
        );
    }

    let omissions = if fallback_used || !candidate_changed {
        empty_omissions(input)
    } else {
        attempted.omissions
    };

    // Token counting (including one-time vocabulary initialization) is reported
    // in its own metadata and excluded from the transformation timing so it does
    // not distort strategy comparisons.
    let processing_time_ms = round_to(started.elapsed().as_secs_f64() * 1000.0, 3);
    let original_tokens = count(input, config.tokenizer);
    let output_tokens = count(&output, config.tokenizer);

    let metrics = build_metrics(
        input,
        &output,
        &original_tokens,
        &output_tokens,
        processing_time_ms,
        config,
    );

    let mut preservation = if fallback_used || !candidate_changed {
        let mut report = validate_output(
            input,
            &output,
            classification.kind,
            &config.preserve_patterns,
        );
        if let Some(reason) = &fallback_reason {
            report.checks.push(PreservationCheck {
                id: "fallback".into(),
                label: "Candidate transformation rejected".into(),
                passed: true,
                detail: format!("{reason} The original input was returned."),
            });
        }
        report
    } else {
        validation
    };
    preservation.checks.push(PreservationCheck {
        id: "byte-metrics".into(),
        label: "Byte counts verified".into(),
        passed: metrics.original_bytes == input.len()
            && metrics.output_bytes == output.len(),
        detail: format!(
            "Original: {} bytes; output: {} bytes. Both values were measured directly from the returned strings.",
            metrics.original_bytes, metrics.output_bytes
        ),
    });
    preservation.passed = preservation.checks.iter().all(|check| check.passed);

    Ok(CompressionResult {
        output,
        strategy: config.strategy,
        classification,
        metrics,
        omissions,
        preservation,
        warnings,
        fallback_used,
        fallback_reason,
        transformation_applied,
    })
}

struct Attempt {
    output: String,
    omissions: Omissions,
    warnings: Vec<String>,
}

fn empty_omissions(input: &str) -> Omissions {
    let lines = if input.is_empty() {
        0
    } else {
        split_content_lines(input).len()
    };
    Omissions {
        total_input_lines: lines,
        total_output_lines: lines,
        ..Omissions::default()
    }
}

fn build_metrics(
    input: &str,
    output: &str,
    original_tokens: &crate::tokenizer::TokenCount,
    output_tokens: &crate::tokenizer::TokenCount,
    processing_time_ms: f64,
    config: &CompressionConfig,
) -> Metrics {
    let original_bytes = input.len();
    let output_bytes = output.len();
    let original_chars = input.chars().count();
    let output_chars = output.chars().count();
    let byte_reduction_percent = percent(original_bytes, output_bytes);
    let token_reduction_percent = percent(original_tokens.count, output_tokens.count);
    let cost = config.pricing.as_ref().map(|pricing| {
        let original_input_cost =
            original_tokens.count as f64 / 1_000_000.0 * pricing.input_per_million;
        let compressed_input_cost =
            output_tokens.count as f64 / 1_000_000.0 * pricing.input_per_million;
        CostEstimate {
            currency: pricing.currency.clone(),
            input_price_per_million: pricing.input_per_million,
            original_input_cost: round_to(original_input_cost, 8),
            compressed_input_cost: round_to(compressed_input_cost, 8),
            input_cost_saved: round_to(original_input_cost - compressed_input_cost, 8),
            note: "Estimate only, based on the user-supplied input-token price. Caching, batch discounts, provider rules, retries, and tokenizer differences can change actual billing. Output/completion pricing is not applicable to context reduction.".into(),
        }
    });

    Metrics {
        original_bytes,
        output_bytes,
        bytes_saved: original_bytes as i64 - output_bytes as i64,
        byte_reduction_percent,
        original_chars,
        output_chars,
        chars_saved: original_chars as i64 - output_chars as i64,
        original_tokens: original_tokens.count,
        output_tokens: output_tokens.count,
        tokens_saved: original_tokens.count as i64 - output_tokens.count as i64,
        token_reduction_percent,
        token_measurement: original_tokens.measurement.clone(),
        processing_time_ms,
        cost,
    }
}

fn percent(original: usize, output: usize) -> f64 {
    if original == 0 {
        return 0.0;
    }
    round_to(
        (original as f64 - output as f64) / original as f64 * 100.0,
        2,
    )
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InputFormat, TokenizerKind};

    fn config(strategy: Strategy) -> CompressionConfig {
        CompressionConfig {
            strategy,
            tokenizer: TokenizerKind::Estimate,
            ..CompressionConfig::default()
        }
    }

    #[test]
    fn metrics_match_returned_strings() {
        let input = "a\nb\na\nb\na\nb\n";
        let result = compress(input, &config(Strategy::Conservative)).unwrap();
        assert_eq!(result.metrics.original_bytes, input.len());
        assert_eq!(result.metrics.output_bytes, result.output.len());
        assert_eq!(
            result.metrics.bytes_saved,
            input.len() as i64 - result.output.len() as i64
        );
        assert!(result.preservation.passed);
    }

    #[test]
    fn deterministic_for_same_input_and_config() {
        let input = "x\nx\nx\nerror: stop\n";
        let first = compress(input, &config(Strategy::Aggressive)).unwrap();
        let second = compress(input, &config(Strategy::Aggressive)).unwrap();
        assert_eq!(first.output, second.output);
        assert_eq!(first.metrics.bytes_saved, second.metrics.bytes_saved);
        assert_eq!(
            first.omissions.total_omitted(),
            second.omissions.total_omitted()
        );
    }

    #[test]
    fn empty_input_is_successful() {
        let result = compress("", &config(Strategy::Balanced)).unwrap();
        assert_eq!(result.output, "");
        assert!(!result.transformation_applied);
        assert!(!result.fallback_used);
    }

    #[test]
    fn forced_invalid_json_errors() {
        let config = CompressionConfig {
            input_format: InputFormat::Json,
            ..config(Strategy::Balanced)
        };
        let error = compress("{ not json", &config).unwrap_err();
        assert!(matches!(error, Error::InvalidJson { .. }));
    }

    #[test]
    fn oversized_input_errors_before_processing() {
        let config = CompressionConfig {
            max_input_bytes: 4,
            ..config(Strategy::Conservative)
        };
        let error = compress("12345", &config).unwrap_err();
        assert!(matches!(error, Error::InputTooLarge { .. }));
    }

    #[test]
    fn cost_is_estimated_only_when_pricing_is_supplied() {
        let plain = compress(
            "same\nsame\nsame\nsame\nsame\n",
            &config(Strategy::Conservative),
        )
        .unwrap();
        assert!(plain.metrics.cost.is_none());

        let priced = CompressionConfig {
            pricing: Some(crate::types::Pricing {
                input_per_million: 3.0,
                currency: "USD".into(),
            }),
            ..config(Strategy::Conservative)
        };
        let result = compress("same\nsame\nsame\nsame\nsame\n", &priced).unwrap();
        assert!(result.metrics.cost.is_some());
        assert_eq!(result.metrics.cost.as_ref().unwrap().currency, "USD");
    }

    #[test]
    fn user_preserve_patterns_win_over_compression() {
        let config = CompressionConfig {
            preserve_patterns: vec!["request-id".into()],
            strategy: Strategy::Aggressive,
            ..CompressionConfig::default()
        };
        let input = "request-id=123\nrequest-id=123\nrequest-id=123\n";
        let result = compress(input, &config).unwrap();
        assert_eq!(result.output, input);
        assert_eq!(result.omissions.total_omitted(), 0);
    }
}

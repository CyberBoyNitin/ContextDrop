//! Post-transformation safety validation.
//!
//! Validation is deterministic and rule-based. It can establish that protected
//! evidence is still present; it cannot prove semantic equivalence.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::json::evidence_counts;
use crate::protect::Protector;
use crate::text::split_content_lines;
use crate::types::{InputKind, PreservationCheck, PreservationReport};

pub(crate) fn validate_output(
    input: &str,
    output: &str,
    kind: InputKind,
    preserve_patterns: &[String],
) -> PreservationReport {
    if kind == InputKind::Json {
        validate_json(input, output, preserve_patterns)
    } else {
        validate_lines(input, output, preserve_patterns)
    }
}

fn validate_lines(input: &str, output: &str, preserve_patterns: &[String]) -> PreservationReport {
    let protector = Protector::new(preserve_patterns);
    let input_lines = split_content_lines(input);
    let output_lines = split_content_lines(output);

    let mut expected: BTreeMap<String, usize> = BTreeMap::new();
    for (index, line) in input_lines.iter().enumerate() {
        if protector.inspect_line(line, index == 0).protected {
            *expected.entry(line.clone()).or_insert(0) += 1;
        }
    }

    let mut available: BTreeMap<String, usize> = BTreeMap::new();
    for line in &output_lines {
        if expected.contains_key(line) {
            *available.entry(line.clone()).or_insert(0) += 1;
        }
    }

    let protected_occurrences = expected.values().sum::<usize>();
    let retained_occurrences = expected
        .iter()
        .map(|(line, count)| (*count).min(*available.get(line).unwrap_or(&0)))
        .sum::<usize>();
    let mut missing = Vec::new();
    for (line, count) in &expected {
        let present = available.get(line).copied().unwrap_or(0);
        if present < *count {
            missing.push(format!(
                "{} occurrence(s) of {:?} missing",
                count - present,
                truncate(line, 80)
            ));
            if missing.len() == 3 {
                break;
            }
        }
    }

    let passed = missing.is_empty();
    let detail = if passed {
        format!(
            "All {protected_occurrences} protected line occurrence(s) from the original are present in the output."
        )
    } else {
        format!(
            "The candidate output did not retain all protected lines: {}.",
            missing.join("; ")
        )
    };

    let checks = vec![
        PreservationCheck {
            id: "protected-lines".into(),
            label: "Protected lines retained".into(),
            passed,
            detail,
        },
        PreservationCheck {
            id: "omission-policy".into(),
            label: "Omission policy".into(),
            passed: true,
            detail: "Only non-protected spans were eligible for omission. Lines adjacent to protected evidence were also excluded from omission.".into(),
        },
    ];

    PreservationReport {
        passed,
        protected_occurrences,
        retained_occurrences,
        checks,
        note: "This is a deterministic presence check for the documented rules. It does not prove semantic equivalence or catch every kind of important information.".into(),
    }
}

fn validate_json(input: &str, output: &str, preserve_patterns: &[String]) -> PreservationReport {
    let input_value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(error) => {
            return PreservationReport {
                passed: false,
                protected_occurrences: 0,
                retained_occurrences: 0,
                checks: vec![PreservationCheck {
                    id: "input-valid-json".into(),
                    label: "Input is valid JSON".into(),
                    passed: false,
                    detail: format!("The original could not be parsed: {error}"),
                }],
                note: "JSON safety validation could not run on unparseable input.".into(),
            };
        }
    };
    let expected = evidence_counts(&input_value, preserve_patterns);
    let protected_occurrences = expected.values().sum::<usize>();

    let output_value: Value = match serde_json::from_str(output) {
        Ok(value) => value,
        Err(error) => {
            return PreservationReport {
                passed: false,
                protected_occurrences,
                retained_occurrences: 0,
                checks: vec![
                    PreservationCheck {
                        id: "json-valid".into(),
                        label: "Output is valid JSON".into(),
                        passed: false,
                        detail: format!("The candidate output failed JSON parsing: {error}"),
                    },
                    PreservationCheck {
                        id: "json-evidence".into(),
                        label: "Protected JSON evidence retained".into(),
                        passed: false,
                        detail:
                            "Evidence comparison could not run because the output was invalid JSON."
                                .into(),
                    },
                ],
                note: "Invalid JSON transformations are rejected and the original is returned."
                    .into(),
            };
        }
    };

    let available = evidence_counts(&output_value, preserve_patterns);
    let retained_occurrences = expected
        .iter()
        .map(|(key, count)| (*count).min(*available.get(key).unwrap_or(&0)))
        .sum::<usize>();
    let missing = expected
        .iter()
        .filter(|(key, count)| available.get(*key).copied().unwrap_or(0) < **count)
        .take(3)
        .map(|(key, _)| truncate(key, 100))
        .collect::<Vec<_>>();
    let evidence_passed = missing.is_empty();

    let checks = vec![
        PreservationCheck {
            id: "json-valid".into(),
            label: "Output is valid JSON".into(),
            passed: true,
            detail: "The compressed output parses as valid JSON.".into(),
        },
        PreservationCheck {
            id: "json-evidence".into(),
            label: "Protected JSON evidence retained".into(),
            passed: evidence_passed,
            detail: if evidence_passed {
                format!(
                    "All {protected_occurrences} protected JSON evidence occurrence(s) are present in the output."
                )
            } else {
                format!(
                    "Protected JSON evidence was missing after transformation: {}.",
                    missing.join("; ")
                )
            },
        },
    ];

    PreservationReport {
        passed: evidence_passed,
        protected_occurrences,
        retained_occurrences,
        checks,
        note: "JSON validation checks syntax and a rule-based multiset of protected evidence. It cannot prove that the output is schema-compatible with every consumer.".into(),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut result: String = text.chars().take(max_chars).collect();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_validation_detects_missing_protected_text() {
        let input = "error: disk full\nprogress\n";
        let output = "progress\n";
        let report = validate_output(input, output, InputKind::PlainText, &[]);
        assert!(!report.passed);
        assert!(report
            .checks
            .iter()
            .any(|check| check.id == "protected-lines" && !check.passed));
    }

    #[test]
    fn line_validation_passes_when_protected_text_is_present() {
        let input = "error: disk full\nprogress\n";
        let output = "[protected]\nerror: disk full\n";
        let report = validate_output(input, output, InputKind::PlainText, &[]);
        assert!(report.passed);
        assert_eq!(report.protected_occurrences, report.retained_occurrences);
    }

    #[test]
    fn invalid_json_output_fails() {
        let input = r#"{"error":"bad"}"#;
        let report = validate_output(input, "{", InputKind::Json, &[]);
        assert!(!report.passed);
        assert!(report.checks.iter().any(|check| check.id == "json-valid"));
    }

    #[test]
    fn json_evidence_survives_reformatting() {
        let input = "{\n  \"error\": \"disk full\",\n  \"status\": 500\n}\n";
        let output = r#"{"error":"disk full","status":500}"#;
        let report = validate_output(input, output, InputKind::Json, &[]);
        assert!(report.passed);
    }

    #[test]
    fn user_patterns_are_included_in_json_validation() {
        let input = r#"{"tenant":"AcmeCorp","ok":true}"#;
        let output = r#"{"tenant":"other","ok":true}"#;
        let report = validate_output(input, output, InputKind::Json, &["AcmeCorp".to_string()]);
        assert!(!report.passed);
    }
}

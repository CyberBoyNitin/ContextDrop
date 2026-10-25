//! Conservative input classification.
//!
//! When there is not enough evidence for a specialized format, the engine
//! deliberately falls back to line-oriented plain-text handling.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::types::{Classification, InputFormat, InputKind};

const MAX_CLASSIFICATION_LINES: usize = 20_000;

pub(crate) fn classify(text: &str, requested: InputFormat) -> Classification {
    if let Some(kind) = requested.kind() {
        return Classification {
            kind,
            automatic: false,
            confidence: 1.0,
        };
    }

    if text.trim().is_empty() {
        return Classification {
            kind: InputKind::PlainText,
            automatic: true,
            confidence: 1.0,
        };
    }

    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<Value>(text).is_ok()
    {
        return Classification {
            kind: InputKind::Json,
            automatic: true,
            confidence: 0.99,
        };
    }

    let all_lines = text
        .lines()
        .take(MAX_CLASSIFICATION_LINES)
        .collect::<Vec<_>>();
    let lines = all_lines
        .iter()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return Classification {
            kind: InputKind::PlainText,
            automatic: true,
            confidence: 1.0,
        };
    }

    if looks_like_json_lines(&lines) {
        return Classification {
            kind: InputKind::StructuredLog,
            automatic: true,
            confidence: 0.9,
        };
    }

    let (test_score, test_hits) = test_score(&lines);
    let (build_score, build_hits) = build_score(&lines);
    let (structured_score, structured_hits) = structured_score(&lines);
    let (terminal_score, terminal_hits) = terminal_score(text, &lines);
    let count = lines.len() as f64;

    // Test runners often contain build output too, so a credible test signature
    // wins over build markers.
    if test_score >= 3.0 && test_hits >= 2 && test_score >= build_score * 0.75 {
        return classified(InputKind::TestOutput, test_score, count);
    }
    if build_score >= 3.0 && build_hits >= 2 {
        return classified(InputKind::BuildLog, build_score, count);
    }
    if structured_score >= 3.0 && structured_hits >= 3 && structured_hits as f64 / count >= 0.35 {
        return classified(InputKind::StructuredLog, structured_score, count);
    }
    if test_score >= 2.0 && test_hits >= 2 {
        return classified(InputKind::TestOutput, test_score, count);
    }
    if terminal_score >= 3.0 && terminal_hits >= 2 {
        return classified(InputKind::Terminal, terminal_score, count);
    }

    Classification {
        kind: InputKind::PlainText,
        automatic: true,
        confidence: 0.55,
    }
}

fn classified(kind: InputKind, score: f64, line_count: f64) -> Classification {
    let density = score / (score + line_count.min(30.0) * 0.15);
    let confidence = (0.55 + density * 0.4).clamp(0.55, 0.96);
    Classification {
        kind,
        automatic: true,
        confidence: (confidence * 100.0).round() / 100.0,
    }
}

fn looks_like_json_lines(lines: &[&str]) -> bool {
    if lines.len() < 3 {
        return false;
    }
    let sample = lines.iter().take(40);
    let mut total = 0usize;
    let mut valid = 0usize;
    for line in sample {
        let trimmed = line.trim();
        if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
            continue;
        }
        total += 1;
        if serde_json::from_str::<Value>(trimmed).is_ok() {
            valid += 1;
        }
    }
    valid >= 3 && total >= 3 && valid as f64 / total as f64 >= 0.8
}

fn test_score(lines: &[&str]) -> (f64, usize) {
    static RUST_TEST: OnceLock<Regex> = OnceLock::new();
    static GO_TEST: OnceLock<Regex> = OnceLock::new();
    static PY_TEST: OnceLock<Regex> = OnceLock::new();
    static JEST_SUMMARY: OnceLock<Regex> = OnceLock::new();
    static PASS_MARK: OnceLock<Regex> = OnceLock::new();

    let rust_test = RUST_TEST.get_or_init(|| {
        Regex::new(r"(?i)^\s*test\s+.+\s+\.\.\.\s+(?:ok|FAILED|ignored)\b").unwrap()
    });
    let go_test =
        GO_TEST.get_or_init(|| Regex::new(r"^\s*---\s+(?:PASS|FAIL|SKIP):\s+\S+").unwrap());
    let py_test = PY_TEST.get_or_init(|| Regex::new(r"^\s*(?:PASSED|FAILED)\s+\S+").unwrap());
    let jest_summary =
        JEST_SUMMARY.get_or_init(|| Regex::new(r"^\s*(?:Test Suites|Tests):\s+\d+").unwrap());
    let pass_mark = PASS_MARK.get_or_init(|| Regex::new(r"^\s*[✓✔✗✘❌]").unwrap());

    let mut score = 0.0;
    let mut hits = 0;
    for line in lines {
        let mut matched = false;
        if line.contains("test result:") {
            score += 3.0;
            matched = true;
        }
        if line.contains("running ") && line.contains("test") {
            score += 2.0;
            matched = true;
        }
        if rust_test.is_match(line) {
            score += 2.0;
            matched = true;
        }
        if go_test.is_match(line) {
            score += 2.0;
            matched = true;
        }
        if py_test.is_match(line) {
            score += 2.0;
            matched = true;
        }
        if jest_summary.is_match(line) {
            score += 3.0;
            matched = true;
        }
        if pass_mark.is_match(line) {
            score += 1.0;
            matched = true;
        }
        if line.to_ascii_lowercase().contains("pytest") {
            score += 1.0;
            matched = true;
        }
        if matched {
            hits += 1;
        }
    }
    (score, hits)
}

fn build_score(lines: &[&str]) -> (f64, usize) {
    static BUILD_MARKERS: OnceLock<Regex> = OnceLock::new();
    let markers = BUILD_MARKERS.get_or_init(|| {
        Regex::new(
            r"(?i)(?:^\s*(?:Compiling|Finished|Building|Blocking)\s+\S|^\s*error(?:\[[A-Z0-9]+\])?\s*:|^\s*warning\s*:|npm ERR!|error TS\d+|BUILD (?:SUCCESSFUL|FAILED)|Could not resolve|make(?:\[\d+\])?:|dotnet build|MSBuild|eslint|webpack|vite build|next build|^\s*>\s+\S+@\S+\s+(?:build|watch))",
        )
        .unwrap()
    });
    let mut score = 0.0;
    let mut hits = 0;
    for line in lines {
        if markers.is_match(line) {
            hits += 1;
            score += if line.to_ascii_lowercase().contains("error") {
                2.0
            } else {
                1.0
            };
        }
    }
    (score, hits)
}

fn structured_score(lines: &[&str]) -> (f64, usize) {
    static TIMESTAMP: OnceLock<Regex> = OnceLock::new();
    static LEVEL: OnceLock<Regex> = OnceLock::new();
    static LOGFMT: OnceLock<Regex> = OnceLock::new();

    let timestamp = TIMESTAMP.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*(?:\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?|\[[A-Z][a-z]{2} [A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}|[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})",
        )
        .unwrap()
    });
    let level = LEVEL.get_or_init(|| {
        Regex::new(r"(?i)\b(?:TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|FATAL|CRITICAL)\b").unwrap()
    });
    let logfmt = LOGFMT.get_or_init(|| Regex::new(r"\b[a-z_][a-z0-9_]*=[^\s]+").unwrap());

    let mut score = 0.0;
    let mut hits = 0;
    for line in lines {
        let mut matched = false;
        let trimmed = line.trim_start();
        if trimmed.starts_with('{') && serde_json::from_str::<Value>(trimmed).is_ok() {
            score += 2.0;
            matched = true;
        }
        if timestamp.is_match(line) {
            score += 1.0;
            matched = true;
        }
        if level.is_match(line) {
            score += 0.5;
            matched = true;
        }
        if logfmt.find_iter(line).count() >= 2 {
            score += 1.0;
            matched = true;
        }
        if matched {
            hits += 1;
        }
    }
    (score, hits)
}

fn terminal_score(text: &str, lines: &[&str]) -> (f64, usize) {
    static PROMPT: OnceLock<Regex> = OnceLock::new();
    let prompt = PROMPT.get_or_init(|| {
        Regex::new(
            r"(?im)^\s*(?:[$>]\s+\S|PS\s+[^>\r\n]*>\s*\S|(?:command|cmd|args|argv)\s*[:=]\s*\S)",
        )
        .unwrap()
    });
    let mut score = 0.0;
    let mut hits = 0;
    if text.contains("\u{1b}[") {
        score += 4.0;
        hits += 2;
    }
    for line in lines {
        if prompt.is_match(line) {
            score += 2.0;
            hits += 1;
        }
    }
    (score, hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_objects_and_arrays_are_detected() {
        assert_eq!(
            classify(r#"{"a":1}"#, InputFormat::Auto).kind,
            InputKind::Json
        );
        assert_eq!(classify("[1,2,3]", InputFormat::Auto).kind, InputKind::Json);
    }

    #[test]
    fn broken_json_does_not_claim_json_classification() {
        assert_eq!(
            classify(r#"{"a": "#, InputFormat::Auto).kind,
            InputKind::PlainText
        );
    }

    #[test]
    fn manual_format_overrides_detection() {
        let classification = classify(r#"{"a":1}"#, InputFormat::PlainText);
        assert_eq!(classification.kind, InputKind::PlainText);
        assert!(!classification.automatic);
        assert_eq!(classification.confidence, 1.0);
    }

    #[test]
    fn rust_test_output_is_recognized() {
        let sample = "\
running 2 tests
test parser::accepts_valid ... ok
test parser::rejects_bad ... FAILED
test result: FAILED. 1 passed; 1 failed; 0 ignored
";
        assert_eq!(
            classify(sample, InputFormat::Auto).kind,
            InputKind::TestOutput
        );
    }

    #[test]
    fn rust_build_errors_are_recognized() {
        let sample = "\
   Compiling demo v0.1.0
error[E0308]: mismatched types
 --> src/main.rs:10:5
error: could not compile `demo` (bin \"demo\")
";
        assert_eq!(
            classify(sample, InputFormat::Auto).kind,
            InputKind::BuildLog
        );
    }

    #[test]
    fn ordinary_prose_defaults_to_plain_text() {
        let sample = "A short note about the release.\nIt contains no logs or errors.\n";
        assert_eq!(
            classify(sample, InputFormat::Auto).kind,
            InputKind::PlainText
        );
    }

    #[test]
    fn json_lines_are_recognized_as_structured_logs() {
        let sample = "\
{\"level\":\"info\",\"msg\":\"started\"}
{\"level\":\"info\",\"msg\":\"ready\"}
{\"level\":\"error\",\"msg\":\"failed\"}
";
        assert_eq!(
            classify(sample, InputFormat::Auto).kind,
            InputKind::StructuredLog
        );
    }
}

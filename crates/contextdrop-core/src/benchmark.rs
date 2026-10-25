//! Reproducible benchmark execution shared by the CLI and local API.
//!
//! A benchmark always runs the real compression engine. Critical-evidence
//! checks are explicit substring assertions supplied by a fixture manifest;
//! they are never inferred from compression ratios.

use serde::{Deserialize, Serialize};

use crate::compress::compress;
use crate::types::{CompressionConfig, CompressionResult, InputFormat, Strategy};
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureManifest {
    #[serde(default = "manifest_version")]
    pub version: u32,
    pub fixtures: Vec<FixtureSpec>,
}

fn manifest_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file: String,
    pub category: String,
    #[serde(default)]
    pub format: InputFormat,
    #[serde(default)]
    pub must_preserve: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BenchmarkSource {
    BuiltIn,
    User,
}

#[derive(Debug, Clone)]
pub struct BenchmarkCase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub format: InputFormat,
    pub source: BenchmarkSource,
    pub content: String,
    pub must_preserve: Vec<String>,
    pub notes: Option<String>,
}

impl BenchmarkCase {
    pub fn from_fixture(spec: &FixtureSpec, content: String) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            description: spec.description.clone(),
            category: spec.category.clone(),
            format: spec.format,
            source: BenchmarkSource::BuiltIn,
            content,
            must_preserve: spec.must_preserve.clone(),
            notes: spec.notes.clone(),
        }
    }

    pub fn user(name: String, content: String, format: InputFormat) -> Self {
        Self {
            id: "user-input".into(),
            name,
            description: "User-supplied benchmark input".into(),
            category: "Custom".into(),
            format,
            source: BenchmarkSource::User,
            content,
            must_preserve: Vec::new(),
            notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCaseInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub format: InputFormat,
    pub source: BenchmarkSource,
    pub original_bytes: usize,
    pub must_preserve: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalCheck {
    pub label: String,
    pub pattern: String,
    pub found_in_original: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStrategyResult {
    pub strategy: Strategy,
    pub compression: CompressionResult,
    pub critical_checks: Vec<CriticalCheck>,
}

impl BenchmarkStrategyResult {
    pub fn critical_checks_passed(&self) -> bool {
        self.critical_checks.iter().all(|check| check.passed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub case: BenchmarkCaseInfo,
    pub results: Vec<BenchmarkStrategyResult>,
}

impl BenchmarkRun {
    pub fn all_critical_checks_passed(&self) -> bool {
        self.results
            .iter()
            .all(BenchmarkStrategyResult::critical_checks_passed)
    }
}

/// Run the real engine with all three strategies against one case.
pub fn run_benchmark(
    case: &BenchmarkCase,
    base_config: &CompressionConfig,
) -> Result<BenchmarkRun> {
    let mut base_config = base_config.clone();
    base_config.input_format = case.format;
    // One-time regex initialization would otherwise be charged to whichever
    // strategy happens to run first. Warm the rule sets with discarded inputs so
    // strategy timings reflect transformation work, not lazy initialization.
    warm_rule_engines(&base_config);

    let mut results = Vec::with_capacity(Strategy::all().len());
    for strategy in Strategy::all() {
        base_config.strategy = strategy;
        let mut compression = compress(&case.content, &base_config)?;
        let critical_checks = case
            .must_preserve
            .iter()
            .map(|pattern| {
                let found_in_original = case.content.contains(pattern);
                let retained = compression.output.contains(pattern);
                let passed = found_in_original && retained;
                let detail = if !found_in_original {
                    format!(
                        "Fixture metadata error: the expected evidence is absent from the original input ({pattern:?})."
                    )
                } else if retained {
                    "Expected evidence is present in the processed output.".into()
                } else {
                    "Expected evidence is missing from the processed output.".into()
                };
                CriticalCheck {
                    label: format!("Retains evidence: {pattern}"),
                    pattern: pattern.clone(),
                    found_in_original,
                    passed,
                    detail,
                }
            })
            .collect::<Vec<_>>();

        if !critical_checks.iter().all(|check| check.passed) {
            let failed = critical_checks
                .iter()
                .filter(|check| !check.passed)
                .map(|check| check.pattern.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            compression.warnings.push(format!(
                "Benchmark evidence assertion failed for pattern(s): {failed}"
            ));
        }

        results.push(BenchmarkStrategyResult {
            strategy,
            compression,
            critical_checks,
        });
    }

    Ok(BenchmarkRun {
        case: BenchmarkCaseInfo {
            id: case.id.clone(),
            name: case.name.clone(),
            description: case.description.clone(),
            category: case.category.clone(),
            format: case.format,
            source: case.source,
            original_bytes: case.content.len(),
            must_preserve: case.must_preserve.clone(),
            notes: case.notes.clone(),
        },
        results,
    })
}

fn warm_rule_engines(base: &CompressionConfig) {
    let mut config = base.clone();
    config.tokenizer = crate::types::TokenizerKind::Estimate;
    config.input_format = InputFormat::PlainText;
    config.strategy = Strategy::Aggressive;
    config.preserve_patterns.clear();
    let mut sample = String::from("$ warm up\\nwarning: warm-up line\\nsame\\nsame\\nsame\\n");
    for percent in [10, 20, 30] {
        sample.push_str(&format!(
            "[##########----------] {percent}% downloading packages\\n"
        ));
    }
    sample.push_str("test warm::a ... ok\\ntest warm::b ... ok\\nerror: warm-up\\n");
    let _ = compress(&sample, &config);

    config.input_format = InputFormat::StructuredLog;
    let structured = "2026-10-20T14:00:00.000Z INFO warmup request_id=1 duration_ms=1\\n\
2026-10-20T14:00:01.000Z INFO warmup request_id=2 duration_ms=2\\n\
2026-10-20T14:00:02.000Z INFO warmup request_id=3 duration_ms=3\\n";
    let _ = compress(structured, &config);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(content: &str, must_preserve: Vec<&str>) -> BenchmarkCase {
        BenchmarkCase {
            id: "test".into(),
            name: "Test".into(),
            description: "Test fixture".into(),
            category: "Test".into(),
            format: InputFormat::Auto,
            source: BenchmarkSource::BuiltIn,
            content: content.into(),
            must_preserve: must_preserve.into_iter().map(str::to_string).collect(),
            notes: None,
        }
    }

    #[test]
    fn benchmark_runs_all_three_strategies() {
        let run = run_benchmark(
            &case("error: stop\nok\nok\n", vec!["error: stop"]),
            &CompressionConfig::default(),
        )
        .unwrap();
        assert_eq!(run.results.len(), 3);
        assert!(run.all_critical_checks_passed());
    }

    #[test]
    fn missing_fixture_evidence_is_an_explicit_failure() {
        let run = run_benchmark(
            &case("hello\n", vec!["error: imaginary"]),
            &CompressionConfig::default(),
        )
        .unwrap();
        assert!(!run.all_critical_checks_passed());
        assert!(!run.results[0].critical_checks[0].found_in_original);
    }
}

//! Core data types shared by the engine, CLI, and HTTP API.

use serde::{Deserialize, Serialize};

use crate::Error;

/// Compression intensity selected by the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
}

impl Strategy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Balanced => "balanced",
            Self::Aggressive => "aggressive",
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::Conservative, Self::Balanced, Self::Aggressive]
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Conservative => "Removes only consecutive repeated lines.",
            Self::Balanced => {
                "Also compresses passing test entries, progress updates, and repeated JSON records."
            }
            Self::Aggressive => {
                "Adds global duplicate removal and normalized structured-log grouping. May lose non-protected context."
            }
        }
    }
}

/// Input format selected by the user.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputFormat {
    #[default]
    Auto,
    PlainText,
    Terminal,
    BuildLog,
    TestOutput,
    Json,
    StructuredLog,
}

impl InputFormat {
    pub const fn kind(self) -> Option<InputKind> {
        match self {
            Self::Auto => None,
            Self::PlainText => Some(InputKind::PlainText),
            Self::Terminal => Some(InputKind::Terminal),
            Self::BuildLog => Some(InputKind::BuildLog),
            Self::TestOutput => Some(InputKind::TestOutput),
            Self::Json => Some(InputKind::Json),
            Self::StructuredLog => Some(InputKind::StructuredLog),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::PlainText => "plain-text",
            Self::Terminal => "terminal",
            Self::BuildLog => "build-log",
            Self::TestOutput => "test-output",
            Self::Json => "json",
            Self::StructuredLog => "structured-log",
        }
    }
}

/// The resolved (non-automatic) input classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputKind {
    PlainText,
    Terminal,
    BuildLog,
    TestOutput,
    Json,
    StructuredLog,
}

impl InputKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "plain-text",
            Self::Terminal => "terminal",
            Self::BuildLog => "build-log",
            Self::TestOutput => "test-output",
            Self::Json => "json",
            Self::StructuredLog => "structured-log",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::PlainText => "Plain text",
            Self::Terminal => "Terminal output",
            Self::BuildLog => "Build log",
            Self::TestOutput => "Test-runner output",
            Self::Json => "JSON",
            Self::StructuredLog => "Structured log",
        }
    }
}

/// Tokenizer family used for token measurements.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenizerKind {
    #[serde(rename = "o200k_base")]
    #[default]
    O200kBase,
    #[serde(rename = "cl100k_base")]
    Cl100kBase,
    #[serde(rename = "estimate")]
    Estimate,
}

impl TokenizerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::O200kBase => "o200k_base",
            Self::Cl100kBase => "cl100k_base",
            Self::Estimate => "estimate",
        }
    }
}

/// Optional, user-provided pricing used only for a labelled cost estimate.
///
/// ContextDrop reduces prompt-side text. Therefore only an input-token price is
/// applicable; a completion/output price would require inventing a completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pricing {
    pub input_per_million: f64,
    pub currency: String,
}

/// Complete configuration for one deterministic compression run.
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub strategy: Strategy,
    pub input_format: InputFormat,
    pub preserve_patterns: Vec<String>,
    pub tokenizer: TokenizerKind,
    pub pricing: Option<Pricing>,
    pub collapse_repetitions: bool,
    pub collapse_passing_tests: bool,
    pub collapse_progress_updates: bool,
    pub structural_json: bool,
    pub max_input_bytes: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            strategy: Strategy::Balanced,
            input_format: InputFormat::Auto,
            preserve_patterns: Vec::new(),
            tokenizer: TokenizerKind::O200kBase,
            pricing: None,
            collapse_repetitions: true,
            collapse_passing_tests: true,
            collapse_progress_updates: true,
            structural_json: true,
            max_input_bytes: 1_000_000,
        }
    }
}

impl CompressionConfig {
    /// Validate user-controlled configuration before any expensive work is done.
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=64 * 1024 * 1024).contains(&self.max_input_bytes) {
            return Err(Error::InvalidConfig(
                "max_input_bytes must be between 1 byte and 64 MiB".into(),
            ));
        }
        if self.preserve_patterns.len() > 100 {
            return Err(Error::InvalidConfig(
                "at most 100 preserve patterns are supported".into(),
            ));
        }
        for pattern in &self.preserve_patterns {
            if pattern.is_empty() {
                return Err(Error::InvalidConfig(
                    "preserve patterns cannot be empty".into(),
                ));
            }
            if pattern.len() > 256 {
                return Err(Error::InvalidConfig(
                    "each preserve pattern must be at most 256 bytes".into(),
                ));
            }
            if pattern.chars().any(|c| c == '\n' || c == '\r' || c == '\0') {
                return Err(Error::InvalidConfig(
                    "preserve patterns cannot contain line breaks or NUL bytes".into(),
                ));
            }
        }
        if let Some(pricing) = &self.pricing {
            if !pricing.input_per_million.is_finite()
                || pricing.input_per_million < 0.0
                || pricing.input_per_million > 1_000_000.0
            {
                return Err(Error::InvalidConfig(
                    "input token price must be a finite number between 0 and 1,000,000".into(),
                ));
            }
            let currency = &pricing.currency;
            if !(3..=8).contains(&currency.len())
                || !currency.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(Error::InvalidConfig(
                    "currency must be a 3-8 character ASCII code such as USD or EUR".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub kind: InputKind,
    pub automatic: bool,
    /// Confidence in the automatic classification. Manual selections are 1.0.
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenMeasurementKind {
    Exact,
    Estimated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMeasurement {
    pub kind: TokenMeasurementKind,
    pub name: String,
    pub label: String,
    pub method: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub currency: String,
    pub input_price_per_million: f64,
    pub original_input_cost: f64,
    pub compressed_input_cost: f64,
    pub input_cost_saved: f64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub original_bytes: usize,
    pub output_bytes: usize,
    pub bytes_saved: i64,
    pub byte_reduction_percent: f64,
    pub original_chars: usize,
    pub output_chars: usize,
    pub chars_saved: i64,
    pub original_tokens: usize,
    pub output_tokens: usize,
    pub tokens_saved: i64,
    pub token_reduction_percent: f64,
    pub token_measurement: TokenMeasurement,
    pub processing_time_ms: f64,
    pub cost: Option<CostEstimate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Omissions {
    pub repeated_line_runs: usize,
    pub repeated_lines_omitted: usize,
    pub passing_test_lines_omitted: usize,
    pub progress_lines_omitted: usize,
    pub normalized_log_lines_omitted: usize,
    pub json_duplicate_items_omitted: usize,
    pub markers_inserted: usize,
    pub total_input_lines: usize,
    pub total_output_lines: usize,
}

impl Omissions {
    pub fn total_omitted(&self) -> usize {
        self.repeated_lines_omitted
            + self.passing_test_lines_omitted
            + self.progress_lines_omitted
            + self.normalized_log_lines_omitted
            + self.json_duplicate_items_omitted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservationCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreservationReport {
    pub passed: bool,
    pub protected_occurrences: usize,
    pub retained_occurrences: usize,
    pub checks: Vec<PreservationCheck>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub output: String,
    pub strategy: Strategy,
    pub classification: Classification,
    pub metrics: Metrics,
    pub omissions: Omissions,
    pub preservation: PreservationReport,
    pub warnings: Vec<String>,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub transformation_applied: bool,
}

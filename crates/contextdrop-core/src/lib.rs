#![forbid(unsafe_code)]
//! ContextDrop's deterministic, local-first compression engine.
//!
//! The engine accepts UTF-8 text, classifies nothing by magic, and never reads
//! files, executes commands, or performs network I/O. Callers are responsible
//! for safe input acquisition and size limits.

mod benchmark;
mod classify;
mod compress;
mod error;
mod json;
mod protect;
mod text;
mod tokenizer;
mod types;
mod validate;

pub use benchmark::{
    run_benchmark, BenchmarkCase, BenchmarkCaseInfo, BenchmarkRun, BenchmarkSource,
    BenchmarkStrategyResult, CriticalCheck, FixtureManifest, FixtureSpec,
};
pub use compress::compress;
pub use error::{Error, Result};
pub use protect::{
    inspect_protection, protection_rule_catalog, ProtectedLine, ProtectionInspection,
    ProtectionRuleInfo,
};
pub use types::{
    Classification, CompressionConfig, CompressionResult, CostEstimate, InputFormat, InputKind,
    Metrics, Omissions, PreservationCheck, PreservationReport, Pricing, Strategy, TokenMeasurement,
    TokenMeasurementKind, TokenizerKind,
};

/// Semantic version of the compression engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum body size accepted by higher-level adapters when no tighter limit is
/// configured. The core itself uses `CompressionConfig::max_input_bytes`.
pub const RECOMMENDED_MAX_INPUT_BYTES: usize = 1_000_000;

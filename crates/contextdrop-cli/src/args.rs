use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use contextdrop_core::{CompressionConfig, InputFormat, Pricing, Strategy, TokenizerKind};

const EXAMPLES: &str = "\
Examples:
  contextdrop compress build.log
  Get-Content build.log | contextdrop compress --stdin
  contextdrop compress build.log --strategy conservative --format json
  contextdrop compress build.log --preserve CUSTOMER-ID
  contextdrop compare test-output.log
  contextdrop benchmark fixtures/build/rust-compiler.log

Exit codes:
  0  success
  1  input, output, or configuration error
  2  command-line usage error
  3  benchmark evidence assertion failed
  4  input exceeded the configured size limit
  5  invalid structured input or engine error
";

#[derive(Debug, Parser)]
#[command(
    name = "contextdrop",
    version,
    about = "Less noise. Better context. Deterministic local context compression.",
    long_about = "ContextDrop reduces repetitive developer-tool output locally and deterministically while preserving diagnostic evidence according to documented rules. It never runs commands, sends text over the network, or calls an AI model.",
    arg_required_else_help = true,
    subcommand_required = true,
    after_help = EXAMPLES
)]
pub struct Cli {
    /// Suppress human-readable diagnostics on standard error.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compress a file or standard input and write the processed text to stdout.
    Compress(CompressArgs),
    /// Show a real unified diff between original and processed content.
    Compare(CompareArgs),
    /// Run all three strategies against one or more files or fixtures.
    Benchmark(BenchmarkArgs),
}

#[derive(Debug, Args)]
#[command(after_help = EXAMPLES)]
pub struct CompressArgs {
    #[command(flatten)]
    pub input: InputSelection,
    #[command(flatten)]
    pub engine: EngineArgs,

    /// Compression strategy.
    #[arg(long, value_enum, default_value_t = StrategyArg::Balanced)]
    pub strategy: StrategyArg,

    /// Output format: processed text (default) or a machine-readable JSON result.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Write output to a new file instead of stdout. Existing files require --force.
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Explicitly allow overwriting an existing output file, including the input file.
    #[arg(long, requires = "output")]
    pub force: bool,
}

#[derive(Debug, Args)]
#[command(after_help = EXAMPLES)]
pub struct CompareArgs {
    #[command(flatten)]
    pub input: InputSelection,
    #[command(flatten)]
    pub engine: EngineArgs,

    /// Compression strategy.
    #[arg(long, value_enum, default_value_t = StrategyArg::Balanced)]
    pub strategy: StrategyArg,

    /// Output format: human-readable unified diff or machine-readable JSON.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Number of unchanged context lines shown around each diff hunk.
    #[arg(long, default_value_t = 3)]
    pub context: usize,
}

#[derive(Debug, Args)]
#[command(after_help = EXAMPLES)]
pub struct BenchmarkArgs {
    /// Files or directories to benchmark. Directories are read non-recursively.
    #[arg(value_name = "PATH", required = true)]
    pub paths: Vec<PathBuf>,

    /// Fixture manifest with categories and required-evidence assertions.
    #[arg(long, value_name = "FILE")]
    pub manifest: Option<PathBuf>,

    /// Output format: a human-readable table or machine-readable JSON.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    #[command(flatten)]
    pub engine: EngineArgs,
}

#[derive(Debug, Args)]
pub struct InputSelection {
    /// Input file. Use --stdin to read from a pipe instead.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Read input from standard input.
    #[arg(long, conflicts_with = "file")]
    pub stdin: bool,
}

#[derive(Debug, Args, Clone)]
pub struct EngineArgs {
    /// Input format. Automatic classification is conservative.
    #[arg(long, value_enum, default_value_t = InputFormatArg::Auto)]
    pub input_format: InputFormatArg,

    /// Tokenizer family used for token measurements.
    #[arg(long, value_enum, default_value_t = TokenizerArg::O200kBase)]
    pub tokenizer: TokenizerArg,

    /// Additional literal, case-insensitive text to preserve. Repeatable.
    #[arg(long = "preserve", value_name = "TEXT", action = clap::ArgAction::Append)]
    pub preserve_patterns: Vec<String>,

    /// Maximum accepted input size in bytes (1..=67,108,864).
    #[arg(long, default_value_t = 1_000_000, value_parser = parse_max_input)]
    pub max_input_bytes: usize,

    /// Optional input-token price per million tokens. Cost output is an estimate.
    #[arg(long, value_name = "PRICE")]
    pub price_input_per_million: Option<f64>,

    /// Currency code for the optional price estimate (e.g. USD, EUR).
    #[arg(long, default_value = "USD")]
    pub currency: String,

    /// Disable repeated-line and duplicate-line collapsing.
    #[arg(long)]
    pub no_collapse_repetitions: bool,

    /// Retain every passing test line.
    #[arg(long)]
    pub keep_passing_tests: bool,

    /// Retain every progress update.
    #[arg(long)]
    pub keep_progress_updates: bool,

    /// Disable structural deduplication of large JSON record arrays.
    #[arg(long)]
    pub disable_structural_json: bool,
}

impl EngineArgs {
    pub fn to_config(&self, strategy: Strategy) -> CompressionConfig {
        CompressionConfig {
            strategy,
            input_format: self.input_format.into(),
            preserve_patterns: self.preserve_patterns.clone(),
            tokenizer: self.tokenizer.into(),
            pricing: self.price_input_per_million.map(|price| Pricing {
                input_per_million: price,
                currency: self.currency.clone(),
            }),
            collapse_repetitions: !self.no_collapse_repetitions,
            collapse_passing_tests: !self.keep_passing_tests,
            collapse_progress_updates: !self.keep_progress_updates,
            structural_json: !self.disable_structural_json,
            max_input_bytes: self.max_input_bytes,
        }
    }

    pub fn tokenizer_name(&self) -> &'static str {
        match self.tokenizer {
            TokenizerArg::O200kBase => "o200k_base",
            TokenizerArg::Cl100kBase => "cl100k_base",
            TokenizerArg::Estimate => "estimate",
        }
    }
}

fn parse_max_input(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "max input size must be a non-negative integer".to_string())?;
    if !(1..=67_108_864).contains(&parsed) {
        return Err("max input size must be between 1 and 67,108,864 bytes".into());
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StrategyArg {
    Conservative,
    Balanced,
    Aggressive,
}

impl From<StrategyArg> for Strategy {
    fn from(value: StrategyArg) -> Self {
        match value {
            StrategyArg::Conservative => Strategy::Conservative,
            StrategyArg::Balanced => Strategy::Balanced,
            StrategyArg::Aggressive => Strategy::Aggressive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InputFormatArg {
    Auto,
    #[value(name = "plain-text")]
    PlainText,
    Terminal,
    #[value(name = "build-log")]
    BuildLog,
    #[value(name = "test-output")]
    TestOutput,
    Json,
    #[value(name = "structured-log")]
    StructuredLog,
}

impl From<InputFormatArg> for InputFormat {
    fn from(value: InputFormatArg) -> Self {
        match value {
            InputFormatArg::Auto => InputFormat::Auto,
            InputFormatArg::PlainText => InputFormat::PlainText,
            InputFormatArg::Terminal => InputFormat::Terminal,
            InputFormatArg::BuildLog => InputFormat::BuildLog,
            InputFormatArg::TestOutput => InputFormat::TestOutput,
            InputFormatArg::Json => InputFormat::Json,
            InputFormatArg::StructuredLog => InputFormat::StructuredLog,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TokenizerArg {
    #[value(name = "o200k_base")]
    O200kBase,
    #[value(name = "cl100k_base")]
    Cl100kBase,
    Estimate,
}

impl From<TokenizerArg> for TokenizerKind {
    fn from(value: TokenizerArg) -> Self {
        match value {
            TokenizerArg::O200kBase => TokenizerKind::O200kBase,
            TokenizerArg::Cl100kBase => TokenizerKind::Cl100kBase,
            TokenizerArg::Estimate => TokenizerKind::Estimate,
        }
    }
}

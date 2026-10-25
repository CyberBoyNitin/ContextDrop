#![forbid(unsafe_code)]
//! Command-line interface for ContextDrop.

mod args;
mod benchmark;
mod error;
mod io_utils;
mod render;

use std::process;

use args::{Cli, Command, CompareArgs, CompressArgs, OutputFormat};
use clap::Parser;
use contextdrop_core::{compress, CompressionResult};
use serde::Serialize;
use similar::TextDiff;

use crate::error::CliError;

fn main() {
    let cli = Cli::parse();
    if let Err(error) = execute(cli) {
        eprintln!("contextdrop: error: {error}");
        process::exit(error.exit_code());
    }
}

fn execute(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Compress(arguments) => run_compress(arguments, cli.quiet),
        Command::Compare(arguments) => run_compare(arguments),
        Command::Benchmark(arguments) => run_benchmark(arguments),
    }
}

fn run_compress(arguments: CompressArgs, quiet: bool) -> Result<(), CliError> {
    let input = io_utils::read_input(&arguments.input, arguments.engine.max_input_bytes)?;
    let config = arguments.engine.to_config(arguments.strategy.into());
    let result = compress(&input.text, &config)?;

    match arguments.format {
        OutputFormat::Text => {
            if let Some(path) = &arguments.output {
                io_utils::ensure_output_safe(
                    path,
                    arguments.input.file.as_deref(),
                    arguments.force,
                )?;
                io_utils::write_file(path, result.output.as_bytes())?;
            } else {
                io_utils::write_stdout(result.output.as_bytes())?;
            }
            render::print_compress_diagnostics(&result, quiet);
        }
        OutputFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(&result)?;
            bytes.push(b'\n');
            if let Some(path) = &arguments.output {
                io_utils::ensure_output_safe(
                    path,
                    arguments.input.file.as_deref(),
                    arguments.force,
                )?;
                io_utils::write_file(path, &bytes)?;
            } else {
                io_utils::write_stdout(&bytes)?;
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct CompareOutput<'a> {
    report_version: u32,
    engine_version: &'static str,
    original: &'a str,
    unified_diff: String,
    compression: &'a CompressionResult,
}

fn run_compare(arguments: CompareArgs) -> Result<(), CliError> {
    if arguments.context > 100 {
        return Err(CliError::Invocation(
            "--context must be between 0 and 100".into(),
        ));
    }
    let input = io_utils::read_input(&arguments.input, arguments.engine.max_input_bytes)?;
    let config = arguments.engine.to_config(arguments.strategy.into());
    let result = compress(&input.text, &config)?;

    match arguments.format {
        OutputFormat::Text => {
            let rendered = render::render_comparison_with_original(
                &input.label,
                &input.text,
                &result,
                arguments.context,
            );
            io_utils::write_stdout(rendered.as_bytes())?;
        }
        OutputFormat::Json => {
            let unified_diff = if input.text == result.output {
                String::new()
            } else {
                TextDiff::from_lines(&input.text, &result.output)
                    .unified_diff()
                    .context_radius(arguments.context)
                    .header("original", "contextdrop")
                    .to_string()
            };
            let report = CompareOutput {
                report_version: 1,
                engine_version: contextdrop_core::VERSION,
                original: &input.text,
                unified_diff,
                compression: &result,
            };
            let mut bytes = serde_json::to_vec_pretty(&report)?;
            bytes.push(b'\n');
            io_utils::write_stdout(&bytes)?;
        }
    }
    Ok(())
}

fn run_benchmark(arguments: args::BenchmarkArgs) -> Result<(), CliError> {
    let report = benchmark::execute_benchmark(
        &arguments.paths,
        arguments.manifest.as_deref(),
        &arguments.engine,
    )?;

    match arguments.format {
        OutputFormat::Text => {
            let rendered = render::render_benchmark(&report);
            io_utils::write_stdout(rendered.as_bytes())?;
        }
        OutputFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(&report)?;
            bytes.push(b'\n');
            io_utils::write_stdout(&bytes)?;
        }
    }

    if !report.all_checks_passed() {
        return Err(CliError::BenchmarkEvidenceFailed);
    }
    Ok(())
}

#![forbid(unsafe_code)]
//! Local stdio Model Context Protocol server for ContextDrop.

use std::io::{self, BufReader, BufWriter};
use std::process;

fn main() {
    let mut arguments = std::env::args().skip(1);
    if let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                print_help();
                return;
            }
            "-V" | "--version" => {
                println!("contextdrop-mcp {}", contextdrop_core::VERSION);
                return;
            }
            other => {
                eprintln!("contextdrop-mcp: unknown argument: {other}");
                eprintln!("Run contextdrop-mcp --help for usage.");
                process::exit(2);
            }
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(error) =
        contextdrop_mcp::serve(BufReader::new(stdin.lock()), BufWriter::new(stdout.lock()))
    {
        eprintln!("contextdrop-mcp: I/O error: {error}");
        process::exit(1);
    }
}

fn print_help() {
    println!(
        "\
contextdrop-mcp {version}

Local stdio Model Context Protocol server for ContextDrop.

USAGE:
    contextdrop-mcp

The process speaks newline-delimited JSON-RPC on stdin/stdout. It is intended
to be launched by an MCP client, not used interactively. It performs no network
I/O, writes no files, and sends diagnostics only to stderr.

TOOLS:
    compress_context         Compress pasted UTF-8 text
    compress_context_file    Read and compress a local UTF-8 file
    inspect_protection       Inspect protected diagnostic lines
    list_protection_rules    List built-in protection rules

PROTOCOL:
    2026-07-28 (modern) and 2025-11-25 / 2025-06-18 / 2025-03-26 /
    2024-11-05 (legacy initialize).
",
        version = contextdrop_core::VERSION
    );
}

# Architecture

ContextDrop is a local-first deterministic context compressor. This document
describes the shipped architecture at version 0.1.0, the data flow through each
surface, and the boundaries that keep processing local and inspectable.

## System overview

```text
             ┌──────────────────────────────────────────────────────┐
             │                 Browser (optional)                   │
             │  React 19 · Vite · Tailwind 4 · React Router         │
             │  Workspace · Benchmark Lab · Rules · Settings        │
             └───────────────┬──────────────────────────────────────┘
                             │ same-origin /api requests
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│ contextdrop-server (Axum, loopback only)                            │
│  /api/health   /api/rules     /api/fixtures/:id                     │
│  /api/compress /api/benchmark /api/inspect                          │
│  static dashboard · strict Host/Origin checks · body limits         │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
             ┌─────────────────┴─────────────────┐
             │                                   │
             ▼                                   ▼
┌──────────────────────────────┐   ┌──────────────────────────────────┐
│ contextdrop CLI (clap)       │   │ contextdrop-mcp (stdio)          │
│ compress · compare           │   │ JSON-RPC · MCP tools · file read │
│ files/stdin/stdout · JSON    │   │ no sockets · no writes · no exec │
└──────────────┬───────────────┘   └───────────────┬──────────────────┘
               │                                   │ MCP stdio
               │                                   ▼
               │                         ┌────────────────────────────┐
               │                         │ Agent / MCP client         │
               │                         │ OpenCode · Claude · Cursor │
               │                         └────────────────────────────┘
               │ pure Rust calls
               ▼
┌─────────────────────────────────────────────────────────────────────┐
│ contextdrop-core (pure library, no I/O)                             │
│  classify → protect → transform → validate → fallback → measure     │
│  text · JSON · structured logs · tokenizer · benchmark runner       │
└─────────────────────────────────────────────────────────────────────┘
```

The core library never performs file I/O, launches a process, opens a socket, or
calls a model. The CLI owns its explicit file reads and writes; the MCP server
owns its explicit input-file reads; the server owns HTTP access. This separation
makes the engine straightforward to test and audit.

## Repository map

```text
crates/
  contextdrop-core/      Pure compression engine and benchmark primitives
  contextdrop-cli/       Command-line interface and integration tests
  contextdrop-server/    Loopback Axum API, embedded fixtures, static host
  contextdrop-mcp/       Local stdio MCP server and protocol tests
web/
  src/                   React dashboard
  public/                Static assets such as the favicon
  dist/                  Production build (generated; not source)
fixtures/                Twelve deterministic benchmark fixtures + manifest
scripts/                 Fixture generator
docs/                    Architecture, privacy, security, methodology, MCP, comparison
```

## Compression pipeline

The public entry point is `contextdrop_core::compress(input, config)`. It runs
in a fixed order:

1. **Validate configuration and input limit.**
   Max input size, tokenizer choice, pricing, and preserve patterns are checked
   before any transformation.
2. **Classify the input.**
   `classify` inspects the strongest available signals and returns a kind plus
   confidence. Forced formats bypass automatic detection. Automatic detection
   is intentionally conservative: ambiguous JSON-like content falls back to
   plain-text handling.
3. **Build the protection map.**
   Built-in rules and literal user patterns mark protected lines. The protector
   also marks adjacent context lines as ineligible so an error keeps its source
   location, caret, or command context. Protection is computed on the original
   input and applies across all strategies.
4. **Apply exactly one strategy.**
   - Conservative: collapse consecutive exact repeated lines.
   - Balanced: Conservative plus contiguous passing-test runs, progress
     updates, and duplicate records in qualifying JSON arrays.
   - Aggressive: Balanced plus global exact duplicate lines and normalized
     structured-log groups.
   JSON inputs use lexical minification at every strategy; structural array
   deduplication is a Balanced+ operation controlled by the `structural_json`
   setting. JSON changes insert the `$contextdrop` marker and produce an explicit
   schema-change warning.
5. **Validate the candidate.**
   Text output must retain the multiset of protected lines. JSON output must
   parse and retain rule-based protected evidence. Invalid candidates are
   rejected.
6. **Fallback when necessary.**
   If validation fails, if no transformation qualifies, or if the candidate is
   not smaller in bytes, the original input is returned. Fallback is explicit in
   the result (`fallback_used`, `fallback_reason`, `transformation_applied`).
7. **Measure.**
   Byte and character counts, exact or estimated token counts, per-kind omission
   counts, preservation checks, warnings, classification, strategy, and timing
   are returned together. Timing covers transformation only; tokenizer
   initialization is excluded.

Protection always wins over reduction. A strategy never removes a protected line
or its adjacent context. A successful presence check is evidence that documented
rules held; it is not a proof of semantic equivalence.

## Input classification

Supported kinds are `plain-text`, `terminal`, `build-log`, `test-output`, `json`,
and `structured-log`. JSON detection validates the complete document before
claiming JSON handling. JSON-lines data is treated as structured logs. Automatic
classification is exposed in every result and rendered in the dashboard.
Manual format selection is available for deterministic overrides.

## Preservation model

Built-in rules cover:

- failure, fatal, warning, security, and vulnerability markers;
- stack traces, tracebacks, panics, goroutine dumps, cause chains, and carets;
- paths with line numbers, including Python `File "…", line …` forms;
- exit codes and process status;
- failed test markers, assertion detail, expected/received blocks;
- test and build summaries;
- HTTP 4xx/5xx evidence;
- commands, shell prompts, and build/test command context.

User preserve patterns are literal, case-insensitive substrings with a limit of
100 patterns and 256 characters each. They are never compiled as regular
expressions, evaluated as shell syntax, or executed. The public
`protection_rule_catalog()` exposes stable IDs and labels without exposing the
regexes.

## CLI design

`contextdrop` has three subcommands:

- `compress`: read one file or stdin, process, and write stdout or an explicitly
  requested output file. Existing output files are refused unless `--force` is
  supplied.
- `compare`: emit a unified diff plus compression metrics.
- `benchmark`: run all strategies over files or the fixture manifest and evaluate
  evidence assertions.

Inputs are never executed. Inputs are decoded as strict UTF-8. Size limits are
enforced while reading streams. Exit codes are stable and documented in CLI help.

## MCP boundary

`contextdrop-mcp` is a separate binary that speaks newline-delimited JSON-RPC
over stdin/stdout. It advertises only the `tools` capability and exposes four
read-only tools: compress text, read-and-compress one explicit file, inspect
protection, and list protection rules. It has no command-execution, write,
network, resource, or prompt tools.

The MCP layer owns only transport framing, argument validation, and the explicit
`compress_context_file` read. Compression is delegated in-process to
`contextdrop-core`. File reads:

- require an explicit path from the caller;
- resolve relative paths against the server process's working directory;
- reject non-regular files and non-UTF-8 content;
- enforce the per-call input limit before and during the read;
- never write, rename, or delete anything.

The server supports the legacy `initialize` handshake for MCP `2025-11-25`
through `2024-11-05` and the per-request metadata model for MCP `2026-07-28`.
Protocol behavior is stateless; tool calls do not depend on earlier calls.
stdout carries JSON-RPC messages only. Diagnostics, if any, go to stderr. The
server does not log request or file content.

The MCP layer inherits the filesystem permissions of the user running the
client. It is not a sandbox and does not enforce a path allowlist; the caller
must only pass paths it intends the agent to read. See [MCP integration](MCP.md).

## Server boundary

The server binds only to a loopback address and rejects non-loopback bind
addresses at startup. API requests must:

- use a loopback `Host` header;
- if an `Origin` header is present, use a loopback origin;
- for POST, use `Content-Type: application/json`;
- stay within a 4 MiB HTTP body and a 1,000,000-byte input limit;
- pass a four-request concurrency limit and a 20-second processing timeout.

CORS is not a wildcard. The server reflects only validated loopback origins and
does not enable credentials. No request content is logged. Errors are sanitized
into JSON envelopes with stable codes.

The dashboard is a separately built Vite SPA. The server registers `/`,
`/workspace`, `/benchmark`, `/rules`, and `/settings` explicitly so direct
navigation works without turning missing assets into HTML responses. API routes
are nested under `/api`; unknown API routes return JSON 404 responses.

## Dashboard

The dashboard uses React Router and a typed API client. Workspace and Benchmark
Lab call the real local API; Rules calls the real protection inspector; Settings
stores only local preferences. There is no fixture fallback, generated metrics,
or synthetic “success” state. Loading, empty, error, fallback, and warning states
are rendered from actual responses. The layout is responsive, uses a bottom
navigation on small screens, respects `prefers-reduced-motion`, and keeps focus
visible.

## Determinism and reproducibility

There is no randomness, time-dependent behavior, model sampling, or concurrency
inside the core. Given the same input, configuration, and tokenizer, the output
and measurements are reproducible. Benchmark timing varies by machine and build
profile; it is reported separately and is not part of the transformation result.

The benchmark runner:

- accepts explicit cases or the repository manifest;
- runs Conservative, Balanced, and Aggressive against the same original input;
- records byte, character, and token metrics;
- evaluates a manifest `must_preserve` list with exact substring checks;
- provides critical checks per strategy.

Fixture evidence checks are intentionally conservative presence checks. They do
not assert semantic equivalence, model-answer quality, or generalization beyond
the fixture corpus.

## Dependency direction

`contextdrop-core` has no dependency on CLI, server, MCP, or web code.
`contextdrop-cli`, `contextdrop-server`, and `contextdrop-mcp` all depend on core.
The web application depends only on the HTTP contract. This direction keeps the
engine portable and prevents UI or transport concerns from leaking into the
compressor.

## Related documents

- [Benchmark methodology](BENCHMARK_METHODOLOGY.md)
- [MCP integration](MCP.md)
- [Threat model](THREAT_MODEL.md)
- [Privacy](PRIVACY.md)
- [Competitive analysis](COMPETITIVE_ANALYSIS.md)
- [Security policy](../SECURITY.md)

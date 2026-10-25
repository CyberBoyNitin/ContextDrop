# Changelog

All notable changes to ContextDrop are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses semantic versioning after 1.0.

## [0.1.0] - 2026-10-24

First public pre-1.0 release.

### Added

- Pure Rust `contextdrop-core` engine with Conservative, Balanced, and Aggressive strategies.
- Automatic and manual classification for plain text, terminal output, build logs, test output, JSON, and structured logs.
- Heuristic protection for errors, failures, security findings, assertions, stack frames, paths with line numbers, exit codes, commands, and summaries, with adjacent-context retention.
- Literal, case-insensitive custom preserve patterns with explicit validation.
- Text duplicate collapsing, passing-test and progress reduction, JSON lexical minification, structural JSON record deduplication, global duplicate removal, and normalized structured-log grouping.
- Post-transformation preservation validation and fallback to the original input on unsafe or non-reducing candidates.
- Exact local token counting with `tiktoken-rs` (`o200k_base` and `cl100k_base`) plus a labelled heuristic estimate.
- Byte, character, token, omission, cost-estimate, classification, warning, and processing-time metrics.
- `contextdrop` CLI with `compress`, `compare`, and `benchmark`, files/stdin, text/JSON output, explicit output overwrite protection, size limits, JSON evidence assertions, and stable exit codes.
- Local stdio MCP server (`contextdrop-mcp`) with `compress_context`, `compress_context_file`, `inspect_protection`, and `list_protection_rules`; dual-era MCP support (`2026-07-28` plus legacy initialize), explicit-path file reads, input limits, and no command, write, or network tools.
- Loopback-only Axum server with strict Host/Origin checks, allowlisted CORS, JSON-only POST endpoints, request/input limits, concurrency limits, work timeouts, security headers, embedded fixtures, and SPA route handling.
- React, TypeScript, Vite, and Tailwind dashboard with Workspace, Benchmark Lab, Rules, and Settings pages backed by the real local API.
- Twelve deterministic synthetic fixtures covering build/test failures, JSON duplication, security findings, structured logs, Unicode, dependency noise, panic backtraces, and a dense-source control.
- Unit, property-based, CLI integration, server, MCP protocol, and fixture-evidence tests.
- Benchmark methodology with exact token and byte results for the included corpus.
- Apache-2.0 license, security policy, contributor guide, code of conduct, threat model, privacy documentation, architecture documentation, competitive analysis, issue forms, pull-request template, Dependabot configuration, and CI.

### Verified in the 0.1.0 baseline

- 95 Rust tests passing (45 core unit, 3 property, 3 CLI unit, 10 CLI integration, 13 server, 17 MCP unit, 4 MCP stdio integration).
- `cargo fmt` and Clippy with warnings denied pass.
- Dashboard TypeScript check and production build pass.
- 168/168 required evidence checks pass across the 12-fixture, 3-strategy benchmark.
- Aggregate benchmark: Conservative 19.11% byte reduction, Balanced 58.86%, Aggressive 86.14% on the synthetic corpus.
- `cargo audit` reports no vulnerabilities across 137 locked Rust dependencies.
- `npm audit` reports no vulnerabilities across the npm tree.

### Known limitations

- Protection is heuristic and does not prove semantic equivalence.
- JSON structural deduplication adds a `$contextdrop` marker and changes document shape.
- Aggressive mode can be larger than Balanced on some inputs because of omission-marker overhead.
- The benchmark corpus is synthetic and does not establish savings for arbitrary workloads.
- No authentication is provided for the local API; it must remain loopback-only.
- The MCP file tool has no sandbox or path allowlist and must be used with a trusted client and explicit paths; it is not a transparent agent proxy.
- MCP metrics describe the input and compressed text only; the structured report adds tool-result overhead and is not included in reported savings.

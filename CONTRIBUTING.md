# Contributing to ContextDrop

Thanks for helping improve ContextDrop. This project favors small, inspectable changes that preserve its central contract: deterministic local processing, evidence-first compression, and no hidden data flows.

## Before you start

- Read the [architecture](docs/ARCHITECTURE.md), [threat model](docs/THREAT_MODEL.md), and [privacy](docs/PRIVACY.md) documents.
- For a security vulnerability, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.
- For large features or rule changes, open an issue first and describe the evidence and failure modes.
- By participating, you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

Requirements:

- Rust stable (`rust-toolchain.toml` selects stable; minimum supported Rust is 1.85)
- Node.js 20+ and npm (tested with Node 24)
- A platform supported by Rust and Axum

From the repository root:

```bash
cargo build --workspace

cd web
npm ci
npm run build
cd ..
```

Run the local dashboard:

```bash
cargo run -p contextdrop-server -- --bind 127.0.0.1:8787 --web-dir web/dist
```

Open <http://127.0.0.1:8787>. Never bind the development server beyond loopback.

## Required checks

Run these before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd web && npm run typecheck && npm run build
```

For dependency changes or releases, also run:

```bash
cargo audit
cd web && npm audit
```

Windows note: some integration tests spawn the built binary rather than `cargo run` because `cargo run` can obscure nonzero program exit codes.

## Project rules

### Core purity

`contextdrop-core` must not perform file I/O, network access, subprocess execution, environment-dependent behavior, or model calls. The CLI, MCP, and server layers own those concerns. Do not introduce `unsafe`; it is forbidden workspace-wide.

### Determinism

Given the same input, configuration, and tokenizer, processing must return the same output and measurements. Do not use randomness, wall-clock-dependent decisions, unordered map iteration that affects output, or background state in the transformation pipeline. Timing metrics may vary and must stay outside decision logic.

### Evidence before reduction

- New omission rules must include a protection analysis and tests for nearby evidence.
- A transformation must not remove protected lines or their documented adjacent context.
- If validation fails or output is not smaller, keep the existing fallback behavior.
- Do not claim semantic equivalence, universal savings, or exact provider billing.
- User preserve patterns must remain literal, non-regex, non-executed strings.

### Honest results

- Fixtures must be synthetic or appropriately licensed, deterministic, and checked in.
- Update `fixtures/manifest.json` when fixture content or required evidence changes.
- Regenerate the fixtures with `node scripts/generate-fixtures.mjs` if you change the generator.
- If compression behavior changes, rerun the benchmark and update [docs/BENCHMARK_METHODOLOGY.md](docs/BENCHMARK_METHODOLOGY.md) with new actual values.
- Do not add fake dashboard data, dead controls, or prefilled success states. Loading, error, empty, fallback, and warning states must reflect the real API.

### Privacy and security

- No telemetry, analytics, remote fonts, remote scripts, or tokenizer network calls.
- Do not log request content on the server, MCP process, or dashboard, or persist pasted input.
- Maintain loopback-only binding, Host/Origin validation, JSON content-type enforcement, and size limits.
- Do not loosen CORS or add credentials without a threat-model update and a compelling reason.
- Do not add arbitrary filesystem access to the HTTP API.
- The MCP server must remain stdio-only, must not add execution or write tools, and must keep file reads to the explicit path and input limit.
- Add tests for security-relevant behavior changes.

### API compatibility

The API contract is mirrored in `web/src/types.ts`. When changing a request, response, error code, or default:

1. Update the Rust types and API handlers.
2. Update `web/src/types.ts`.
3. Update all affected pages and states.
4. Add or update server tests.
5. Update documentation and, if user-facing, the changelog.

Pre-1.0 changes are allowed, but silent breakage is not.

## Tests

Use the right level:

- **Unit tests** for rule behavior, transforms, validation, and renderers.
- **Property tests** for arbitrary Unicode, valid JSON, and panic resistance.
- **CLI integration tests** for exit codes, stdin/files, overwrite protection, and output formats.
- **Server tests** for API guards, limits, CORS/origin checks, SPA routing, and real engine responses.
- **MCP tests** for legacy/modern handshakes, tool schemas, stdio purity, text and file tools, limits, and error behavior.
- **Fixture regressions** for protected evidence and meaningful reductions.
- **Frontend checks** with `tsc` and a production build. Add a browser-level test when adding interactive behavior.

Tests must not depend on external services, network access, real customer data, or a particular machine's clock.

## Pull requests

Keep changes focused. A good pull request includes:

- the problem and why it matters;
- a concise description of the approach and any trade-offs;
- tests that fail before the change and pass after it;
- updated docs for behavior, API, security, privacy, or benchmark changes;
- actual command output for test and benchmark claims;
- screenshots for visible dashboard changes;
- no unrelated formatting churn.

Maintainers may ask for narrower scope, stronger preservation tests, or removal of unsupported claims. “It compresses more” is not sufficient without evidence and safety analysis.

## Commit messages

Clear, imperative summaries are preferred, for example:

```text
Add adjacent-context protection for Python tracebacks
Fix SPA fallback for missing static assets
Document exact token benchmark methodology
```

Squashing is acceptable. Do not commit generated `target/`, `node_modules/`, or `web/dist/` output.

## Documentation

Documentation is part of the product. When behavior changes:

- README: user-visible features, commands, and verified results.
- `docs/ARCHITECTURE.md`: data flow and boundaries.
- `docs/MCP.md`: MCP tools, client configuration, protocol support, or file-access behavior.
- `docs/THREAT_MODEL.md` / `docs/PRIVACY.md`: security or data-flow changes.
- `docs/BENCHMARK_METHODOLOGY.md`: measurement or result changes.
- `docs/COMPETITIVE_ANALYSIS.md`: positioning claims.
- `SECURITY.md`: reporting process or supported versions.

Do not claim tests, audits, benchmarks, or security properties that were not actually run.

## License

By contributing, you agree that your contribution is licensed under the Apache License 2.0, the same license as this project. See [LICENSE](LICENSE).

## Questions

Open a discussion or a non-sensitive issue. For conduct concerns, follow the reporting guidance in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

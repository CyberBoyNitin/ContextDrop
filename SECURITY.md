# Security policy

ContextDrop processes potentially sensitive logs and source excerpts. Security reports are taken seriously, especially issues involving data leaving the machine, unintended execution, unsafe file writes, cross-origin access to the local API, MCP file reads outside documented behavior, or compression that removes protected evidence.

## Supported versions

| Version | Supported |
| --- | --- |
| 0.1.x (current pre-1.0 line) | Yes, until a newer minor release supersedes it |
| Older pre-release builds | No |

Pre-1.0 releases may change APIs. Security fixes are made on the current release line unless a backport is explicitly announced.

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.**

Use the repository's **Security → Report a vulnerability** private advisory workflow. If that workflow is unavailable in your distribution channel, contact the maintainers through the private channel by which you received the build. Include:

- affected version or commit;
- operating system and build profile;
- whether the issue affects the CLI, core library, server, dashboard, or MCP server;
- a minimal reproduction that does not include real secrets or customer data;
- expected and actual behavior;
- impact assessment, if known.

Please avoid attaching production logs. Redact paths, tokens, customer identifiers, and proprietary code. A synthetic minimal case is more useful than a large real one.

### What to expect

- Initial acknowledgement target: **3 business days**.
- Triage and severity assessment target: **7 business days**.
- Coordinated disclosure target: **90 days** after acknowledgement, or sooner by agreement.
- Status updates when the issue is confirmed, fixed, or requires coordination.

These are targets for a volunteer-led project, not a commercial service-level agreement.

## In scope

- Core compression removing or corrupting protected diagnostic evidence contrary to the documented rules.
- Fallback or validation failures that return unsafe output.
- Cross-origin access, DNS rebinding, or authentication bypass affecting the loopback server.
- A way to make the API, CLI, or MCP server read, write, or execute arbitrary files or commands beyond documented behavior.
- An MCP transport or protocol bug that writes non-JSON-RPC data to stdout, opens a network listener, or exposes file content in diagnostics.
- Content logging, telemetry, or an outbound network request from ContextDrop code.
- XSS, HTML injection, or script execution in the dashboard.
- Denial of service reachable through the documented loopback API and its limits.
- Dependency vulnerabilities with a practical exploit path in ContextDrop.
- Build or release artifacts that differ from the reviewed source.

## Out of scope

- The user intentionally running `--bind 0.0.0.0`, a reverse proxy, or a tunnel and exposing the local API.
- Other processes or users with access to the same account, files, browser, or process memory.
- The user or agent intentionally passing a readable path to `compress_context_file`; that is documented file access, not a sandbox escape.
- Retention, telemetry, or model transmission performed by an MCP client or agent after it receives a tool result.
- Secrets already present in logs. ContextDrop is not a redactor or secret scanner.
- Semantic changes that are disclosed by an omission marker, or JSON schema changes from documented structural deduplication.
- Best-practice reports without a demonstrated impact (for example, missing HSTS on a loopback HTTP server).
- Vulnerabilities only in development dependencies with no path to a shipped artifact.
- Social engineering or physical access.

## Security design

ContextDrop's security boundary is described in detail in:

- [Threat model](docs/THREAT_MODEL.md)
- [Privacy](docs/PRIVACY.md)
- [Architecture](docs/ARCHITECTURE.md)

Key invariants:

- Core is pure and performs no file, network, process, or model I/O.
- The server refuses non-loopback bind addresses.
- API Host and Origin headers are validated; CORS is allowlisted.
- POST requests require JSON; request and input sizes are bounded.
- Server input is not logged or persisted.
- The MCP server uses stdio only, opens no sockets, writes no files, and reads only the explicit path passed to its file tool.
- MCP input is bounded per call and the process emits only JSON-RPC on stdout.
- Protected evidence is validated after transformation; failure returns the original.
- User preserve patterns are literal and never executed.
- Existing CLI output files are not overwritten without `--force`.
- No telemetry or remote content is loaded by the dashboard.

## Safe harbor

Good-faith security research that follows this policy, avoids privacy violations, avoids service disruption, and uses only data you are authorized to access will not be pursued by the project. Do not test against systems or data you do not own or have permission to use. Do not exfiltrate data, establish persistence, or degrade another user's system.

## Dependency and release checks

Before a release, maintainers are expected to run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo audit
cd web && npm ci && npm audit && npm run build
```

At version 0.1.0, `cargo audit` reported no advisories across 137 locked Rust dependencies and `npm audit` reported no vulnerabilities across the npm tree. These are point-in-time results, not permanent guarantees. See the CI workflow and release notes for current status.

## Disclosure credit

With permission, the project will credit reporters in the advisory and release notes. Let us know how you would like to be identified.

Last updated: 2026-10-24

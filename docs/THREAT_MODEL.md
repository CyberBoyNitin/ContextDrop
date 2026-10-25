# Threat model

This document describes the security boundary of ContextDrop 0.1.0, the assets it handles, the threats considered during design, and the residual risks that remain. It is intended for contributors, reviewers, and operators running the local dashboard.

## Security objectives

1. **Local processing.** Input text stays on the machine and is never sent to a remote service by ContextDrop.
2. **No unintended execution.** Logs, commands, paths, JSON, and preservation patterns are data. They are never evaluated as shell, JavaScript, regex replacement code, or subprocess arguments.
3. **Evidence preservation.** Protected diagnostic lines survive compression or the transformation is rejected and the original is returned.
4. **Bounded local API.** The optional HTTP surface accepts only loopback traffic and is resistant to accidental cross-origin use and resource exhaustion.
5. **Safe file handling.** The CLI reads only requested paths and refuses to overwrite an existing output unless explicitly forced. The MCP server reads only the explicit file path passed to its file tool and never writes.
6. **No hidden local transport.** The MCP server uses stdio, opens no socket, and does not make outbound requests.
7. **Supply-chain visibility.** Dependencies are locked and audited in CI.

ContextDrop is not a sandbox, secrets manager, access-control system, or multi-tenant service. It does not claim to make untrusted logs safe to publish.

## Assets

| Asset | Sensitivity | Where it lives |
| --- | --- | --- |
| Input logs, source excerpts, test output | Potentially high; may contain paths, tokens, or customer data | Process memory; browser memory |
| Compressed output | Same as input | Process memory; stdout/file/chosen output; MCP tool result |
| MCP file paths | Low to high; reveals filenames and directory structure | MCP request and tool metadata |
| Preserve patterns and preferences | Low, but can reveal project names | Browser localStorage |
| Results, checks, warnings | May reveal file names and failure details | Browser memory / JSON output / MCP structured content |
| Embedded benchmark fixtures | Public synthetic data | Binary / repository |
| Local API availability | Medium; CPU and memory on the host | Loopback socket |
| MCP process availability | Low; CPU and memory on the host | Client-spawned stdio process |

## Trust boundaries

```text
Logs/files/stdin  ── untrusted text ──>  CLI or loopback API
                                              │
                                              ▼
                                   contextdrop-core (pure Rust)
                                   validation, rules, no I/O

Browser origin  ── same-origin /api ──>  loopback Axum server
Local processes ── can reach loopback ─>  loopback Axum server (no auth)

Agent / MCP client ── stdio JSON-RPC ──>  contextdrop-mcp
                                              │ explicit path read
                                              ▼
                                   contextdrop-core (pure Rust)
```

- **Input content is untrusted.** It may be adversarial, enormous, malformed, or contain terminal escapes.
- **The local machine and user account are trusted.** Anyone with the user's file permissions can read the input anyway.
- **Other same-user processes are inside the local trust boundary.** They may be able to call the API. This is a deliberate consequence of not adding accounts.
- **The MCP client is trusted to choose paths.** The MCP server reads a path only when the client passes it to `compress_context_file`; it has no authentication or path allowlist.
- **Network peers are outside the boundary.** The server must never be reachable from them. The MCP server has no network listener.

## Threats and mitigations

### T1 — Input executes as code or shell

**Scenario:** A pasted log contains `rm -rf`, PowerShell fragments, escape sequences, or JavaScript. The user expects compression, but the tool executes something.

**Mitigations:**

- Core is a pure library with no process, shell, filesystem, or network APIs.
- The CLI never interprets input as arguments.
- Preservation patterns are literal case-insensitive substrings, not regular expressions, and are never executed.
- The dashboard renders output as text in React; it does not use `dangerouslySetInnerHTML` or evaluate content.
- JSON is parsed with `serde_json`, not evaluated.
- ContextDrop does not interpret ANSI escapes. The dashboard renders text rather than launching a terminal emulator.

**Residual risk:** Copying output into a shell yourself can still execute it. ContextDrop displays evidence; it does not make hostile commands safe. CLI output may retain ANSI or OSC control sequences from the input, and a terminal emulator may act on them; review untrusted logs in the dashboard or a plain-text viewer, not an interactive terminal.

### T2 — Cross-site request forgery or DNS rebinding against the local server

**Scenario:** A website visited by the user tries to POST to `http://127.0.0.1:8787/api/compress`, read local data, or use a non-loopback `Host` reached through rebinding.

**Mitigations:**

- The server rejects bind addresses that are not loopback.
- API middleware rejects non-loopback `Host` values.
- If an `Origin` header is present, it must be a loopback origin; those origins are the only ones reflected in CORS responses.
- CORS is not a wildcard and does not allow credentials.
- POST endpoints require `Content-Type: application/json`, which prevents simple HTML form submissions from being accepted.
- Browsers enforce the same-origin policy for reading responses. No authentication cookies exist to attach to malicious requests.
- `frame-ancestors 'none'` and `X-Frame-Options: DENY` prevent framing the local dashboard.
- Security headers include a restrictive Content Security Policy, `nosniff`, `no-referrer`, a restrictive permissions policy, and `same-origin` opener policy.

**Residual risk:** DNS rebinding behavior varies by browser. The explicit Host allowlist is the primary defense. A browser extension or local malware that can issue requests from inside the machine may bypass browser-origin assumptions. Loopback servers have no strong authentication; do not expose this one.

### T3 — The API is exposed beyond loopback

The server binds to loopback by default and validates the configured address at startup. A reverse proxy, SSH tunnel, container port publish, or firewall change could still expose it.

**Mitigations:**

- Non-loopback binds are rejected with an error.
- There are no authentication, TLS, or multi-user features, so accidental exposure fails closed operationally rather than pretending to be production-secure.
- Documentation requires loopback-only use.

**Residual risk:** If an operator deliberately proxies the loopback server, the proxy becomes the security boundary. That deployment is unsupported. Never run ContextDrop on a network interface controlled by others.

### T4 — Local data leaks through logs, telemetry, or persistence

**Scenario:** Sensitive log content is written to disk, sent to a vendor, or retained in analytics.

**Mitigations:**

- No telemetry or remote endpoints exist in the workspace.
- Server access logging is not enabled. Request bodies are never logged.
- The MCP server writes only JSON-RPC to stdout and does not log input, output, or file contents.
- Error responses contain stable codes and safe messages, not full input content.
- Input and output are held only for request duration and returned to the browser or MCP client.
- The dashboard stores preferences only, not input or result history.
- Tokenizer vocabularies are embedded locally, so exact token counting does not contact a remote tokenizer.

**Residual risk:** The user's operating system, swap file, crash dump, browser, terminal scrollback, or screen recording may capture content. Error paths in the CLI or MCP server may include a filename or short diagnostic fragment. MCP clients and agents may retain tool results in transcripts, session files, or logs, and may send them to a model. ContextDrop does not control that retention. Review output before sharing it.

### T5 — Malicious or malformed input crashes or exhausts the process

**Scenario:** A multi-gigabyte file, deeply nested JSON, pathological Unicode, or crafted structured log causes excessive CPU, memory, or a panic.

**Mitigations:**

- CLI input is bounded by `--max-input-bytes` (default 1,000,000 bytes; configurable with a safe hard maximum).
- Server input is capped at 1,000,000 bytes and HTTP bodies at 4 MiB.
- MCP input is capped at 4,000,000 bytes per call; the MCP message is capped at 32 MiB and file metadata is checked before reading.
- Server work runs on blocking threads with a 20-second timeout.
- Concurrency is limited to four jobs.
- JSON parsing uses `serde_json` recursion limits and typed access; no unsafe code is permitted in the workspace.
- Parsing and transformations use standard Rust string handling. Property tests cover arbitrary Unicode input for panic resistance.
- Candidate output must be smaller and pass validation, limiting expansion attacks from repeating markers or schema changes.

**Residual risk:** The CLI intentionally trusts the local user to select the input size; a very large local file can still consume time and memory. The tokenizer loads embedded tables on first use. Timing attacks are not meaningful for this local tool, but response latency can reveal input structure to a local observer. A 20-second server timeout stops the HTTP wait and releases the request permit, but Rust blocking work cannot be forcibly cancelled; a pathological job may continue on a blocking thread after the client receives `504`. Repeated requests from the same local user could therefore overlap beyond the four-request concurrency window. The input bound and deterministic transforms limit, but do not formally eliminate, this local denial-of-service risk.

### T6 — Compression removes evidence needed for diagnosis

**Scenario:** The compressor drops an error, stack frame, path, command, or failed assertion, leading an agent or human to diagnose the wrong problem.

**Mitigations:**

- Built-in protection rules mark failure, fatal, security, assertion, stack, path/line, exit-code, and command context.
- Adjacent context around protected lines is also protected.
- User-defined literal patterns can add project-specific evidence.
- A post-transformation validator checks that the multiset of protected lines is retained (or that JSON evidence counts survive).
- A failed check, missing byte reduction, or invalid transformation returns the original unchanged.
- Omissions and preservation checks are returned as structured data and shown in the dashboard.
- All claims are bounded: presence checks are not semantic equivalence.

**Residual risk:** Heuristics cannot recognize every important line. Novel formats, localized wording, encoded errors, or context that is important but not rare may be omitted. Use Conservative, add preserve patterns, and inspect diffs. Never rely on compression alone when exact transcript fidelity is required.

### T7 — JSON output breaks consumers

**Scenario:** JSON minification or deduplication changes key order, number formatting, or object shape, and an application rejects the result.

**Mitigations:**

- JSON is parsed and serialized with `serde_json`, not regex-edited.
- Structural deduplication inserts an explicit `$contextdrop` marker and emits a schema-change warning.
- Validation parses output and checks evidence.
- Users can select `Input format: Plain text` to avoid JSON rewriting, or use Conservative without structural deduplication.

**Residual risk:** `$contextdrop` is an added key. Strict schemas, signature verification, or downstream code that assumes an exact shape may reject the output. Do not use structural JSON dedupe for protocol payloads where exact shape matters.

### T8 — Unsafe file overwrite or path traversal

**Scenario:** CLI or server reads or writes files outside the intended path.

**Mitigations:**

- The server has no file-path API. Fixtures are embedded by stable ID; unknown IDs return 404.
- SPA static serving is restricted to the configured `web/dist` root, with known routes mapped explicitly.
- The MCP server has exactly one file-reading tool. It requires an explicit path, rejects non-regular files, requires UTF-8, and enforces the input limit before and during the read. It never writes.
- CLI output is explicit. An existing destination is refused unless `--force` is supplied; input and output are distinct path arguments.
- The CLI does not follow content-embedded file references or commands.

**Residual risk:** Users with filesystem access can always select any readable path, and `--force` can overwrite a chosen file. The MCP file tool has no sandbox or allowlist, so an agent can read any file the user can read if prompted to. Use normal file permissions, review agent tool calls, and do not give an untrusted prompt control of the path.

### T9 — Supply-chain compromise

**Scenario:** A dependency contains a known vulnerability or a malicious release.

**Mitigations:**

- `Cargo.lock` and `package-lock.json` are committed and CI installs with locked resolution (`--locked`, `npm ci`).
- CI runs `cargo audit` and `npm audit`.
- The runtime dependency graph is small: compression depends on `regex`, `serde`, `serde_json`, `tiktoken-rs`, and `thiserror`; the MCP server adds only `serde_json` and the core; server and web stacks are separated from the core.
- Unsafe code is forbidden workspace-wide.
- The repository has a security policy and private reporting path.

**Residual risk:** Audit databases only know disclosed advisories. No automated system proves a package is benign. Review dependency updates and changelogs.

### T10 — Browser-side injection through rendered results

**Scenario:** Log text, a fixture name, or a preserve pattern injects HTML, a URL, or script into the dashboard.

**Mitigations:**

- React escapes text content by default.
- No raw HTML rendering is used for input, output, warnings, fixture metadata, or errors.
- The Content Security Policy disallows inline scripts and third-party script origins.
- Links are not constructed from log content in the dashboard.

**Residual risk:** A compromised browser extension or a vulnerability in a third-party frontend dependency could bypass these protections. Keep dependencies updated.

### T11 — An MCP client abuses the file tool to read unintended files

**Scenario:** A prompt injection or a compromised agent asks `compress_context_file` to read credentials, SSH keys, or unrelated source files and then returns compressed content or error details.

**Mitigations:**

- The tool reads exactly one explicit regular-file path; it does not enumerate directories, expand globs, or follow content-embedded links.
- It rejects non-UTF-8 and over-limit files, and does not write a copy.
- The MCP server has no network client, so it cannot upload the content itself.
- Tool calls are visible to the MCP client and can be subject to client approval controls.
- The server instructions tell the agent to use the file tool for large build/test/log output, not arbitrary files.
- The tool result is compressed text, not the original file, and the structured report exposes metrics and fallback state for review.

**Residual risk:** The server runs with the user's filesystem permissions and has no path allowlist or sandbox. A trusted agent can read any file the user can read, and an untrusted prompt can attempt to direct it. Use client-side tool approval, least-privilege accounts, and do not connect the server to agents or prompts that can choose arbitrary paths. ContextDrop cannot prevent the MCP client from retaining or forwarding the result.

## Security invariants

These are expected to hold and are exercised by tests or code structure:

1. Core contains no I/O, process, or network client calls.
2. Non-loopback bind addresses are rejected.
3. API Host and Origin checks run before handlers.
4. POST endpoints reject non-JSON content types.
5. Request and input size limits are enforced.
6. Protected evidence is validated after transformation; failure returns input.
7. User preserve patterns are never compiled as regex.
8. Existing CLI output files are not overwritten without `--force`.
9. Dashboard requests are same-origin and API responses are `no-store`.
10. The MCP server opens no sockets, writes no files, and exposes only read-only tools.
11. The MCP file tool requires an explicit regular-file path and enforces the input limit.
12. No telemetry, analytics, or remote script is present.

## Out of scope

- Multi-user hosting, authentication, authorization, and tenant isolation.
- Encryption at rest or in transit (loopback HTTP is plaintext; MCP stdio is a local pipe).
- Secret detection or redaction. Use dedicated scanners before sharing logs.
- Guaranteeing that compressed output is semantically equivalent for any model.
- Protecting against a compromised operating system, browser, terminal, user account, MCP client, or agent.
- Protecting a local MCP client from prompt injection that directs calls to arbitrary readable paths.
- Malicious browsers or browser extensions.
- Denial-of-service protection beyond local request limits and timeouts.

## Deployment guidance

- Prefer the CLI for sensitive data and batch jobs.
- For the dashboard, run the server on loopback and use `127.0.0.1`.
- For MCP, use a client you trust, review tool-call approvals, and pass only the paths you intend the agent to read.
- Do not add `--bind 0.0.0.0`, reverse proxies, or container port mappings.
- Keep Rust, Node, browsers, and dependencies current.
- Re-run `cargo audit` and `npm audit` before releases.
- Do not publish compressed logs without the same review you would apply to the originals. Compression is not redaction.

## Reporting

Report suspected vulnerabilities privately as described in [SECURITY.md](../SECURITY.md). Do not include real secrets or customer data in an issue. This threat model is reviewed when data flows, API endpoints, dependencies, or deployment assumptions change.

Last updated: 2026-10-24

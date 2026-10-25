# Privacy

ContextDrop is built around a narrow privacy boundary: input is processed on the
machine where it already exists, by code you can inspect, without accounts,
telemetry, or required network services.

This document describes what the shipped version does. It is not a legal privacy
notice and does not replace your organization's data-handling policies.

## Summary

- No telemetry, analytics, crash reporting, or “phone home” behavior.
- No accounts, API keys, cloud service, or database are required.
- Core compression performs no file, network, or subprocess I/O.
- The optional dashboard server listens only on loopback.
- The optional MCP server uses local stdio only and opens no sockets.
- Request and tool content are not logged or persisted by ContextDrop.
- Browser settings are stored in local storage; input text is not.
- Model calls and remote tokenizers are not used.
- Basic tokenizer vocabularies are embedded in the local binary.

## Data flows

### CLI

```text
file ──read──> ContextDrop process ──write──> stdout or explicit --output
stdin ─read──>                       ──write──>
```

- The CLI reads exactly the file or stdin stream you select.
- It writes processed text or JSON only to stdout or the output path you request.
- It never executes input content, follows commands found in logs, or sends text
  to another process over the network.
- Temporary files are not used for compression. Benchmark output goes to the
  destination you choose.
- Human-readable diagnostics go to stderr. They contain counts and labels, not
  the full input text. Error messages may include a path or a short fragment
  where needed for diagnosis.

### MCP server

```text
agent / MCP client
   │  newline-delimited JSON-RPC over stdin/stdout
   ▼
contextdrop-mcp process
   │  explicit path read (compress_context_file only)
   │  in-process function call
   ▼
contextdrop-core ── compressed result ──> agent / MCP client
```

- The MCP server is launched by the client as a local subprocess. It does not
  listen on a TCP port and makes no outbound network requests.
- `compress_context` processes text supplied in the tool call.
- `compress_context_file` reads exactly the one regular UTF-8 file path supplied
  by the caller. It does not enumerate directories, follow a caller-provided
  glob, or write to the path.
- Input and output remain in process memory for the duration of a call. The
  server does not write a cache, history, or log.
- stdout is reserved for JSON-RPC messages. Diagnostics, if any, go to stderr and
  do not include full input or output text.
- The MCP client controls what happens to the tool result after it is returned.
  It may display it, send it to a model, or persist it in its own transcript,
  session files, or logs. That retention is outside ContextDrop's control.

### Local server and dashboard

```text
browser (loopback page)
   │  HTTP POST /api/compress, /api/benchmark, /api/inspect
   ▼
contextdrop-server (127.0.0.1 or ::1)
   │  in-process function call
   ▼
contextdrop-core ── returns result to the browser
```

- The server binds to a loopback address and refuses non-loopback bind addresses.
- The dashboard is served by that same server; its API requests are same-origin.
- Origin, Host, content type, body size, concurrency, and work time are checked.
- Input text is held in process memory for the duration of a request. It is not
  written to disk, logged, or added to a database.
- The server does not use cookies, sessions, or credentials.
- Errors returned to the browser are sanitized. Full inputs are not included in
  logs.

The server exposes a local HTTP API without authentication. Any process running
under the same user account that can reach loopback may be able to call it. Use
the CLI or a separate dedicated account if your local threat model requires
stronger process isolation. Never expose the server through a reverse proxy, port
forward, container publish, or non-loopback interface. This is outside the
supported boundary.

### Dashboard browser storage

The dashboard stores a single preferences object under the key
`contextdrop.settings.v1`. It contains strategy, tokenizer, input format, size
limit, transformation toggles, custom preserve patterns, and optional
input-token pricing preferences.

The dashboard does not persist:

- pasted or loaded input text;
- compressed output;
- result history;
- benchmark runs;
- file contents;
- API responses.

Refreshing or closing the page clears result state from memory. Files you choose
to download are created by your browser only when you press a download control.

If browser storage is unavailable or blocked, the dashboard continues to work for
the current session and clearly reports that preferences will reset on reload.

### Static assets and fonts

The production dashboard bundles its JavaScript, CSS, and font files with Vite.
The page does not load fonts, scripts, analytics, or images from third-party
origins at runtime. The repository does not contain tracking pixels or remote
telemetry endpoints.

### Tokenizers

The default exact tokenizers, `o200k_base` and `cl100k_base`, use vocabulary data
embedded in the `tiktoken-rs` dependency. They do not call OpenAI or any other
service at runtime. The heuristic estimator is local arithmetic and is labelled
as estimated.

### Optional cost estimate

If you enable pricing, you enter an input price and currency yourself. ContextDrop
uses them only to multiply local token counts. No provider API key is requested,
and no billing data is sent anywhere.

## Retention

| Data | Location | Retention |
| --- | --- | --- |
| Input/output text (CLI, HTTP, MCP) | Process memory | Request duration; released afterward |
| Result view | Browser memory | Until reload, navigation state reset, or tab close |
| MCP tool result | Agent / MCP client process | Controlled by the client and its session policy |
| Preferences and custom patterns | Browser localStorage | Until you clear/reset them or browser data is cleared |
| Downloaded results | Your filesystem | Controlled entirely by you |
| Error/count diagnostics | stderr / HTTP response / MCP result | Not persisted by ContextDrop |

Server access logs are not enabled. Startup prints the bind address and a
privacy reminder. It does not print request content.

## What ContextDrop cannot protect against

- Other local processes, browser extensions, or administrators with access to
  your account, process memory, clipboard, or filesystem.
- Data you intentionally paste into another tool while using compressed output.
- Secrets already present in logs. Protection improves the chance that evidence
  survives compression; it is not a secret scanner or redactor.
- MCP clients or agents that store tool results in their own transcripts,
  session files, logs, or remote services. ContextDrop does not control the
  client's retention or transmission policies.
- A path passed to `compress_context_file` being readable by the user running
  the MCP server. The server has no sandbox or path allowlist.
- Misconfiguration that exposes the API beyond loopback.
- Third-party dependencies with vulnerabilities. Dependency audits and the
  security policy describe the current process; see the repository's
  [security policy](../SECURITY.md).

## Verifying the boundary

You can verify the main claims from source and runtime behavior:

```bash
# Core has no network, filesystem, or process dependencies by design.
cargo tree -p contextdrop-core

# The MCP adapter depends on the pure core and local JSON libraries only.
cargo tree -p contextdrop-mcp

# Start the server and confirm it is bound to loopback only.
cargo run -p contextdrop-server -- --bind 127.0.0.1:8787

# Exercise the MCP tool path against a repository fixture.
cargo test -p contextdrop-mcp

# Observe no outbound requests when using the CLI in an isolated profile
# (platform tools vary; no telemetry code exists in the workspace).
```

The server will exit with an error if asked to bind to a non-loopback address. The
MCP server has no listener and no HTTP client. Searching the source for network
clients will find none in `contextdrop-core`, the CLI, or `contextdrop-mcp`; the
only HTTP listener is the loopback Axum server.

## Changes

This document is versioned with the repository. Material changes to data flows
must be reflected here and in the threat model before release.

Last updated: 2026-10-24

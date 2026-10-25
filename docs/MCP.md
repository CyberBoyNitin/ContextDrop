# MCP integration

ContextDrop ships `contextdrop-mcp`, a local stdio
[Model Context Protocol](https://modelcontextprotocol.io/) server. It lets an
agent compress a log or read a log file through a tool call instead of loading
the raw text into context first.

This is a tool server, not a transparent proxy. The agent (or the person using
it) decides when to call ContextDrop. ContextDrop does not intercept model
traffic, wrap an agent process, or read files that were not passed to it.

## Install with one command

Windows PowerShell:

```powershell
irm https://github.com/CyberBoyNitin/ContextDrop/releases/latest/download/install.ps1 | iex
```

macOS or Linux:

```bash
curl -fsSL https://github.com/CyberBoyNitin/ContextDrop/releases/latest/download/install.sh | bash
```

The installer verifies the release checksum, installs `contextdrop-mcp`, and
registers it with OpenCode, Claude Code, Claude Desktop, and Cursor when found.
Restart the client afterwards. Installer options, manual downloads, and
troubleshooting are in [INSTALL.md](INSTALL.md).

The rest of this document covers building from a checkout and manual per-client
setup.

## Build

```bash
cargo build --release --workspace
```

The server binary is:

- macOS/Linux: `./target/release/contextdrop-mcp`
- Windows: `target\release\contextdrop-mcp.exe`

The binary has no runtime configuration and takes no command-line arguments.
An MCP client launches it and speaks newline-delimited JSON-RPC over stdin and
stdout.

## Tools

| Tool | Purpose | Side effects |
| --- | --- | --- |
| `compress_context` | Compress UTF-8 text supplied by the agent. | None. |
| `compress_context_file` | Read one explicit local UTF-8 file and compress it. | Reads the requested path only. Never writes. |
| `inspect_protection` | Show which pasted lines match built-in evidence rules and literal preserve patterns. | None. |
| `list_protection_rules` | List the built-in rule identifiers and labels. | None. |

All tools are annotated read-only and idempotent. There are no tools for
running commands, writing files, making network requests, or reading a
directory tree.

### `compress_context`

Required argument: `text`.

Optional arguments:

- `strategy`: `conservative`, `balanced` (default), or `aggressive`.
- `input_format`: `auto` (default), `plain-text`, `terminal`, `build-log`,
  `test-output`, `json`, or `structured-log`.
- `preserve_patterns`: literal, case-insensitive substrings that must survive.
- `tokenizer`: `o200k_base` (default), `cl100k_base`, or `estimate`. This only
  changes token metrics; compression is independent of the tokenizer.
- `max_input_bytes`: 1 to 4,000,000. The default is 1,000,000.

### `compress_context_file`

Same options, but the required argument is `path` instead of `text`.

The path is resolved against the MCP server process's working directory.
Clients normally start local servers in the workspace they are serving, but
this differs by client; use an absolute path if the agent is unsure. The file
must be a regular, readable UTF-8 file no larger than `max_input_bytes`.

Use this tool instead of reading the raw file and then calling
`compress_context`. The raw file never enters the tool result.

### `inspect_protection`

Required argument: `text`. Optional: `preserve_patterns` and `max_matches`
(default 50, maximum 200). It reports counts, reason counts, and matching line
identifiers. It does not compress.

### `list_protection_rules`

Takes no arguments and returns stable rule identifiers with human-readable
labels.

## Result contract

A successful compression call returns:

- `content`: one text block containing the compressed output.
- `structuredContent`: `{ "source": ..., "result": ... }`.

The nested `result` is the same report shape as the CLI/API JSON report:
strategy, classification, byte/character/token metrics, omissions,
preservation checks, warnings, `fallback_used`, `fallback_reason`, and
`transformation_applied`.

If validation fails or the candidate is not smaller, ContextDrop returns the
original unchanged with `fallback_used: true`. The tool result is not an error
in that case; inspect `fallback_reason`.

The structured report adds tool-result overhead. Byte and token metrics describe
the input and compressed output strings only; they do not include tool schemas,
the structured report, chat framing, or provider billing, and they are not a
promise about total agent-context savings. For a minimal result, a client can
use the `content` text block; a client that shows only `structuredContent` sees
the output under `result.output` plus the full report.

Tool input or file errors return `isError: true` with a text explanation.
Unknown tool names are JSON-RPC invalid-params errors.

## OpenCode V2

Add the server under `mcp.servers` in `opencode.jsonc`. Use an absolute path
on Windows (forward slashes avoid escaping):

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "contextdrop": {
        "type": "local",
        "command": [
          "C:/Users/you/Documents/ContextDrop/target/release/contextdrop-mcp.exe"
        ]
      }
    }
  }
}
```

Or let the CLI write the entry:

```powershell
opencode mcp add contextdrop --global -- C:\Users\you\Documents\ContextDrop\target\release\contextdrop-mcp.exe
opencode mcp list
```

With Code Mode enabled, call the tools as
`tools.contextdrop.compress_context_file`; with Code Mode disabled, they appear
as `contextdrop_compress_context_file`. OpenCode's `protocol` setting accepts
`legacy` (its default), `auto`, or `2026-07-28`. This server supports the legacy
handshake and the modern version, so `auto` probes with `server/discover` and
stays on 2026-07-28 when the probe succeeds. The verified setup below uses the
default legacy handshake.

## Claude Code

Add a user-scoped stdio server:

```bash
claude mcp add --transport stdio --scope user contextdrop -- /absolute/path/to/contextdrop-mcp
```

On Windows:

```powershell
claude mcp add --transport stdio --scope user contextdrop -- C:\path\to\contextdrop-mcp.exe
```

Verify with `claude mcp list` or `/mcp` inside a session. Remove `--scope user`
to add it only to the current project.

## Claude Desktop

Add an entry to `claude_desktop_config.json`:

- Windows: `%APPDATA%\Claude\claude_desktop_config.json`
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "contextdrop": {
      "command": "C:/path/to/contextdrop-mcp.exe",
      "args": []
    }
  }
}
```

Restart Claude Desktop after changing the file.

## Cursor

Use the MCP settings UI or a project `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "contextdrop": {
      "command": "C:/path/to/contextdrop-mcp.exe",
      "args": []
    }
  }
}
```

Use forward slashes in JSON paths, or escape backslashes.

## Generic stdio client

Any MCP client that supports local stdio servers can launch:

```text
command: /absolute/path/to/contextdrop-mcp
args: []
```

The client must not write non-MCP text to stdin and must read newline-delimited
JSON-RPC from stdout. Diagnostics are written only to stderr.

## Suggested agent instruction

Server instructions are supplied during initialization, but a project-level
instruction makes the intended workflow explicit:

```text
When a build, test, CI, or log output is large, call ContextDrop's
compress_context_file tool with its path before reading the raw file. Use
compress_context for output already in hand. If fallback_used is true, do not
assume the output was reduced.
```

## Protocol support

`contextdrop-mcp` is a dual-era server:

- Modern MCP `2026-07-28`: per-request `_meta`, `server/discover`,
  `resultType: "complete"` results, and the modern error code for unsupported
  versions.
- Legacy MCP `2025-11-25`, `2025-06-18`, `2025-03-26`, and `2024-11-05`: the
  `initialize` handshake and `notifications/initialized`.

There is no session state used for compression. Every tool call is independent
and deterministic for the same input and options. The server advertises only
the `tools` capability.

The modern protocol requires `io.modelcontextprotocol/clientCapabilities` on
every request; requests missing it are rejected as invalid params. Legacy
clients may use `_meta` for their own correlation fields without selecting the
modern protocol.

## Security

- The server is local-only and uses stdio. It opens no sockets.
- It sends no telemetry and makes no network requests.
- It does not execute commands or evaluate text, paths, or preserve patterns.
- It reads only the file path passed to `compress_context_file`.
- It does not write or persist files; compression is in memory.
- File access follows the operating-system permissions of the user running
  the MCP client. There is no sandbox or path allowlist.
- File content and compressed output are not logged to stderr. Error messages
  identify the path and the failure, not the file contents.
- Tool results are sent to the MCP client and may be retained in the agent's
  transcript, session files, or logs by that client. ContextDrop cannot control
  the client's retention.
- The input limit is enforced before compression. The MCP message size is
  capped at 32 MiB; a single tool call can request at most 4,000,000 input
  bytes.

Only connect the server to an agent you trust to read the paths you give it.
Do not expose a local agent session to untrusted users or prompts that can
choose arbitrary file paths.

## Troubleshooting

**The client cannot spawn the server.** Use an absolute path in the config and
confirm the binary exists. On Windows, prefer forward slashes in JSON.

**The server appears connected but a file fails.** Paths are relative to the
server process, not necessarily the editor's open folder. Use an absolute path
or set the client's working directory.

**A file is rejected as too large.** Raise `max_input_bytes` up to 4,000,000,
or compress a smaller slice. The default is 1,000,000 bytes.

**The output is unchanged.** Check `fallback_used` and `fallback_reason` in
`structuredContent.result`. The engine falls back when validation fails or no
safe byte reduction exists.

**Claude truncates a large tool result.** Claude Code applies its own MCP output
limits. ContextDrop does not bypass them; use a smaller input or a more
aggressive strategy, and review the diff.

**The agent cannot find the tools.** Restart the client after changing MCP
configuration, then run `opencode mcp list`, `claude mcp list`, or the client's
MCP status view. In OpenCode, check whether Code Mode requires the
`tools.contextdrop.*` form.

## Verification

The repository test suite covers legacy and modern handshakes, tool discovery,
text and file compression, preservation metadata, invalid arguments, missing
and non-UTF-8 files, unsupported protocol versions, notifications, and stdout
purity.

```bash
cargo test -p contextdrop-mcp
cargo clippy -p contextdrop-mcp --all-targets --all-features -- -D warnings
```

Verified on October 24, 2026 with OpenCode 2.0.27 and its default legacy MCP
handshake:

- `opencode mcp list` reported `contextdrop connected`;
- Code Mode exposed all four tools under `tools.contextdrop`;
- a real `compress_context_file` call on
  `fixtures/rust-build-failure.log` reduced 5,841 bytes / 1,645 `o200k_base`
  tokens to 3,955 bytes / 1,295 tokens, omitted 36 repeated lines, retained all
  10 protected occurrences, and passed all three preservation checks;
- a separate modern `2026-07-28` stdio session completed `server/discover`,
  `tools/list`, and `tools/call` with `resultType: "complete"`.

For a manual stdio smoke test, launch the binary, send one `initialize`
request and one `tools/call` request as separate JSON lines, then close stdin.
Every stdout line must be one JSON-RPC message.

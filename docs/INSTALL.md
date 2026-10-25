# Install ContextDrop

ContextDrop ships two binaries in each release:

- `contextdrop` - the CLI (`compress`, `compare`, `benchmark`)
- `contextdrop-mcp` - the local stdio MCP server for AI clients

The installers download the correct archive for your platform, verify its
SHA-256 checksum, install both binaries, and register the MCP server with the AI
clients they detect. Rust and Node are not required.

## Quick install

### Windows

In PowerShell:

```powershell
irm https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.ps1 | iex
```

This installs to `%LOCALAPPDATA%\ContextDrop\bin` and adds that directory to
your user `PATH`.

### macOS and Linux

```bash
curl -fsSL https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.sh | bash
```

This installs to `~/.local/bin`. If that directory is not on your `PATH`, the
installer prints the line to add to your shell profile.

### What gets registered

The installer detects and configures:

| Client | How it is configured |
| --- | --- |
| OpenCode | `opencode mcp add contextdrop --global -- <path>` |
| Claude Code | `claude mcp add --transport stdio --scope user contextdrop -- <path>` |
| Claude Desktop | adds `mcpServers.contextdrop` to `claude_desktop_config.json` |
| Cursor | adds `mcpServers.contextdrop` to `~/.cursor/mcp.json` |

A `.contextdrop-backup` copy is written next to each JSON config before it is
modified. Restart the client after installing.

Then ask the agent:

> Use contextdrop's `compress_context_file` tool on `C:\logs\build.log`, then explain the failure.

The server is a tool, not a proxy. The agent must call it explicitly.

## Installer options

Passing options through `irm ... | iex` or `curl ... | bash` is awkward.
Download the script first if you need flags.

### PowerShell

```powershell
$script = Join-Path $env:TEMP install-contextdrop.ps1
irm https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.ps1 -OutFile $script
& $script -Client none -Version v0.1.0 -InstallDir "$env:LOCALAPPDATA\ContextDrop\bin"
```

Or pass options in one line by running the downloaded script in a child scope:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.ps1))) -Client none
```

| Option | Default | Purpose |
| --- | --- | --- |
| `-Client` | `auto` | `auto`, `opencode`, `claude`, `claude-desktop`, `cursor`, `all`, or `none` |
| `-Version` | `latest` | Release tag such as `v0.1.0` |
| `-InstallDir` | `%LOCALAPPDATA%\ContextDrop\bin` | Where the binaries go |
| `-NoPath` | off | Do not modify the user `PATH` |
| `-BaseUrl` | GitHub | Download base URL for a mirror or test server |
| `-DryRun` | off | Print the plan without changing anything |

### Bash

```bash
curl -fsSL https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.sh \
  | bash -s -- --client none --version v0.1.0
```

| Option | Default | Purpose |
| --- | --- | --- |
| `--client` | `auto` | `auto`, `opencode`, `claude`, `claude-desktop`, `cursor`, `all`, or `none` |
| `--version` | `latest` | Release tag such as `v0.1.0` |
| `--dir` | `~/.local/bin` | Where the binaries go |
| `--base-url` | GitHub | Download base URL for a mirror or test server |
| `--dry-run` | off | Print the plan without changing anything |

Use `-Client`/`--client` to choose one client, `all` to configure every supported
client, or `none` to install only the binaries. With `auto` (the default), only
clients that appear to be installed are configured.

## Manual install

Download the asset for your platform from
[Releases](https://github.com/CyberBoyNitin/ContextDrop/releases):

| Platform | Asset |
| --- | --- |
| Windows x64 | `contextdrop-x86_64-pc-windows-msvc.zip` |
| Linux x64 | `contextdrop-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 | `contextdrop-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Apple silicon | `contextdrop-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `contextdrop-x86_64-apple-darwin.tar.gz` |

Each archive has a matching `.sha256` file. Verify before running:

```powershell
Get-FileHash .\contextdrop-x86_64-pc-windows-msvc.zip -Algorithm SHA256
Get-Content .\contextdrop-x86_64-pc-windows-msvc.zip.sha256
```

```bash
sha256sum -c contextdrop-x86_64-unknown-linux-gnu.tar.gz.sha256
shasum -a 256 -c contextdrop-aarch64-apple-darwin.tar.gz.sha256   # macOS
```

Extract the archive, then register the MCP server as described in
[MCP integration](MCP.md). Use an absolute path to `contextdrop-mcp` in the
client configuration.

## Updating

Run the same installer again. It downloads the latest release and replaces the
two binaries in place. JSON client configs are backed up before each change; the
`contextdrop` entry is updated, other servers are left alone.

To pin a version, pass `-Version v0.1.0` or `--version v0.1.0`.

## Uninstall

1. Remove the MCP entry from each client (use the client's MCP settings or
   remove `mcpServers.contextdrop` from its JSON config).
2. Remove the binaries:
   - Windows: delete `%LOCALAPPDATA%\ContextDrop` and remove that `bin`
     directory from your user `PATH`.
   - macOS/Linux: delete `~/.local/bin/contextdrop` and
     `~/.local/bin/contextdrop-mcp`.
3. Optionally delete `.contextdrop-backup` files left beside client configs.

## Troubleshooting

**PowerShell blocks the installer.** Run it for the current process only:

```powershell
Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
irm https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.ps1 | iex
```

**The download fails.** No release may be published yet, or GitHub may be
unreachable from your network. Check
[Releases](https://github.com/CyberBoyNitin/ContextDrop/releases). For a mirror,
set `CONTEXTDROP_DOWNLOAD_BASE_URL` to a base URL that contains the same asset
names and `.sha256` files.

**The checksum does not match.** Do not run the binary. Download the asset
again; if it still fails, report it via the security policy rather than opening
a public issue.

**`contextdrop` is not found after installing.** Open a new terminal so the
updated `PATH` is loaded, or call the binary by its full path.

**The client does not show the tools.** Restart the client, then check
`opencode mcp list`, `claude mcp list`, or the client's MCP status view. Confirm
the configured command points at the installed `contextdrop-mcp` binary.

**macOS blocks the binary.** The installer clears the quarantine attribute. For
a manual install, run:

```bash
xattr -d com.apple.quarantine /path/to/contextdrop-mcp
```

**The installer edits JSON with Python.** Cursor and Claude Desktop configs are
merged by `python3`. If it is missing, the installer prints the exact snippet to
paste instead.

## Security notes

- The release archives are built by GitHub Actions from the tagged commit. The
  installers fetch them over HTTPS and verify the published SHA-256 checksum
  before extracting. Checksums come from the same release, so they detect
  corruption and truncated downloads, not a compromised release account.
- The MCP server opens no sockets, executes nothing, and writes no files. It
  reads only the explicit path passed to `compress_context_file`.
- `irm ... | iex` and `curl ... | bash` run remote code. If you prefer to review
  first, download the script, read it, and run it locally.
- Release binaries are currently unsigned. Windows SmartScreen and macOS
  Gatekeeper may warn on first run for manual installs. The installers avoid the
  browser quarantine path but do not code-sign the binaries.

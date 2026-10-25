#!/usr/bin/env bash
#
# Install ContextDrop and register its MCP server with local AI clients.
# Supports macOS and Linux. Windows users should run scripts/install.ps1.
#
set -euo pipefail

REPO="CyberBoyNitin/ContextDrop"
SERVER_NAME="contextdrop"
VERSION="${CONTEXTDROP_VERSION:-latest}"
INSTALL_DIR="${CONTEXTDROP_INSTALL_DIR:-$HOME/.local/bin}"
CLIENT="auto"
BASE_URL="${CONTEXTDROP_DOWNLOAD_BASE_URL:-}"
DRY_RUN=0
RELEASES_URL="https://github.com/${REPO}/releases"

usage() {
    cat <<'EOF'
Install ContextDrop and register its MCP server.

Usage:
  install.sh [options]

Options:
  --client <name>    auto (default), opencode, claude, claude-desktop,
                     cursor, all, or none
  --version <tag>    Release tag such as v0.1.0 (default: latest)
  --dir <path>       Install directory (default: ~/.local/bin)
  --base-url <url>   Override the release download base URL (mirrors/testing)
  --dry-run          Print the plan without downloading or changing anything
  -h, --help         Show this help

Examples:
  curl -fsSL https://github.com/CyberBoyNitin/ContextDrop/releases/latest/download/install.sh | bash
  curl -fsSL .../install.sh | bash -s -- --client none
  ./install.sh --client opencode --version v0.1.0
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --client)
            CLIENT="${2:-}"
            shift 2
            ;;
        --client=*)
            CLIENT="${1#*=}"
            shift
            ;;
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        --version=*)
            VERSION="${1#*=}"
            shift
            ;;
        --dir)
            INSTALL_DIR="${2:-}"
            shift 2
            ;;
        --dir=*)
            INSTALL_DIR="${1#*=}"
            shift
            ;;
        --base-url)
            BASE_URL="${2:-}"
            shift 2
            ;;
        --base-url=*)
            BASE_URL="${1#*=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$CLIENT" in
    auto|opencode|claude|claude-desktop|cursor|all|none) ;;
    *)
        echo "Invalid --client: $CLIENT" >&2
        exit 2
        ;;
esac

case "$VERSION" in
    latest) ;;
    v*) ;;
    *) VERSION="v$VERSION" ;;
esac

case "$INSTALL_DIR" in
    "~/"*) INSTALL_DIR="$HOME/${INSTALL_DIR#"~/"}" ;;
esac

os_name="$(uname -s)"
arch_name="$(uname -m)"

case "$os_name" in
    Darwin)
        os_part="apple-darwin"
        claude_dir="$HOME/Library/Application Support/Claude"
        ;;
    Linux)
        os_part="unknown-linux-gnu"
        claude_dir="${XDG_CONFIG_HOME:-$HOME/.config}/Claude"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo "This script is for macOS and Linux." >&2
        echo "On Windows, run scripts/install.ps1 in PowerShell." >&2
        exit 1
        ;;
    *)
        echo "Unsupported operating system: $os_name" >&2
        exit 1
        ;;
esac

case "$arch_name" in
    x86_64|amd64)
        arch_part="x86_64"
        ;;
    arm64|aarch64)
        arch_part="aarch64"
        ;;
    *)
        echo "Unsupported CPU architecture: $arch_name" >&2
        exit 1
        ;;
esac

target="${arch_part}-${os_part}"
asset="contextdrop-${target}.tar.gz"

if [[ -n "$BASE_URL" ]]; then
    download_base="${BASE_URL%/}"
elif [[ "$VERSION" == "latest" ]]; then
    download_base="https://github.com/${REPO}/releases/latest/download"
else
    download_base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

archive_url="${download_base}/${asset}"
checksum_url="${archive_url}.sha256"

detect_clients() {
    if command -v opencode >/dev/null 2>&1; then printf '%s\n' opencode; fi
    if command -v claude >/dev/null 2>&1; then printf '%s\n' claude; fi
    if [[ -d "$HOME/.cursor" ]]; then printf '%s\n' cursor; fi
    if [[ -d "$claude_dir" ]]; then printf '%s\n' claude-desktop; fi
}

case "$CLIENT" in
    auto)
        clients="$(detect_clients | tr '\n' ' ')"
        ;;
    all)
        clients="opencode claude cursor claude-desktop"
        ;;
    none)
        clients=""
        ;;
    *)
        clients="$CLIENT"
        ;;
esac

if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "ContextDrop installer (dry run)"
    echo "  Platform asset: ${asset}"
    echo "  Version:        ${VERSION}"
    echo "  Archive:        ${archive_url}"
    echo "  Install dir:    ${INSTALL_DIR}"
    if [[ -z "$clients" ]]; then
        echo "  MCP clients:    none (registration skipped)"
    else
        echo "  MCP clients:    ${clients}"
    fi
    exit 0
fi

download() {
    local url="$1"
    local output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --retry-delay 1 -o "$output" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$output" "$url"
    else
        echo "curl or wget is required to download ContextDrop." >&2
        return 1
    fi
}

verify_checksum() {
    local archive="$1"
    local checksum_file="$2"
    local expected actual

    expected="$(awk 'NR == 1 { print $1 }' "$checksum_file" | tr '[:upper:]' '[:lower:]')"
    if [[ -z "$expected" ]]; then
        echo "The checksum file is empty." >&2
        return 1
    fi

    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$archive" | awk '{ print $1 }')"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$archive" | awk '{ print $1 }')"
    else
        echo "No SHA-256 tool found (sha256sum or shasum)." >&2
        return 1
    fi

    [[ "$actual" == "$expected" ]]
}

merge_mcp_json() {
    local config_path="$1"
    local command_path="$2"

    python3 - "$config_path" "$command_path" <<'PY'
import json
import os
import sys

config_path, command_path = sys.argv[1], sys.argv[2]
data = {}
if os.path.exists(config_path) and os.path.getsize(config_path) > 0:
    with open(config_path, encoding="utf-8") as handle:
        data = json.load(handle)

servers = data.setdefault("mcpServers", {})
servers["contextdrop"] = {"command": command_path, "args": []}

directory = os.path.dirname(config_path)
if directory:
    os.makedirs(directory, exist_ok=True)

temporary = config_path + ".tmp"
with open(temporary, "w", encoding="utf-8") as handle:
    json.dump(data, handle, indent=2)
    handle.write("\n")
os.replace(temporary, config_path)
PY
}

register_opencode() {
    if ! command -v opencode >/dev/null 2>&1; then
        return 1
    fi
    if opencode mcp add "$SERVER_NAME" --global -- "$MCP_PATH" >/dev/null 2>&1; then
        echo "  OpenCode: added to the global config"
        return 0
    fi
    echo "  OpenCode: automatic registration failed"
    echo "    opencode mcp add $SERVER_NAME --global -- $MCP_PATH"
    return 1
}

register_claude_code() {
    if ! command -v claude >/dev/null 2>&1; then
        return 1
    fi

    local output
    if output="$(claude mcp add --transport stdio --scope user "$SERVER_NAME" -- "$MCP_PATH" 2>&1)"; then
        echo "  Claude Code: added (user scope)"
        return 0
    fi
    if printf '%s' "$output" | grep -qi 'already exists'; then
        echo "  Claude Code: already configured"
        return 0
    fi

    echo "  Claude Code: automatic registration failed"
    echo "    claude mcp add --transport stdio --scope user $SERVER_NAME -- $MCP_PATH"
    return 1
}

register_json_client() {
    local display_name="$1"
    local config_path="$2"
    local config_dir

    config_dir="$(dirname "$config_path")"
    if [[ ! -d "$config_dir" ]]; then
        return 1
    fi

    if [[ -f "$config_path" ]]; then
        cp "$config_path" "${config_path}.contextdrop-backup"
    fi

    if command -v python3 >/dev/null 2>&1; then
        if merge_mcp_json "$config_path" "$MCP_PATH"; then
            echo "  ${display_name}: added"
            return 0
        fi
    else
        echo "  ${display_name}: python3 not found, cannot edit automatically"
    fi

    echo "    Add this to ${config_path}:"
    echo "    { \"mcpServers\": { \"${SERVER_NAME}\": { \"command\": \"${MCP_PATH}\", \"args\": [] } } }"
    return 1
}

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/contextdrop.XXXXXX")"
cleanup() {
    rm -rf "$temp_dir"
}
trap cleanup EXIT

archive_path="${temp_dir}/${asset}"
checksum_path="${temp_dir}/${asset}.sha256"

echo "Downloading ${archive_url}"
if ! download "$archive_url" "$archive_path"; then
    echo "Could not download a ContextDrop release." >&2
    echo "Check ${RELEASES_URL} for published assets." >&2
    exit 1
fi

if ! download "$checksum_url" "$checksum_path"; then
    echo "Could not download the SHA-256 checksum for ${asset}." >&2
    exit 1
fi

if ! verify_checksum "$archive_path" "$checksum_path"; then
    echo "Checksum verification failed for ${asset}." >&2
    exit 1
fi
echo "Verified SHA-256 checksum."

tar -xzf "$archive_path" -C "$temp_dir"
package_root="${temp_dir}/contextdrop-${target}"
if [[ ! -f "${package_root}/contextdrop" || ! -f "${package_root}/contextdrop-mcp" ]]; then
    echo "The release archive did not contain the expected binaries." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 0755 "${package_root}/contextdrop" "${INSTALL_DIR}/contextdrop"
install -m 0755 "${package_root}/contextdrop-mcp" "${INSTALL_DIR}/contextdrop-mcp"

if [[ "$os_name" == "Darwin" ]]; then
    xattr -d com.apple.quarantine "${INSTALL_DIR}/contextdrop" 2>/dev/null || true
    xattr -d com.apple.quarantine "${INSTALL_DIR}/contextdrop-mcp" 2>/dev/null || true
fi

MCP_PATH="${INSTALL_DIR}/contextdrop-mcp"

echo ""
echo "Installed to ${INSTALL_DIR}"

case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        echo "Add the install directory to your PATH:"
        echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
        echo "Then add that line to ~/.zshrc, ~/.bashrc, or your shell profile."
        ;;
esac

registered=""
if [[ -n "$clients" ]]; then
    echo "Configuring MCP clients:"
    for requested in $clients; do
        case "$requested" in
            opencode)
                if register_opencode; then registered="${registered} OpenCode"; fi
                ;;
            claude)
                if register_claude_code; then registered="${registered} Claude Code"; fi
                ;;
            cursor)
                if register_json_client "Cursor" "$HOME/.cursor/mcp.json"; then
                    registered="${registered} Cursor"
                fi
                ;;
            claude-desktop)
                if register_json_client "Claude Desktop" "${claude_dir}/claude_desktop_config.json"; then
                    registered="${registered} Claude Desktop"
                fi
                ;;
        esac
    done
else
    echo "MCP client registration skipped."
fi

echo ""
if [[ -n "$registered" ]]; then
    echo "MCP configured for:${registered}"
    echo "Restart the client, then ask it to compress a log with contextdrop."
else
    echo "No MCP client was configured."
    echo "Add this local stdio server in your client:"
    echo "  command: ${MCP_PATH}"
    echo "  args: []"
fi

echo ""
echo "Try the CLI:"
echo "  ${INSTALL_DIR}/contextdrop compress ./build.log"

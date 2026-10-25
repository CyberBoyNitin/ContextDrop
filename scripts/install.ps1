#Requires -Version 5.1
<#
.SYNOPSIS
    Install ContextDrop and register its MCP server with local AI clients.

.DESCRIPTION
    Downloads the latest (or a pinned) ContextDrop release for Windows, verifies
    its SHA-256 checksum, installs the CLI and MCP server, and registers the MCP
    server with every supported client it can detect.

.PARAMETER Client
    Which client to configure: auto (default), opencode, claude,
    claude-desktop, cursor, all, or none.

.PARAMETER Version
    Release tag to install, such as v0.1.0. Defaults to the latest release.

.PARAMETER InstallDir
    Directory for contextdrop.exe and contextdrop-mcp.exe.

.PARAMETER BaseUrl
    Override the release download base URL. Useful for mirrors and testing.

.PARAMETER NoPath
    Do not add the install directory to the user PATH.

.PARAMETER DryRun
    Print the plan without downloading or changing anything.

.EXAMPLE
    irm https://raw.githubusercontent.com/CyberBoyNitin/ContextDrop/main/scripts/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Client none -Version v0.1.0

.EXAMPLE
    .\install.ps1 -Client opencode
#>
[CmdletBinding()]
param(
    [ValidateSet('auto', 'opencode', 'claude', 'claude-desktop', 'cursor', 'all', 'none')]
    [string]$Client = 'auto',

    [string]$Version = 'latest',

    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'ContextDrop\bin'),

    [string]$BaseUrl = $env:CONTEXTDROP_DOWNLOAD_BASE_URL,

    [switch]$NoPath,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'CyberBoyNitin/ContextDrop'
$ServerName = 'contextdrop'
$ReleasesUrl = "https://github.com/$Repo/releases"

function Assert-Windows {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'This installer is for Windows. On macOS or Linux, use scripts/install.sh.'
    }
}

function Normalize-Version {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -eq 'latest') { return 'latest' }
    if ($Value -notmatch '^[vV]') { return "v$Value" }
    return $Value
}

function Get-PlatformAsset {
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    if ($architecture -eq 'Arm64') {
        Write-Warning 'Windows on Arm detected. Installing the x64 build, which runs under emulation.'
    }
    elseif ($architecture -ne 'X64') {
        throw "Unsupported Windows architecture: $architecture"
    }
    return 'contextdrop-x86_64-pc-windows-msvc.zip'
}

function Get-RequestedClients {
    if ($Client -eq 'auto') {
        $found = @()
        if (Get-Command opencode -ErrorAction SilentlyContinue) { $found += 'opencode' }
        if (Get-Command claude -ErrorAction SilentlyContinue) { $found += 'claude' }
        if (Test-Path -LiteralPath (Join-Path $env:APPDATA 'Claude')) { $found += 'claude-desktop' }
        if (Test-Path -LiteralPath (Join-Path $env:USERPROFILE '.cursor')) { $found += 'cursor' }
        return $found
    }
    if ($Client -eq 'all') { return @('opencode', 'claude', 'claude-desktop', 'cursor') }
    if ($Client -eq 'none') { return @() }
    return @($Client)
}

function Add-OpenCodeServer {
    param([Parameter(Mandatory)][string]$Command)

    if (-not (Get-Command opencode -ErrorAction SilentlyContinue)) { return $false }

    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & opencode mcp add --global $ServerName -- $Command *> $null
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
    }

    if ($exitCode -ne 0) {
        Write-Warning "OpenCode: automatic registration failed. Run: opencode mcp add --global $ServerName -- `"$Command`""
        return $false
    }

    Write-Host '  OpenCode: added to the global config'
    return $true
}

function Add-ClaudeCodeServer {
    param([Parameter(Mandatory)][string]$Command)

    if (-not (Get-Command claude -ErrorAction SilentlyContinue)) { return $false }

    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = & claude mcp add --transport stdio --scope user $ServerName -- $Command 2>&1 | Out-String
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
    }

    if ($exitCode -eq 0) {
        Write-Host '  Claude Code: added (user scope)'
        return $true
    }
    if ($output -match 'already exists') {
        Write-Host '  Claude Code: already configured'
        return $true
    }

    Write-Warning "Claude Code: automatic registration failed. Run: claude mcp add --transport stdio --scope user $ServerName -- `"$Command`""
    return $false
}

function Set-JsonMcpServer {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Command
    )

    $directory = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $directory)) { return $false }

    $config = $null
    if (Test-Path -LiteralPath $Path) {
        $raw = Get-Content -LiteralPath $Path -Raw
        if (-not [string]::IsNullOrWhiteSpace($raw)) {
            try {
                $config = $raw | ConvertFrom-Json
            }
            catch {
                throw "Cannot parse $Path as JSON: $($_.Exception.Message)"
            }
            Copy-Item -LiteralPath $Path -Destination "$Path.contextdrop-backup" -Force
        }
    }

    if ($null -eq $config) { $config = [pscustomobject]@{} }

    $configNames = @($config.PSObject.Properties | ForEach-Object { $_.Name })
    if ($configNames -notcontains 'mcpServers') {
        $config | Add-Member -NotePropertyName 'mcpServers' -NotePropertyValue ([pscustomobject]@{})
    }
    elseif ($null -eq $config.mcpServers) {
        $config.mcpServers = [pscustomobject]@{}
    }

    $entry = [pscustomobject]@{ command = $Command; args = @() }
    $serverNames = @($config.mcpServers.PSObject.Properties | ForEach-Object { $_.Name })
    if ($serverNames -contains $ServerName) {
        $config.mcpServers.$ServerName = $entry
    }
    else {
        $config.mcpServers | Add-Member -NotePropertyName $ServerName -NotePropertyValue $entry
    }

    $json = $config | ConvertTo-Json -Depth 100
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, $utf8)
    return $true
}

function Add-JsonClientServer {
    param(
        [Parameter(Mandatory)][string]$DisplayName,
        [Parameter(Mandatory)][string]$ConfigPath,
        [Parameter(Mandatory)][string]$Command
    )

    if (Set-JsonMcpServer -Path $ConfigPath -Command $Command) {
        Write-Host "  ${DisplayName}: added"
        return $true
    }
    return $false
}

function Add-InstallDirToPath {
    param([Parameter(Mandatory)][string]$Directory)

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @()
    if (-not [string]::IsNullOrWhiteSpace($userPath)) {
        $parts = @($userPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }

    $alreadyPresent = $false
    foreach ($part in $parts) {
        if ($part.TrimEnd('\') -ieq $Directory.TrimEnd('\')) { $alreadyPresent = $true; break }
    }

    if (-not $alreadyPresent) {
        $newPath = (@($parts) + $Directory) -join ';'
        $environmentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        if ($null -eq $environmentKey) {
            [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        }
        else {
            try {
                $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
                if ($environmentKey.GetValueNames() -contains 'Path') {
                    $kind = $environmentKey.GetValueKind('Path')
                }
                $environmentKey.SetValue('Path', $newPath, $kind)
            }
            finally {
                $environmentKey.Close()
            }
        }
        Write-Host "Added $Directory to the user PATH. Open a new terminal to use it."
    }

    $sessionParts = @($env:Path -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if (-not ($sessionParts | Where-Object { $_.TrimEnd('\') -ieq $Directory.TrimEnd('\') })) {
        $env:Path = "$Directory;$env:Path"
    }
}

Assert-Windows

$Version = Normalize-Version $Version
$Asset = Get-PlatformAsset
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$RequestedClients = @(Get-RequestedClients)

if (-not [string]::IsNullOrWhiteSpace($BaseUrl)) {
    $downloadBase = $BaseUrl.TrimEnd('/')
}
elseif ($Version -eq 'latest') {
    $downloadBase = "https://github.com/$Repo/releases/latest/download"
}
else {
    $downloadBase = "https://github.com/$Repo/releases/download/$Version"
}

$archiveUrl = "$downloadBase/$Asset"
$checksumUrl = "$archiveUrl.sha256"
$mcpPath = Join-Path $InstallDir 'contextdrop-mcp.exe'

if ($DryRun) {
    Write-Host "ContextDrop installer (dry run)"
    Write-Host "  Platform asset: $Asset"
    Write-Host "  Version:        $Version"
    Write-Host "  Archive:        $archiveUrl"
    Write-Host "  Install dir:    $InstallDir"
    Write-Host "  Update PATH:    $(-not $NoPath)"
    if ($RequestedClients.Count -eq 0) {
        Write-Host "  MCP clients:    none (registration skipped)"
    }
    else {
        Write-Host "  MCP clients:    $($RequestedClients -join ', ')"
    }
    return
}

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ('contextdrop-install-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

try {
    $archivePath = Join-Path $tempDir $Asset
    $checksumPath = Join-Path $tempDir "$Asset.sha256"

    Write-Host "Downloading $archiveUrl"
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $archiveUrl -OutFile $archivePath -UseBasicParsing
        Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath -UseBasicParsing
    }
    catch {
        throw "Could not download a ContextDrop release. Check $ReleasesUrl. $($_.Exception.Message)"
    }

    $expectedHash = ((Get-Content -LiteralPath $checksumPath -Raw) -split '\s+')[0].ToLowerInvariant()
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expectedHash -ne $actualHash) {
        throw "Checksum mismatch for $Asset. Expected $expectedHash but got $actualHash."
    }
    Write-Host 'Verified SHA-256 checksum.'

    Expand-Archive -LiteralPath $archivePath -DestinationPath $tempDir -Force
    $packageRoot = Get-ChildItem -LiteralPath $tempDir -Directory |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'contextdrop-mcp.exe') } |
        Select-Object -First 1
    if ($null -eq $packageRoot) {
        throw 'The release archive did not contain contextdrop-mcp.exe.'
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    foreach ($binary in @('contextdrop.exe', 'contextdrop-mcp.exe')) {
        $source = Join-Path $packageRoot.FullName $binary
        if (-not (Test-Path -LiteralPath $source)) {
            throw "The release archive is missing $binary."
        }
        $destination = Join-Path $InstallDir $binary
        try {
            Copy-Item -LiteralPath $source -Destination $destination -Force
        }
        catch {
            throw "Could not install $binary. Close any AI client using ContextDrop, then run the installer again. $($_.Exception.Message)"
        }
        Unblock-File -LiteralPath $destination -ErrorAction SilentlyContinue
    }

    if (-not $NoPath) {
        Add-InstallDirToPath -Directory $InstallDir
    }

    Write-Host ''
    Write-Host "Installed to $InstallDir"

    $registered = @()
    if ($RequestedClients.Count -gt 0) {
        Write-Host 'Configuring MCP clients:'
        foreach ($requested in $RequestedClients) {
            try {
                switch ($requested) {
                    'opencode' {
                        if (Add-OpenCodeServer -Command $mcpPath) { $registered += 'OpenCode' }
                    }
                    'claude' {
                        if (Add-ClaudeCodeServer -Command $mcpPath) { $registered += 'Claude Code' }
                    }
                    'claude-desktop' {
                        $desktopConfig = Join-Path $env:APPDATA 'Claude\claude_desktop_config.json'
                        if (Add-JsonClientServer -DisplayName 'Claude Desktop' -ConfigPath $desktopConfig -Command $mcpPath) {
                            $registered += 'Claude Desktop'
                        }
                    }
                    'cursor' {
                        $cursorConfig = Join-Path $env:USERPROFILE '.cursor\mcp.json'
                        if (Add-JsonClientServer -DisplayName 'Cursor' -ConfigPath $cursorConfig -Command $mcpPath) {
                            $registered += 'Cursor'
                        }
                    }
                }
            }
            catch {
                Write-Warning "${requested}: $($_.Exception.Message)"
            }
        }
    }
    else {
        Write-Host 'MCP client registration skipped.'
    }

    Write-Host ''
    if ($registered.Count -gt 0) {
        Write-Host "MCP configured for: $($registered -join ', ')"
        Write-Host 'Restart the client, then ask it to compress a log with contextdrop.'
    }
    else {
        Write-Host 'No MCP client was configured.'
        Write-Host 'Add this local stdio server in your client:'
        Write-Host "  command: $mcpPath"
        Write-Host '  args: []'
    }

    Write-Host ''
    Write-Host 'Try the CLI:'
    Write-Host "  & `"$(Join-Path $InstallDir 'contextdrop.exe')`" compress .\build.log"
}
finally {
    Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}

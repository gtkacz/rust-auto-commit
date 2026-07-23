# cgen installer for Windows
# Usage: irm https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.ps1 | iex

param(
    [string]$Version = $env:CGEN_VERSION,
    [string]$InstallDir = $env:CGEN_INSTALL_DIR
)

$ErrorActionPreference = "Stop"

$Repo = "gtkacz/smart-commit-rs"
$BinaryName = "cgen.exe"
$Artifact = "cgen-windows-amd64.exe"

function Write-Info($msg) { Write-Host $msg -ForegroundColor Cyan }
function Write-Success($msg) { Write-Host $msg -ForegroundColor Green }
function Write-Err($msg) { Write-Host "error: $msg" -ForegroundColor Red; exit 1 }

if ([string]::IsNullOrWhiteSpace($Version)) {
    Write-Info "Fetching latest release..."
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
        $Version = $Release.tag_name
    } catch {
        Write-Err "Could not fetch latest release. Check https://github.com/$Repo/releases"
    }
}
if ($Version -notmatch '^v?\d+\.\d+\.\d+$') {
    Write-Err "Invalid release version '$Version'"
}

Write-Info "Latest version: $Version"

$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Artifact"
$ChecksumUrl = "https://github.com/$Repo/releases/download/$Version/checksums.sha256"

# Determine install directory
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "cgen"
}
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$InstallPath = Join-Path $InstallDir $BinaryName
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) "cgen-install-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $TempDir | Out-Null
$TempBinary = Join-Path $TempDir $BinaryName
$TempChecksums = Join-Path $TempDir "checksums.sha256"

try {
    Write-Info "Downloading $Artifact..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempBinary -UseBasicParsing
    Invoke-WebRequest -Uri $ChecksumUrl -OutFile $TempChecksums -UseBasicParsing

    $ChecksumLine = Get-Content -LiteralPath $TempChecksums |
        Where-Object { $_ -match "^[0-9A-Fa-f]{64}\s+\*?$([regex]::Escape($Artifact))$" } |
        Select-Object -First 1
    if (-not $ChecksumLine) {
        Write-Err "Release does not contain a checksum for $Artifact"
    }
    $Expected = ($ChecksumLine -split '\s+')[0].ToLowerInvariant()
    $Actual = (Get-FileHash -LiteralPath $TempBinary -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Expected -ne $Actual) {
        Write-Err "Checksum mismatch for $Artifact; installation aborted"
    }

    $StagedPath = Join-Path $InstallDir ".cgen-install-$PID.exe"
    Copy-Item -LiteralPath $TempBinary -Destination $StagedPath -Force
    Move-Item -LiteralPath $StagedPath -Destination $InstallPath -Force
} catch {
    Write-Err "Installation failed: $($_.Exception.Message)"
} finally {
    Remove-Item -LiteralPath $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}

# Add to PATH if not already there
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$PathEntries = @($UserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
if ($PathEntries -notcontains $InstallDir) {
    Write-Info "Adding $InstallDir to user PATH..."
    $NewUserPath = (($PathEntries + $InstallDir) -join ';')
    [Environment]::SetEnvironmentVariable("Path", $NewUserPath, "User")
    $env:Path = "$env:Path;$InstallDir"
}

Write-Success "`ncgen $Version installed successfully!"
Write-Host ""
Write-Host "  Installed to: $InstallPath"
Write-Host "  Run 'cgen config' to set up your API key."
Write-Host "  Run 'cgen --help' for usage information."
Write-Host ""
Write-Host "  Restart your terminal for PATH changes to take effect." -ForegroundColor Yellow

# ==============================================================================
# Bilterm — Plain Binary Installer for Windows (PowerShell)
# ==============================================================================
# Installs `bilterm.exe` for users who just want to run the binary (from a
# prebuilt GitHub Release, an existing local build, or `cargo build --release`).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 [OPTIONS]
# ==============================================================================

param(
    [switch]$Local,
    [switch]$Cargo,
    [string]$Prefix = "",
    [switch]$BinaryOnly,
    [switch]$FromSource,
    [string]$Tag = "latest",
    [switch]$Uninstall,
    [switch]$Force,
    [switch]$DryRun,
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$GithubRepo = if ($env:BILTERM_GITHUB_REPO) { $env:BILTERM_GITHUB_REPO } else { "bsommers/bilterm" }
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir

if ($Help) {
    Write-Host @"
Bilterm Plain Binary Installer (Windows PowerShell)

Usage:
  .\scripts\install.ps1 [-Local] [-Cargo] [-Prefix <DIR>] [-BinaryOnly] [-FromSource] [-Uninstall] [-DryRun]

Options:
  -Local          Install to $HOME\.local\bin\bilterm.exe (default)
  -Cargo          Install to $HOME\.cargo\bin\bilterm.exe
  -Prefix <DIR>   Install to <DIR>\bin\bilterm.exe
  -BinaryOnly     Download prebuilt binary from GitHub Releases (no Rust needed)
  -FromSource     Compile from source using Cargo
  -Tag <TAG>      Release tag to download (default: latest)
  -Uninstall      Remove installed bilterm.exe
  -Force          Overwrite existing target binary
  -DryRun         Print actions without modifying the filesystem
  -Help           Show this help message
"@
    exit 0
}

function Write-Info($msg) { Write-Host "[bilterm-install] $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "  [OK] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host " [ERR] $msg" -ForegroundColor Red }

if ($Prefix) {
    $BinDir = Join-Path (Resolve-Path -LiteralPath $Prefix -ErrorAction SilentlyContinue ?? $Prefix) "bin"
} elseif ($Cargo) {
    $BinDir = Join-Path $HOME ".cargo\bin"
} else {
    $BinDir = Join-Path $HOME ".local\bin"
}

$TargetBin = Join-Path $BinDir "bilterm.exe"

if ($Uninstall) {
    if (Test-Path $TargetBin) {
        if ($DryRun) {
            Write-Host "  [dry-run] Remove-Item $TargetBin"
        } else {
            Remove-Item -Force $TargetBin
        }
        Write-Ok "Removed $TargetBin"
    } else {
        Write-Warn "Binary not found at $TargetBin"
    }
    exit 0
}

function Try-DownloadPrebuilt($dest) {
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "i686" }
    $triple = "$arch-pc-windows-msvc"
    $url = if ($Tag -eq "latest") {
        "https://github.com/$GithubRepo/releases/latest/download/bilterm-$triple.zip"
    } else {
        "https://github.com/$GithubRepo/releases/download/$Tag/bilterm-$triple.zip"
    }
    Write-Info "Checking for prebuilt binary at $url..."
    if ($DryRun) {
        Write-Host "  [dry-run] Invoke-WebRequest -Uri $url"
        return $true
    }
    $tmpZip = Join-Path $env:TEMP "bilterm-$triple.zip"
    $tmpExtract = Join-Path $env:TEMP "bilterm-extract-$PID"
    try {
        Invoke-WebRequest -Uri $url -OutFile $tmpZip -ErrorAction Stop
        Expand-Archive -Path $tmpZip -DestinationPath $tmpExtract -Force
        $exeItem = Get-ChildItem -Path $tmpExtract -Filter "bilterm.exe" -Recurse | Select-Object -First 1
        if ($exeItem) {
            Copy-Item -Path $exeItem.FullName -Destination $dest -Force
            Write-Ok "Downloaded prebuilt bilterm.exe ($triple)"
            return $true
        }
    } catch {
        return $false
    } finally {
        Remove-Item -Force -Recurse $tmpZip, $tmpExtract -ErrorAction SilentlyContinue
    }
    return $false
}

if (-not $DryRun) {
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
} else {
    Write-Host "  [dry-run] New-Item -ItemType Directory -Force -Path $BinDir"
}

$localBuild = Join-Path $RepoRoot "target\release\bilterm.exe"
$cargoToml = Join-Path $RepoRoot "Cargo.toml"

if ($BinaryOnly) {
    if (-not (Try-DownloadPrebuilt $TargetBin)) {
        Write-Err "Could not download prebuilt Windows binary."
        Write-Err "Remedy: omit -BinaryOnly or run '.\scripts\setup.ps1 -Yes -Install' to build from source."
        exit 1
    }
} elseif (-not $FromSource -and (Test-Path $localBuild)) {
    Write-Info "Using existing local build at $localBuild"
    if ($DryRun) {
        Write-Host "  [dry-run] Copy-Item $localBuild $TargetBin"
    } else {
        Copy-Item -Path $localBuild -Destination $TargetBin -Force
    }
} elseif (-not $FromSource -and (Try-DownloadPrebuilt $TargetBin)) {
    # Installed via prebuilt release
} else {
    $cargoBin = Join-Path $HOME ".cargo\bin"
    if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Cargo is not installed and no prebuilt binary was found."
        Write-Err "Remedy: run '.\scripts\setup.ps1 -Yes -Install' to install Rust and build Bilterm."
        exit 1
    }
    if (Test-Path $cargoToml) {
        Write-Info "Building release binary from $cargoToml..."
        Push-Location $RepoRoot
        try {
            if ($DryRun) {
                Write-Host "  [dry-run] cargo build --release"
            } else {
                & cargo build --release
                Copy-Item -Path $localBuild -Destination $TargetBin -Force
            }
        } finally {
            Pop-Location
        }
    } else {
        Write-Info "Installing bilterm from https://github.com/$GithubRepo.git via cargo..."
        if ($DryRun) {
            Write-Host "  [dry-run] cargo install --git https://github.com/$GithubRepo.git"
        } else {
            & cargo install --git "https://github.com/$GithubRepo.git"
            $built = Join-Path $HOME ".cargo\bin\bilterm.exe"
            if ((Test-Path $built) -and ($built -ne $TargetBin)) {
                Copy-Item -Path $built -Destination $TargetBin -Force
            }
        }
    }
}

Write-Ok "Installed binary: $TargetBin"

if ($env:PATH -notlike "*$BinDir*") {
    Write-Warn "'$BinDir' is not currently in your PATH."
    Write-Host "  Add it to your User PATH in PowerShell:"
    Write-Host "    [Environment]::SetEnvironmentVariable('Path', `$env:Path + ';$BinDir', 'User')"
}

Write-Host @"

Next steps:
  1. bilterm -v                         # verify installed version
  2. bilterm                            # launch interactive IBM 3270 x Borland TUI
  3. bilterm -h                         # view CLI switches & options
"@

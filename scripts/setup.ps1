# ==============================================================================
# Bilterm — Windows Developer & Build Environment Setup (PowerShell)
# ==============================================================================
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\scripts\setup.ps1 [OPTIONS]
# ==============================================================================

param(
    [switch]$Check,
    [switch]$DryRun,
    [switch]$Yes,
    [switch]$WithDevTools,
    [switch]$Build,
    [switch]$Install,
    [string]$Prefix = "",
    [switch]$Help
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$RequiredChannel = "1.90"

if ($Help) {
    Write-Host @"
Bilterm Developer & Build Environment Setup (Windows PowerShell)

Usage:
  .\scripts\setup.ps1 [-Check] [-DryRun] [-Yes] [-WithDevTools] [-Build] [-Install] [-Prefix <DIR>]

Options:
  -Check          Verify installed tools without modifying the system
  -DryRun         Print commands without executing them
  -Yes            Auto-confirm package and rustup installations
  -WithDevTools   Install cargo-nextest, cargo-insta, and cargo-llvm-cov
  -Build          Build release binary (cargo build --release) after setup
  -Install        Install bilterm.exe after setup (via scripts\install.ps1)
  -Prefix <DIR>   Custom install directory passed to scripts\install.ps1
  -Help           Show this help message
"@
    exit 0
}

function Write-Info($msg) { Write-Host "[bilterm-setup] $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "  [OK] $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "[WARN] $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host " [ERR] $msg" -ForegroundColor Red }

$Missing = 0

# 1. Check Git
if (Get-Command git -ErrorAction SilentlyContinue) {
    Write-Ok "git found ($((Get-Command git).Source))"
} else {
    Write-Warn "git not found"
    if ($Check) {
        $Missing++
        Write-Warn "Remedy: winget install --id Git.Git -e"
    } elseif ($DryRun) {
        Write-Host "  [dry-run] winget install --id Git.Git -e"
    } elseif (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Info "Installing Git via winget..."
        winget install --id Git.Git -e --accept-source-agreements --accept-package-agreements
    } else {
        Write-Err "Please install Git from https://git-scm.com/download/win"
        exit 1
    }
}

# 2. Check MSVC Build Tools / Linker
$ hasCl = [bool](Get-Command cl.exe -ErrorAction SilentlyContinue)
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVsCpp = $hasCl -or ((Test-Path $vsWhere) -and (& $vsWhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath))

if ($hasVsCpp) {
    Write-Ok "MSVC C++ Build Tools found"
} else {
    Write-Warn "Visual Studio C++ Build Tools not detected (required by Rust on Windows MSVC)"
    if ($Check) {
        $Missing++
        Write-Warn "Remedy: winget install --id Microsoft.VisualStudio.2022.BuildTools --override '--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'"
    } elseif ($DryRun) {
        Write-Host "  [dry-run] winget install --id Microsoft.VisualStudio.2022.BuildTools ..."
    } elseif ($Yes -and (Get-Command winget -ErrorAction SilentlyContinue)) {
        Write-Info "Installing Visual Studio 2022 C++ Build Tools via winget..."
        winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    } else {
        Write-Warn "Run with -Yes to install Visual Studio Build Tools via winget, or install from https://visualstudio.microsoft.com/visual-cpp-build-tools/"
    }
}

# 3. Check / Install Rustup & Cargo
$cargoBin = Join-Path $HOME ".cargo\bin"
if (Test-Path $cargoBin) {
    $env:PATH = "$cargoBin;$env:PATH"
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Warn "Rust / Cargo not found"
    if ($Check) {
        $Missing++
        Write-Warn "Remedy: winget install --id Rustlang.Rustup -e"
    } elseif ($DryRun) {
        Write-Host "  [dry-run] Install rustup-init.exe -y --default-toolchain $RequiredChannel"
    } else {
        Write-Info "Downloading and running rustup-init.exe (toolchain $RequiredChannel)..."
        $rustupInit = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit
        & $rustupInit -y --default-toolchain $RequiredChannel -c rustfmt -c clippy
        $env:PATH = "$cargoBin;$env:PATH"
    }
} else {
    $rustcVer = (& rustc --version)
    Write-Ok "Rust found ($rustcVer)"
    if (-not $Check -and (Get-Command rustup -ErrorAction SilentlyContinue)) {
        if ($DryRun) {
            Write-Host "  [dry-run] rustup toolchain install $RequiredChannel --component rustfmt --component clippy"
        } else {
            & rustup toolchain install $RequiredChannel --component rustfmt --component clippy
        }
    }
}

# 4. Optional Dev Tools
if ($WithDevTools) {
    foreach ($tool in @("cargo-nextest", "cargo-insta", "cargo-llvm-cov")) {
        if ($Check) {
            Write-Info "Checked optional dev tool: $tool"
        } elseif ($DryRun) {
            Write-Host "  [dry-run] cargo install --locked $tool"
        } else {
            Write-Info "Installing $tool..."
            & cargo install --locked $tool
        }
    }
}

if ($Check) {
    if ($Missing -gt 0) {
        Write-Err "Found $Missing missing requirement(s). Re-run with '.\scripts\setup.ps1 -Yes' to install."
        exit 1
    }
    Write-Ok "All Bilterm Windows build requirements are satisfied!"
    exit 0
}

if ($Build) {
    Write-Info "Building Bilterm release binary..."
    Push-Location $RepoRoot
    try {
        if ($DryRun) {
            Write-Host "  [dry-run] cargo build --release"
        } else {
            & cargo build --release
        }
    } finally {
        Pop-Location
    }
}

if ($Install) {
    $installScript = Join-Path $RepoRoot "scripts\install.ps1"
    $installParams = @{}
    if ($DryRun) { $installParams["DryRun"] = $true }
    if ($Prefix) { $installParams["Prefix"] = $Prefix }
    & $installScript @installParams
    exit 0
}

Write-Ok "Bilterm Windows development environment setup complete!"
Write-Host @"

Next steps:
  1. cargo build --release              # compile target\release\bilterm.exe
  2. .\scripts\install.ps1              # install bilterm.exe to user PATH
  3. bilterm                            # launch Bilterm TUI
"@

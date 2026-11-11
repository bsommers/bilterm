# ==============================================================================
# Bilterm - Plain Binary & Desktop Launcher Installer for Windows (PowerShell)
# ==============================================================================
# Installs `bilterm.exe`, registers `Win+R` (`App Paths\bilterm.exe`), adds the
# install directory to the User `PATH`, and creates Windows Start Menu shortcuts
# (`Bilterm.lnk` and `Bilterm (Native GUI).lnk`).
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
    [string]$Archive = "",
    [switch]$NoDesktop,
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
Bilterm Plain Binary & Desktop Launcher Installer (Windows PowerShell)

Usage:
  .\scripts\install.ps1 [-Local] [-Cargo] [-Prefix <DIR>] [-BinaryOnly] [-FromSource] [-Archive <ZIP>] [-NoDesktop] [-Uninstall] [-DryRun]

Options:
  -Local          Install to $HOME\.local\bin\bilterm.exe (default)
  -Cargo          Install to $HOME\.cargo\bin\bilterm.exe
  -Prefix <DIR>   Install to <DIR>\bin\bilterm.exe
  -BinaryOnly     Only download the prebuilt binary (never build; implied outside a checkout)
  -FromSource     Compile from source using Cargo (requires Rust)
  -Tag <TAG>      Release tag to download (default: latest)
  -Archive <ZIP>  Install from a local release archive (bilterm-<tag>-<triple>.zip)
  -NoDesktop      Skip Windows Run (App Paths) and Start Menu shortcut registration
  -Uninstall      Remove installed bilterm.exe, App Paths entry, and Start Menu shortcuts
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
    # (no `??` here: in command mode it is passed to Resolve-Path as a stray argument, so -Prefix always failed)
    $prefixPath = Resolve-Path -LiteralPath $Prefix -ErrorAction SilentlyContinue
    if (-not $prefixPath) { $prefixPath = $Prefix }
    $BinDir = Join-Path $prefixPath "bin"
} elseif ($Cargo) {
    $BinDir = Join-Path $HOME ".cargo\bin"
} else {
    $BinDir = Join-Path $HOME ".local\bin"
}

$TargetBin = Join-Path $BinDir "bilterm.exe"
$AppPathsRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\bilterm.exe"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$ShortcutTui = Join-Path $StartMenuDir "Bilterm.lnk"
$ShortcutGui = Join-Path $StartMenuDir "Bilterm (Native GUI).lnk"

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
    if (Test-Path $AppPathsRegKey) {
        if ($DryRun) {
            Write-Host "  [dry-run] Remove-Item $AppPathsRegKey"
        } else {
            Remove-Item -Path $AppPathsRegKey -Force -ErrorAction SilentlyContinue
        }
        Write-Ok "Removed Windows Run (App Paths) registration"
    }
    foreach ($lnk in @($ShortcutTui, $ShortcutGui)) {
        if (Test-Path $lnk) {
            if ($DryRun) {
                Write-Host "  [dry-run] Remove-Item $lnk"
            } else {
                Remove-Item -Force $lnk -ErrorAction SilentlyContinue
            }
            Write-Ok "Removed Start Menu shortcut: $lnk"
        }
    }
    exit 0
}

# Native OS architecture. $env:PROCESSOR_ARCHITECTURE reports AMD64 inside an
# x64-emulated PowerShell on Windows on ARM, so read the machine-wide value.
function Get-NativeArch {
    $a = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -ErrorAction SilentlyContinue).PROCESSOR_ARCHITECTURE
    if (-not $a) { $a = $env:PROCESSOR_ARCHITECTURE }
    switch ($a) { 'ARM64' { 'aarch64' } 'AMD64' { 'x86_64' } default { 'i686' } }
}

function Install-FromZip($zip, $dest, $label) {
    $tmpExtract = Join-Path $env:TEMP "bilterm-extract-$PID"
    try {
        Expand-Archive -Path $zip -DestinationPath $tmpExtract -Force
        $exeItem = Get-ChildItem -Path $tmpExtract -Filter "bilterm.exe" -Recurse | Select-Object -First 1
        if (-not $exeItem) { return $false }
        Copy-Item -Path $exeItem.FullName -Destination $dest -Force
        Write-Ok "Installed bilterm.exe from $label"
        return $true
    } finally {
        Remove-Item -Force -Recurse $tmpExtract -ErrorAction SilentlyContinue
    }
}

$ReleaseBaseUrl = if ($env:BILTERM_RELEASE_BASE_URL) { $env:BILTERM_RELEASE_BASE_URL } else { "https://github.com/$GithubRepo/releases" }
$ApiBaseUrl = if ($env:BILTERM_API_BASE_URL) { $env:BILTERM_API_BASE_URL } else { "https://api.github.com/repos/$GithubRepo" }
$ReleasesPage = "https://github.com/$GithubRepo/releases"

function Show-NoBinaryHelp($what) {
    Write-Err "No prebuilt bilterm binary is available for $what."
    Write-Err "Options:"
    Write-Err "  -Tag <TAG>    install a specific release that has a binary (see $ReleasesPage)"
    Write-Err "  -FromSource   build from source (requires Rust: https://rustup.rs)"
    Write-Err "  Releases page: $ReleasesPage"
}

# Download $url (asset $asset), verify against SHA256SUMS next to it, install.
# Returns $true on success, $false if the asset could not be downloaded.
function Fetch-AndInstall($asset, $url, $dest, $label) {
    $tmpZip = Join-Path $env:TEMP "bilterm-$PID-$asset"
    try {
        try {
            Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing -ErrorAction Stop
        } catch {
            return $false
        }
        $sumsUrl = $url.Substring(0, $url.LastIndexOf('/')) + "/SHA256SUMS"
        $sums = $null
        try { $sums = (Invoke-WebRequest -Uri $sumsUrl -UseBasicParsing -ErrorAction Stop).Content } catch {}
        if ($sums -is [byte[]]) { $sums = [System.Text.Encoding]::UTF8.GetString($sums) }
        if ($sums) {
            $expected = $null
            foreach ($line in ($sums -split "`n")) {
                $parts = $line.Trim() -split '\s+', 2
                if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $asset) { $expected = $parts[0]; break }
            }
            if ($expected) {
                $actual = (Get-FileHash -Algorithm SHA256 -Path $tmpZip).Hash
                if ($actual -ine $expected) {
                    Write-Err "SHA256 mismatch for ${asset}: expected $expected, got $actual. Refusing to install."
                    exit 1
                }
                Write-Ok "SHA256 verified for $asset"
            } else {
                Write-Warn "SHA256SUMS has no entry for $asset; skipping checksum verification."
            }
        } else {
            Write-Warn "No SHA256SUMS found at $sumsUrl; skipping checksum verification."
        }
        if (-not (Install-FromZip $tmpZip $dest $label)) {
            Write-Err "Archive $asset does not contain bilterm.exe."
            exit 1
        }
        return $true
    } finally {
        Remove-Item -Force $tmpZip -ErrorAction SilentlyContinue
    }
}

# Newest release (stable first, then prerelease) carrying $asset; $null if none.
function Find-ReleaseWithAsset($asset) {
    try {
        $rels = Invoke-RestMethod -Uri "$ApiBaseUrl/releases?per_page=30" -UseBasicParsing -ErrorAction Stop
    } catch { return $null }
    foreach ($wantPre in @($false, $true)) {
        foreach ($r in $rels) {
            if ([bool]$r.prerelease -ne $wantPre -or $r.draft) { continue }
            foreach ($a in $r.assets) {
                if ($a.name -eq $asset) {
                    return [pscustomobject]@{ Tag = $r.tag_name; Url = $a.browser_download_url; Pre = $wantPre }
                }
            }
        }
    }
    return $null
}

function Try-DownloadPrebuilt($dest) {
    $arch = Get-NativeArch
    # Windows on ARM runs x64 binaries under emulation: fall back if a release has no ARM64 build.
    $triples = @("$arch-pc-windows-msvc")
    if ($arch -eq "aarch64") { $triples += "x86_64-pc-windows-msvc" }
    foreach ($triple in $triples) {
        $asset = "bilterm-$triple.zip"
        $url = if ($Tag -eq "latest") {
            "$ReleaseBaseUrl/latest/download/$asset"
        } else {
            "$ReleaseBaseUrl/download/$Tag/$asset"
        }
        Write-Info "Checking for prebuilt binary at $url..."
        if ($DryRun) {
            Write-Host "  [dry-run] Invoke-WebRequest -Uri $url"
            return $true
        }
        if (Fetch-AndInstall $asset $url $dest "prebuilt release ($triple)") {
            if ($triple -ne "$arch-pc-windows-msvc") { Write-Warn "No $arch build in this release; installed $triple (runs under emulation)" }
            return $true
        }
        if ($Tag -ne "latest") {
            Write-Err "Release '$Tag' has no asset $asset ($url)."
            continue
        }
        Write-Info "Latest release has no $asset; looking for the newest release that does..."
        $hit = Find-ReleaseWithAsset $asset
        if ($hit) {
            $kind = if ($hit.Pre) { " (pre-release)" } else { "" }
            Write-Warn "The latest release has no prebuilt binary for $triple; installing $($hit.Tag)$kind instead."
            if (Fetch-AndInstall $asset $hit.Url $dest "prebuilt release $($hit.Tag) ($triple)") { return $true }
        }
    }
    Show-NoBinaryHelp ("Windows " + $arch)
    return $false
}

if (-not $DryRun) {
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
} else {
    Write-Host "  [dry-run] New-Item -ItemType Directory -Force -Path $BinDir"
}

$localBuild = Join-Path $RepoRoot "target\release\bilterm.exe"
$cargoToml = Join-Path $RepoRoot "Cargo.toml"

if ($Archive) {
    if (-not (Test-Path -LiteralPath $Archive)) { Write-Err "Archive not found: $Archive"; exit 1 }
    if ($DryRun) {
        Write-Host "  [dry-run] Expand $Archive -> $TargetBin"
    } elseif (-not (Install-FromZip $Archive $TargetBin $Archive)) {
        Write-Err "No bilterm.exe inside $Archive"
        exit 1
    }
} elseif ($BinaryOnly) {
    if (-not (Try-DownloadPrebuilt $TargetBin)) {
        exit 1
    }
} elseif (-not $FromSource -and (Test-Path $localBuild)) {
    Write-Info "Using existing local build at $localBuild"
    if ($DryRun) {
        Write-Host "  [dry-run] Copy-Item $localBuild $TargetBin"
    } else {
        Copy-Item -Path $localBuild -Destination $TargetBin -Force
    }
} elseif (-not $FromSource -and -not (Test-Path $cargoToml)) {
    # Standalone / piped install: prebuilt binary only, never an implicit source build.
    if (-not (Try-DownloadPrebuilt $TargetBin)) { exit 1 }
} else {
    $cargoBin = Join-Path $HOME ".cargo\bin"
    if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Cargo is not installed. Install Rust (https://rustup.rs) or run '.\scripts\setup.ps1 -Yes -Install'."
        exit 1
    }
    if (Test-Path $cargoToml) {
        Write-Info "Building release binary from $cargoToml..."
        Push-Location $RepoRoot
        try {
            if ($DryRun) {
                Write-Host "  [dry-run] cargo build -q --release"
            } else {
                & cargo build -q --release
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

if (-not $NoDesktop) {
    if ($DryRun) {
        Write-Host "  [dry-run] Register $AppPathsRegKey -> $TargetBin"
        Write-Host "  [dry-run] Create Start Menu shortcuts: $ShortcutTui, $ShortcutGui"
    } else {
        try {
            New-Item -Path $AppPathsRegKey -Force | Out-Null
            Set-ItemProperty -Path $AppPathsRegKey -Name "(Default)" -Value $TargetBin
            Set-ItemProperty -Path $AppPathsRegKey -Name "Path" -Value $BinDir
            Write-Ok "Registered Windows Run (Win+R -> bilterm) via App Paths"
        } catch {
            Write-Warn "Could not register App Paths key: $_"
        }

        try {
            if (Test-Path $StartMenuDir) {
                $wsh = New-Object -ComObject WScript.Shell
                $s1 = $wsh.CreateShortcut($ShortcutTui)
                $s1.TargetPath = $TargetBin
                $s1.WorkingDirectory = $HOME
                $s1.Description = "Bilterm - IBM 3270 x Borland Turbo Vision Composite Terminal Multiplexer"
                $s1.Save()

                $s2 = $wsh.CreateShortcut($ShortcutGui)
                $s2.TargetPath = $TargetBin
                $s2.Arguments = "--gui"
                $s2.WorkingDirectory = $HOME
                $s2.Description = "Bilterm (Native GUI Window)"
                $s2.Save()
                Write-Ok "Created Start Menu shortcuts: Bilterm and Bilterm (Native GUI)"
            }
        } catch {
            Write-Warn "Could not create Start Menu shortcuts: $_"
        }
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$BinDir*") {
    if ($DryRun) {
        Write-Host "  [dry-run] Add '$BinDir' to User PATH"
    } else {
        $newPath = if ([string]::IsNullOrEmpty($userPath)) { $BinDir } else { "$userPath;$BinDir" }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        $env:PATH = "$BinDir;$env:PATH"
        Write-Ok "Added '$BinDir' to User PATH"
    }
}

Write-Host @"

Next steps:
  1. Win+R -> bilterm                   # launch directly from Windows Run or Start Menu
  2. Win+R -> bilterm --gui             # launch standalone Native GUI window (no terminal needed)
  3. bilterm -h                         # view CLI switches & options
"@

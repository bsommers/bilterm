#!/usr/bin/env pwsh
# Windows entry point for the verification gate. Same contract as scripts/verify.sh on every OS:
# ONE output line (PASS n/n <secs>s | FAIL k/n: <stages> -> target/verify/<stage>/fail.txt),
# exit code 0 pass / 1 fail / 2 usage, details under target/verify/.
#
#   pwsh scripts/verify.ps1                 full gate
#   pwsh scripts/verify.ps1 --quick         clippy + unit tests
#   pwsh scripts/verify.ps1 --only t:smoke_binary_per_os
#   pwsh scripts/verify.ps1 --coverage      (delegates to scripts/coverage.sh)
#
# It is deliberately thin: the stage logic lives once, in verify.sh, and runs under the bash that
# ships with Git for Windows (no WSL, no MSYS2 install needed). On Linux/macOS it simply runs bash.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

function Find-Bash {
    # Prefer Git for Windows: `bash` on PATH may be the WSL launcher (C:\Windows\System32\bash.exe),
    # which runs in a different OS and would test the Linux build instead of the Windows one.
    $candidates = @()
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($git) {
        $gitRoot = Split-Path -Parent (Split-Path -Parent $git.Source)
        $candidates += (Join-Path $gitRoot 'bin/bash.exe'), (Join-Path $gitRoot 'usr/bin/bash.exe')
    }
    foreach ($base in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:LOCALAPPDATA)) {
        if ($base) { $candidates += (Join-Path $base 'Git/bin/bash.exe'), (Join-Path $base 'Programs/Git/bin/bash.exe') }
    }
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { return $c } }
    $onPath = Get-Command bash -ErrorAction SilentlyContinue
    if ($onPath -and ($onPath.Source -notmatch '(?i)\\Windows\\System32\\')) { return $onPath.Source }
    return $null
}

$bash = Find-Bash
if (-not $bash) {
    Write-Output 'FAIL bash not found: install Git for Windows (https://git-scm.com/download/win) to run the gate'
    exit 2
}

$script = 'scripts/verify.sh'
$rest = @($args)
if ($rest.Count -gt 0 -and $rest[0] -eq '--coverage') {
    $script = 'scripts/coverage.sh'
    $rest = @($rest | Select-Object -Skip 1)
}

Push-Location $repo
try {
    & $bash $script @rest
    exit $LASTEXITCODE
} finally {
    Pop-Location
}

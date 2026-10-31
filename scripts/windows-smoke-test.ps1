# ==============================================================================
# Bilterm - Windows smoke test for a release archive
# ==============================================================================
# Installs the archive with install.ps1 under Windows PowerShell 5.1 (the stock
# shell), then exercises the installed bilterm.exe: PE architecture, CLI, GUI
# frame render, canvas export, headless playbook, and the TCP JSON-RPC daemon.
# Used by scripts/build-windows-vm.sh (win-devvm VM) and CI.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\windows-smoke-test.ps1 -Archive <zip> [-ExpectArch aarch64|x86_64]
# Exit code = number of failed checks.
# ==============================================================================
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [string]$ExpectArch = ""
)
$ErrorActionPreference = "Stop"
$script:fails = 0
function Check($name, [scriptblock]$body) {
    try {
        $r = & $body
        if ($r -is [array]) { $r = $r[-1] }
        if ($r -eq $false) { throw "check returned false" }
        Write-Host "PASS  $name"
    } catch {
        Write-Host "FAIL  $name : $_" -ForegroundColor Red
        $script:fails++
    }
}

$repo = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $HOME ".local\bin\bilterm.exe"
$work = Join-Path $env:TEMP "bilterm-smoke-$PID"
New-Item -ItemType Directory -Force -Path $work | Out-Null

Check "install.ps1 -Archive (Windows PowerShell $($PSVersionTable.PSVersion))" {
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "install.ps1") -Archive $Archive -Force | Out-Host
    ($LASTEXITCODE -eq 0) -and (Test-Path $exe)
}

Check "PE machine type matches '$ExpectArch'" {
    $b = [IO.File]::ReadAllBytes($exe)
    $pe = [BitConverter]::ToInt32($b, 0x3C)
    $machine = [BitConverter]::ToUInt16($b, $pe + 4)
    $arch = switch ($machine) { 0xAA64 { "aarch64" } 0x8664 { "x86_64" } default { "0x{0:X4}" -f $machine } }
    Write-Host "      machine=$arch"
    (-not $ExpectArch) -or ($arch -eq $ExpectArch)
}

Check "bilterm --version" {
    $v = & $exe --version
    Write-Host "      $v"
    ($LASTEXITCODE -eq 0) -and ("$v" -match "bilterm")
}

Check "bilterm --help" { & $exe --help | Out-Null; $LASTEXITCODE -eq 0 }

Check "bilterm ls" { & $exe ls | Out-Host; $LASTEXITCODE -eq 0 }

Check "GUI frame render (--gui-dump PPM)" {
    $ppm = Join-Path $work "frame.ppm"
    & $exe --gui-dump $ppm
    $hdr = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($ppm), 0, 2)
    ($LASTEXITCODE -eq 0) -and ($hdr -eq "P6")
}

Check "canvas export (plot -o PNG)" {
    $png = Join-Path $work "plot.png"
    & $exe plot line "1,4,2,8,5" -o $png | Out-Host
    ($LASTEXITCODE -eq 0) -and ((Get-Item $png).Length -gt 0)
}

$pb = Join-Path $repo "starter-packs\03-robot-scripting-and-qa\playbook.toml"
if (Test-Path $pb) {
    Check "headless playbook (starter pack 03)" {
        $out = & $exe run $pb
        Write-Host "      $out"
        ($LASTEXITCODE -eq 0) -and ("$out" -match '"passed":true')
    }
}

Check "headless daemon + ctl workspace.status (TCP 127.0.0.1:7370, bearer token)" {
    $tok = Join-Path $work "token"
    Set-Content -Path $tok -Value ([guid]::NewGuid().ToString("N")) -NoNewline
    $p = Start-Process -FilePath $exe -ArgumentList "--headless", "--token-file", "`"$tok`"" -PassThru -WindowStyle Hidden
    try {
        $resp = $null
        for ($i = 0; $i -lt 20 -and -not $resp; $i++) {
            Start-Sleep -Milliseconds 500
            # ctl's first positional argument is the endpoint, so name it explicitly.
            try { $resp = & $exe ctl --token-file $tok 127.0.0.1:7370 workspace.status 2>$null } catch { }
        }
        Write-Host "      $resp"
        "$resp" -match "result"
    } finally {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    }
}

Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
Write-Host ""
if ($script:fails -eq 0) { Write-Host "smoke test: all checks passed" } else { Write-Host "smoke test: $script:fails check(s) failed" -ForegroundColor Red }
exit $script:fails

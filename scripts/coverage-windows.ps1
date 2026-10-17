$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$reportDirectory = Join-Path $projectRoot 'tmp'
New-Item -ItemType Directory -Force -Path $reportDirectory | Out-Null
Push-Location (Join-Path $projectRoot 'src-tauri')
try {
    # Match the existing business-logic gate. OS adapters are checked by native smoke;
    # capture bounds live in domain/notch.rs and remain inside this gate.
    $excludedPaths = '([\\/]tests[\\/])|src[\\/](main\.rs|lib\.rs|commands\.rs|infrastructure[\\/](window_ctl|updater|notch_window|topmost|liquid_glass)\.rs)'
    & cargo llvm-cov --lib --test integration_e2e --test risk_scenarios --test notch --test liquid_frames --json --summary-only --ignore-filename-regex $excludedPaths --fail-under-lines 85 --output-path (Join-Path $reportDirectory 'coverage-summary.json')
    if ($LASTEXITCODE -ne 0) { throw "Coverage command failed ($LASTEXITCODE)" }
    $report = Get-Content -Raw (Join-Path $reportDirectory 'coverage-summary.json') | ConvertFrom-Json
    Write-Output ("Business logic line coverage: {0:N2}% (gate: 85%)" -f $report.data[0].totals.lines.percent)
} finally { Pop-Location }

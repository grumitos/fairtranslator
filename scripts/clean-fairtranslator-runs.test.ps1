$ErrorActionPreference = "Stop"

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("fairtl-clean-test-" + [guid]::NewGuid())
$root = Join-Path $temp "runs"
$outside = Join-Path $temp "outside"
$junction = Join-Path $root "linked-run"
$script = Join-Path $PSScriptRoot "clean-fairtranslator-runs.ps1"

try {
    New-Item -ItemType Directory -Path $root, $outside | Out-Null
    Set-Content -LiteralPath (Join-Path $outside "sentinel.txt") -Value "keep"
    New-Item -ItemType Junction -Path $junction -Target $outside | Out-Null

    $rejected = $false
    try {
        & $script -Root $root -RemoveRun "linked-run" | Out-Null
    } catch {
        if ($_.Exception.Message -notmatch "reparse point") {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected) {
        throw "Expected cleanup to reject a reparse-point run"
    }

    if (-not (Test-Path -LiteralPath (Join-Path $outside "sentinel.txt"))) {
        throw "Cleanup touched data behind the reparse point"
    }

    Remove-Item -LiteralPath $junction -Force
    $run = Join-Path $root "regular-run"
    New-Item -ItemType Directory -Path $run | Out-Null
    Set-Content -LiteralPath (Join-Path $run "planner_cache.json") -Value "cache"
    Set-Content -LiteralPath (Join-Path $run "project.fairtl") -Value "project"

    & $script -Root $root -Apply | Out-Null
    if (Test-Path -LiteralPath (Join-Path $run "planner_cache.json")) {
        throw "Cleanup did not remove a regular transient file"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $run "project.fairtl"))) {
        throw "Cleanup removed a retained project file"
    }
} finally {
    if (Test-Path -LiteralPath $junction) {
        Remove-Item -LiteralPath $junction -Force
    }
    if (Test-Path -LiteralPath $temp) {
        Remove-Item -LiteralPath $temp -Recurse -Force
    }
}

Write-Output "clean-fairtranslator-runs reparse-point check passed"

param(
    [string]$Root = "D:\FairTranslatorRuns",
    [switch]$Apply,
    [int]$RetainDays = 14,
    [string[]]$KeepRun = @(),
    [string[]]$RemoveRun = @()
)

$ErrorActionPreference = "Stop"

function Get-TreeSizeBytes {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return 0L
    }

    $sum = (Get-SafeTreeFiles -Path $Path | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) {
        return 0L
    }
    return [int64]$sum
}

function Assert-NotReparsePoint {
    param([Parameter(Mandatory = $true)][System.IO.FileSystemInfo]$Item)

    if (($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Refusing to touch reparse point: $($Item.FullName)"
    }
}

function Get-SafeTreeFiles {
    param([Parameter(Mandatory = $true)][string]$Path)

    $item = Get-Item -LiteralPath $Path -Force
    Assert-NotReparsePoint -Item $item
    if (-not $item.PSIsContainer) {
        return $item
    }

    foreach ($child in (Get-ChildItem -LiteralPath $item.FullName -Force)) {
        Assert-NotReparsePoint -Item $child
        if ($child.PSIsContainer) {
            Get-SafeTreeFiles -Path $child.FullName
        } else {
            $child
        }
    }
}

function Resolve-UnderRoot {
    param(
        [Parameter(Mandatory = $true)][string]$RootPath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $resolvedRoot = (Resolve-Path -LiteralPath $RootPath).Path.TrimEnd("\")
    $resolvedPath = (Resolve-Path -LiteralPath $Path).Path
    $rootPrefix = "$resolvedRoot\"

    if ($resolvedPath -ne $resolvedRoot -and -not $resolvedPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to touch path outside '$resolvedRoot': $resolvedPath"
    }

    Assert-NotReparsePoint -Item (Get-Item -LiteralPath $resolvedPath -Force)

    return $resolvedPath
}

function Remove-SafeTree {
    param(
        [Parameter(Mandatory = $true)][string]$RootPath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $safePath = Resolve-UnderRoot -RootPath $RootPath -Path $Path
    $item = Get-Item -LiteralPath $safePath -Force
    Assert-NotReparsePoint -Item $item
    if ($item.PSIsContainer) {
        foreach ($child in (Get-ChildItem -LiteralPath $safePath -Force)) {
            Remove-SafeTree -RootPath $RootPath -Path $child.FullName
        }
    }
    Remove-Item -LiteralPath $safePath -Force
}

function Read-RunStatus {
    param([Parameter(Mandatory = $true)][string]$RunPath)

    $statusPath = Join-Path $RunPath "qa_translate.status.json"
    if (-not (Test-Path -LiteralPath $statusPath)) {
        return $null
    }

    try {
        return Get-Content -LiteralPath $statusPath -Raw | ConvertFrom-Json
    } catch {
        return $null
    }
}

if (-not (Test-Path -LiteralPath $Root)) {
    throw "Runs root does not exist: $Root"
}

$resolvedRoot = (Resolve-Path -LiteralPath $Root).Path.TrimEnd("\")
Assert-NotReparsePoint -Item (Get-Item -LiteralPath $resolvedRoot -Force)
$reportDir = Join-Path $resolvedRoot "_hygiene"
$now = Get-Date
$cutoff = $now.AddDays(-1 * $RetainDays)
$actions = @()

if (Test-Path -LiteralPath $reportDir) {
    Assert-NotReparsePoint -Item (Get-Item -LiteralPath $reportDir -Force)
} else {
    New-Item -ItemType Directory -Path $reportDir | Out-Null
}

$runDirs = Get-ChildItem -LiteralPath $resolvedRoot -Force -Directory |
    Where-Object { $_.Name -ne "_hygiene" }

foreach ($run in $runDirs) {
    $runPath = Resolve-UnderRoot -RootPath $resolvedRoot -Path $run.FullName
    $status = Read-RunStatus -RunPath $runPath
    $hasProject = Test-Path -LiteralPath (Join-Path $runPath "project.fairtl")
    $hasExport = Test-Path -LiteralPath (Join-Path $runPath "export-es")
    $hasPlayable = Test-Path -LiteralPath (Join-Path $runPath "playable-es")
    $isCompleted = $status -and $status.phase -eq "completed" -and [int]$status.unresolvedEntries -eq 0
    $keepExplicit = $KeepRun -contains $run.Name
    $removeExplicit = $RemoveRun -contains $run.Name

    if ($keepExplicit) {
        continue
    }

    $shouldRemove =
        $removeExplicit -or
        (
            $run.LastWriteTime -lt $cutoff -and
            -not $hasExport -and
            -not $hasPlayable -and
            -not $isCompleted
        )

    if ($shouldRemove) {
        $reason = if ($removeExplicit) { "explicit_remove_run" } else { "stale_incomplete_run" }
        $actions += [pscustomobject]@{
            Type = "dir"
            Path = $runPath
            SizeBytes = Get-TreeSizeBytes -Path $runPath
            Reason = $reason
        }
        continue
    }

    $runFiles = @(Get-SafeTreeFiles -Path $runPath)
    $transientFiles = @($runFiles |
        Where-Object {
            $_.DirectoryName -eq $runPath -and
            (
                $_.Name -eq "planner_cache.json" -or
                $_.Name -like "before-*" -or
                $_.Name -like "runner.*.log" -or
                $_.Name -like "run-jpleft-*.ps1"
            )
        })

    $reviewDir = Join-Path $runPath "review-gpt55"
    if (Test-Path -LiteralPath $reviewDir) {
        $transientFiles += @($runFiles |
            Where-Object {
                $_.DirectoryName -eq $reviewDir -and
                (
                    $_.Name -like "gpt55_final_review_stdout*.log" -or
                    $_.Name -like "gpt55_final_review_stderr*.log"
                )
            })
    }

    $transientFiles += @($runFiles | Where-Object { $_.Name -eq "debug.log" })

    foreach ($file in ($transientFiles | Sort-Object FullName -Unique)) {
        $filePath = Resolve-UnderRoot -RootPath $resolvedRoot -Path $file.FullName
        $actions += [pscustomobject]@{
            Type = "file"
            Path = $filePath
            SizeBytes = Get-TreeSizeBytes -Path $filePath
            Reason = "regenerable_cache_or_transient_log"
        }
    }
}

$beforeBytes = Get-TreeSizeBytes -Path $resolvedRoot
if ($Apply) {
    foreach ($action in ($actions | Sort-Object Type, Path -Descending)) {
        if (Test-Path -LiteralPath $action.Path) {
            Remove-SafeTree -RootPath $resolvedRoot -Path $action.Path
        }
    }
}
$afterBytes = Get-TreeSizeBytes -Path $resolvedRoot

$remainingRuns = Get-ChildItem -LiteralPath $resolvedRoot -Force -Directory |
    Where-Object { $_.Name -ne "_hygiene" } |
    ForEach-Object {
        $size = Get-TreeSizeBytes -Path $_.FullName
        [pscustomobject]@{
            Name = $_.Name
            SizeMB = [math]::Round($size / 1MB, 2)
            Files = @(Get-SafeTreeFiles -Path $_.FullName).Count
            LastWriteTime = $_.LastWriteTime.ToString("s")
        }
    } |
    Sort-Object LastWriteTime -Descending

$report = [pscustomobject]@{
    CreatedAt = $now.ToString("s")
    Root = $resolvedRoot
    Mode = if ($Apply) { "apply" } else { "dry-run" }
    RetainDays = $RetainDays
    PlannedActionCount = $actions.Count
    BeforeMB = [math]::Round($beforeBytes / 1MB, 2)
    AfterMB = [math]::Round($afterBytes / 1MB, 2)
    ReclaimableOrReclaimedMB = if ($Apply) {
        [math]::Round(($beforeBytes - $afterBytes) / 1MB, 2)
    } else {
        [math]::Round((($actions | Measure-Object -Property SizeBytes -Sum).Sum) / 1MB, 2)
    }
    Actions = $actions | ForEach-Object {
        [pscustomobject]@{
            Type = $_.Type
            Path = $_.Path
            SizeMB = [math]::Round($_.SizeBytes / 1MB, 2)
            Reason = $_.Reason
        }
    }
    RetainedRuns = $remainingRuns
}

$reportPath = Join-Path $reportDir ("hygiene-{0}.json" -f $now.ToString("yyyyMMdd-HHmmss"))
$report | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $reportPath -Encoding UTF8
$report | ConvertTo-Json -Depth 7

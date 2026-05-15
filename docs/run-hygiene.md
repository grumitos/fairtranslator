# FairTranslator Run Hygiene

Long translation campaigns write checkpointed artifacts outside the repository,
usually under `D:\FairTranslatorRuns`. Keep that folder usable by preserving the
artifacts needed for resume/review and pruning generated caches or aborted probes.

## Keep

- `project.fairtl`
- `qa_translate.status.json`
- `qa_translate.progress.log`
- `translation_failures.json`, when present
- `export-es`
- `playable-es`
- final review reports such as `review-gpt55\final_review_report.md`

## Prune

- `planner_cache.json`
- `before-*` checkpoint logs/status snapshots
- `runner.*.log`
- `run-jpleft-*.ps1`
- recursive `debug.log` files inside run folders
- temporary `stdout`/`stderr` logs from final review reruns
- aborted/probe run folders older than the retention window with no export, no
  playable copy, and no completed status

## Script

Dry run:

```powershell
.\scripts\clean-fairtranslator-runs.ps1
```

Apply the default cleanup:

```powershell
.\scripts\clean-fairtranslator-runs.ps1 -Apply
```

Use a different runs root or retention window:

```powershell
.\scripts\clean-fairtranslator-runs.ps1 -Root D:\FairTranslatorRuns -RetainDays 30
```

Remove known superseded runs while keeping a specific final run:

```powershell
.\scripts\clean-fairtranslator-runs.ps1 `
  -Apply `
  -KeepRun rj01568532-spark-20260430-231617 `
  -RemoveRun rj01568532-codex-20260430-231158,my-summer-start-es
```

Every run writes a JSON report to `D:\FairTranslatorRuns\_hygiene`.

# Audit TODO

This file is the live audit queue. Closed implementation batches were compacted
out of the old TODO wall on 2026-05-14; keep new entries here only when they are
still actionable.

## Current Status - 2026-05-14

- Version alignment: `0.9.2` in `package.json`, `src-tauri/Cargo.toml`, and
  `src-tauri/tauri.conf.json`.
- Last full hardening baseline: 2026-04-25, recorded in
  `docs/AUDIT_SESSION_MASTER.md`.
- Current campaign QA: Buebito export has about `~49 min` documented runtime
  coverage; one-hour target is still open.
- Ignored local QA context exists in `QA_TRACKER.md`; it remains local operator
  context, not tracked release documentation.

## Active Queue

| ID | Status | Area | Close When | Evidence Needed | Risk |
| --- | --- | --- | --- | --- | --- |
| DEBT-01 | todo | `src-tauri/src/lib.rs`, `src-tauri/src/commands/*` | command-layer tests can run locally/CI without the current Windows harness crash | command-focused Rust tests executing successfully | command glue can drift from tested service logic |
| QA-01 | doing | Buebito campaign, `QA_TRACKER.md`, QA bins, translation services, batch UI, `docs/PLAYTEST_BUEBITO_LOG.md` | the corrected Buebito campaign is fully translated, exported, reopened, and smoke-tested for one hour with detailed notes | generated artifacts, reports, screenshots when useful, timestamped playtest log, contextual plan dump when relevant | app can pass automated checks but fail in real games |
| QA-04 | todo | map `63` custom memory menus/descriptions | Hana/Hina/Rina recall menu entries and lower descriptions render fully in Spanish in the live export | runtime screenshots after fix | advanced replay/content-selection UI can remain mixed-language |
| QA-05 | todo | map `63` helper event `ev_説明` | the inspected Cyrillic token `хотя` is visually disproven or reproduced, source-localized, fixed, and revalidated | screenshot or event/runtime evidence for the visible line | memory-hub helper text may contain non-Spanish garbage |
| QA-06 | todo | map `63` -> `61` Mama recall branch | the black-screen Mama lines read naturally in Spanish in live runtime or are intentionally retained with rationale | follow-up runtime screenshots and text notes | structurally stable scenes can still read unpolished where text is the full focus |

## Closed Baseline Summary

Do not reopen these without fresh evidence:

- `SEC-01`, `SEC-02`: secure API-key storage, legacy migration, endpoint
  boundary checks, explicit CSP, and high-severity npm audit cleanup.
- `UX-01`: descriptions and accessible names for the core dialog/settings/API
  key surfaces.
- `PERF-01`, `PERF-02`: Vite chunking, stale generated config cleanup, active
  file grid/find optimization, deferred search, memoized stats, and resize
  persistence on drag end.
- `ENV-01`: local Rust/Tauri validation works through the Visual Studio Build
  Tools environment.
- `P1-01` through `P1-06`, `P2-01` through `P2-08`, `P3-01`, `P3-02`: prompt
  budgeting, style-guide persistence, dirty-state protection, translation
  integrity, schema versioning, progress semantics, provider copy, Google
  safeguards, compact prompts, QA runner robustness, plugin text export,
  editing latency, and hot-path recomputation were closed by prior batches.
- `QA-02`, `QA-03`: long-run isolated-entry recovery and safe rerun export
  regeneration are implemented in the QA runner.

# Audit Session Master

This file is the compact audit handoff for FairTranslator. It intentionally no
longer stores the full per-batch implementation log: that history had become a
duplicate of release notes, changelog entries, test output, and
`docs/AUDIT_TODO.md`.

## Current Verified State - 2026-05-14

- Repository version is aligned on `0.9.2` in `package.json`,
  `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
- The current product promise remains: desktop translator for RPG Maker MZ, MV,
  VX Ace, VX, and XP. Do not broaden docs or marketing language beyond that
  support boundary without new engine adapters and QA evidence.
- Latest release notes are in `docs/releases/v0.9.2.md`; older release notes and
  `CHANGELOG.md` remain durable release history and should not be deleted as
  cleanup.
- The Buebito runtime playtest log records about `~49 min` of wall-clock
  coverage. The one-hour target is still open.
- The latest full local hardening baseline is the 2026-04-25 validation pass
  listed below. Later Buebito notes are targeted campaign QA, not a replacement
  for the full hardening baseline.

## Active Follow-up

Use `docs/AUDIT_TODO.md` as the live queue. As of this cleanup pass, the active
items are:

- `DEBT-01`: command-layer Tauri/Rust tests are still not directly executable in
  the current Windows harness without `STATUS_ENTRYPOINT_NOT_FOUND`.
- `QA-01`: real-game validation must continue until the Buebito campaign reaches
  the documented one-hour smoke target and the remaining runtime issues are
  triaged.
- `QA-04`: map `63` memory menus still show English labels in the live export.
- `QA-05`: the inspected `ev_説明` helper line with Cyrillic `хотя` still needs
  on-screen reproduction or disproof.
- `QA-06`: the map `63` -> `61` Mama branch remains structurally stable but has
  visibly weak literal Spanish in black-screen dialogue.

## Current Runtime QA Evidence

- Main runtime log: `docs/PLAYTEST_BUEBITO_LOG.md`.
- Detailed Mama route note: `docs/PLAYTEST_BUEBITO_RUNPLAY_MAMA_MAP61.md`.
- Screenshot-derived translation review notes:
  `docs/PLAYTEST_BUEBITO_RELATO_SS_20260411.md`.
- Local ignored QA campaign inventory: `QA_TRACKER.md`. It is intentionally
  ignored and points at local game roots outside the repo, so treat it as local
  operator context rather than public project documentation.

## Closed Workstreams Summary

The old audit stream recorded these closed workstreams. Reopen them only with
fresh code/config/test evidence:

- Security: provider API keys moved to Windows Credential Manager via
  `keyring`; non-secret provider settings remain in `tauri-plugin-store`; legacy
  JSON API keys migrate on load.
- Endpoint and desktop hardening: explicit Tauri CSP, HTTPS-only provider
  endpoint overrides, no credentials/query/fragment in endpoint override URLs.
- Persistence: `.fairtl` schema is versioned at `2`; schema `1` projects without
  `styleGuide` still load with an empty style guide.
- Project safety: dirty-state tracking and discard confirmation protect
  destructive open/create flows.
- Translation planning: OpenAI, OpenCode, Codex, and External AI share prompt-budgeted
  planning with compact prompt serialization and response remapping.
- Translation integrity: provider apply, external-AI apply, and export preflight
  share validation for control codes, placeholders, line drift, blank
  translations, and compact-field heuristics.
- Export: MV/MZ export includes `js/plugins.js`; the exporter restores copied
  asset references and returns warning details.
- Provider disclosure: UI copy now distinguishes OpenAI/OpenCode/Codex full contextual
  orchestration from DeepL/Google best-effort/basic MT paths.
- Google hardening: glossary/control-code/newline protection is boundary-aware
  instead of naive substring replacement.
- QA runner: long Codex runs have checkpoint/resume status, planner cache,
  isolated-entry recovery, failure manifests, and safe `export-es` regeneration.
- Frontend quality: core dialogs/settings have accessibility descriptions and
  labels; active-file grid/find/search and resize hot paths were reduced.
- Packaging/build: production Vite output is split into stable chunks, stale
  generated Vite config artifacts were removed from source control, and
  Windows/Tauri build validation succeeded locally after Build Tools setup.

## Validation Baseline - 2026-04-25

Commands recorded as passing in the last full hardening baseline:

```bash
npx tsc --noEmit
npm run test:run
npm run build
npm audit --audit-level=high
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --no-fail-fast
npm run tauri -- build
```

Earlier targeted validation included focused Rust tests for
`translation_orchestrator`, `compact_prompt`, `translation_integrity`,
`translation_apply`, `ai_batch`, `exporter`, `google`, `codex`, `openai`,
`persistence`, `rpgmaker`, `qa_translate_provider`, and `qa_dump_plan`, plus
frontend tests around project state and translation hooks.

## Next Exact Step

Resume the Buebito export from the current map-`61` Mama CG branch after
selecting `Con fuerza.`, finish that memory until control returns to the
map-`63` hub, reproduce or disprove the `ev_説明` helper line with `хотя`, then
check the remaining sister recall menu to determine whether the English-label
defect affects the full trio or only the already observed branches.

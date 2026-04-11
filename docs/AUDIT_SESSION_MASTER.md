# Audit Session Master

## Session Objective
- Audit, strengthen, and verify FairTranslator as a coherent product across manual, AI, and mixed translation workflows.
- Revalidate prior known findings, identify new product-integrity gaps, implement fixes in controlled batches, verify behavior, and leave a durable audit trail for context recovery.

## Full Scope
- Project lifecycle: create, open, save, reopen, backward compatibility.
- Translation pipeline: batching, chunking, context budgeting, progress reporting, provider parity, external AI round-trip.
- Editing workflow: manual edits, mixed/manual+AI usage, latency, dirty state, warnings, UI truthfulness.
- Export and QA: control codes, line breaks, RPG heuristics, export safety, playtest readiness.
- Verification and documentation: tests, manual validation, intermediate/final audits, residual risks.

## Initial Context
- Current date: 2026-04-11
- Workspace: `D:\Dev\fairtranslator`
- Worktree status at session start: dirty
- Existing modified files were detected before this audit started. They will be treated as pre-existing until individually reviewed.

## Confirmed Findings
- `P1` Prompt budgeting is still not real for LLM-backed flows. `translation_orchestrator` splits LLM batches using only `text + file_context + notes`, but the actual prompts also add style guide, glossary, protected terms, translation examples, file-context hints, metadata, and JSON envelope. Confirmed in [src-tauri/src/services/translation_orchestrator.rs](D:/Dev/fairtranslator/src-tauri/src/services/translation_orchestrator.rs:32), [src-tauri/src/services/translation_orchestrator.rs](D:/Dev/fairtranslator/src-tauri/src/services/translation_orchestrator.rs:278), [src-tauri/src/services/openai.rs](D:/Dev/fairtranslator/src-tauri/src/services/openai.rs:69), [src-tauri/src/services/codex.rs](D:/Dev/fairtranslator/src-tauri/src/services/codex.rs:125), and external AI prompt/export code in [src-tauri/src/services/ai_batch.rs](D:/Dev/fairtranslator/src-tauri/src/services/ai_batch.rs:121) and [src-tauri/src/services/ai_batch.rs](D:/Dev/fairtranslator/src-tauri/src/services/ai_batch.rs:185).
- `P1` The style guide "Save rules" flow still does not persist to disk even though the UI reports success. The backend command only mutates in-memory project state and updates `updated_at`; it never calls project persistence. Confirmed in [src-tauri/src/commands/style_guide.rs](D:/Dev/fairtranslator/src-tauri/src/commands/style_guide.rs:32), [src/components/layout/StyleGuidePanel.tsx](D:/Dev/fairtranslator/src/components/layout/StyleGuidePanel.tsx:86), and [src/hooks/useStyleGuide.ts](D:/Dev/fairtranslator/src/hooks/useStyleGuide.ts:66).
- `P1` The app still has no explicit dirty-state/discard guard. Opening or creating another project replaces the current in-memory project without any unsaved-changes confirmation, so manual edits, glossary changes, style-guide edits, or AI-applied drafts can be lost silently. Confirmed in [src/stores/projectStore.ts](D:/Dev/fairtranslator/src/stores/projectStore.ts:5), [src/components/layout/Toolbar.tsx](D:/Dev/fairtranslator/src/components/layout/Toolbar.tsx:61), [src/components/layout/NewProjectDialog.tsx](D:/Dev/fairtranslator/src/components/layout/NewProjectDialog.tsx:64), and [src/hooks/useProject.ts](D:/Dev/fairtranslator/src/hooks/useProject.ts:21).
- `P2` `.fairtl` schema version was not bumped after introducing `style_guide`. `style_guide` is serialized/deserialized on disk, but `SCHEMA_VERSION` remains `1`, so cross-version compatibility is undefined and older builds can silently resave without the new field contract being versioned. Confirmed in [src-tauri/src/services/persistence.rs](D:/Dev/fairtranslator/src-tauri/src/services/persistence.rs:12), [src-tauri/src/services/persistence.rs](D:/Dev/fairtranslator/src-tauri/src/services/persistence.rs:22), and [src-tauri/src/services/persistence.rs](D:/Dev/fairtranslator/src-tauri/src/services/persistence.rs:73).
- `P2` Translation progress still treats chunks as files and emits duplicate progress events around every translated chunk. `total_files` is derived from planned chunks instead of unique files, then two progress events are emitted per chunk boundary. Confirmed in [src-tauri/src/commands/translation.rs](D:/Dev/fairtranslator/src-tauri/src/commands/translation.rs:525), [src-tauri/src/commands/translation.rs](D:/Dev/fairtranslator/src-tauri/src/commands/translation.rs:653), and [src-tauri/src/commands/translation.rs](D:/Dev/fairtranslator/src-tauri/src/commands/translation.rs:734).
- `P2` Term hints and translation examples are truncated too early at the global collection stage, before per-chunk relevance selection. That can drop the actually relevant memory for later chunks/files. Confirmed in [src-tauri/src/services/translation_orchestrator.rs](D:/Dev/fairtranslator/src-tauri/src/services/translation_orchestrator.rs:227) and [src-tauri/src/services/translation_orchestrator.rs](D:/Dev/fairtranslator/src-tauri/src/services/translation_orchestrator.rs:271).
- `P2` Google remains outside the contextual provider contract. The provider accepts `TranslationBatchContext` but discards it entirely, while the UI copy claims internal orchestration and project memory at the provider-tab level. Confirmed in [src-tauri/src/services/google.rs](D:/Dev/fairtranslator/src-tauri/src/services/google.rs:200), [src/locales/en/editor.json](D:/Dev/fairtranslator/src/locales/en/editor.json:86), and [src/locales/es/editor.json](D:/Dev/fairtranslator/src/locales/es/editor.json:86).
- `P2` External AI export is materially weaker than the in-app LLM path. It lacks real prompt budgeting, does not include contextual term/example selection, and chunks only by entry count. Confirmed in [src-tauri/src/services/ai_batch.rs](D:/Dev/fairtranslator/src-tauri/src/services/ai_batch.rs:121) and [src-tauri/src/services/ai_batch.rs](D:/Dev/fairtranslator/src-tauri/src/services/ai_batch.rs:195).
- `P1` There is no product-level text-integrity guard before provider/apply/export writes translated text back into the project or output game. Provider flow and external AI apply translated text directly, and exporter silently reshapes multiline text when line counts drift. Confirmed in [src-tauri/src/commands/translation.rs](D:/Dev/fairtranslator/src-tauri/src/commands/translation.rs:491), [src-tauri/src/services/ai_batch.rs](D:/Dev/fairtranslator/src-tauri/src/services/ai_batch.rs:354), [src-tauri/src/services/exporter.rs](D:/Dev/fairtranslator/src-tauri/src/services/exporter.rs:1231), and [src-tauri/src/services/exporter.rs](D:/Dev/fairtranslator/src-tauri/src/services/exporter.rs:756).
- `P2` Google glossary protection is currently naive substring replacement, which can corrupt unrelated words containing the protected term. Confirmed in [src-tauri/src/services/google.rs](D:/Dev/fairtranslator/src-tauri/src/services/google.rs:46) and [src-tauri/src/services/google.rs](D:/Dev/fairtranslator/src-tauri/src/services/google.rs:72).
- `P2` Export still treats whitespace-only draft/reviewed/final translations as exportable content because `collect_translations` checks `!entry.translation.is_empty()` instead of trimming. Confirmed in [src-tauri/src/services/exporter.rs](D:/Dev/fairtranslator/src-tauri/src/services/exporter.rs:354).
- `P2` Manual editing responsiveness is weakened by artificial latency and full-project recomputation. Inline edit waits `200ms` before entering edit mode; edits close before optimistic state is shown; replace-all applies N sequential IPC updates; sidebar/status/grid recompute against the full project after each commit. Confirmed in [src/components/editor/translation-grid/TranslationGridTableView.tsx](D:/Dev/fairtranslator/src/components/editor/translation-grid/TranslationGridTableView.tsx:119), [src/components/editor/translation-grid/useInlineEdit.ts](D:/Dev/fairtranslator/src/components/editor/translation-grid/useInlineEdit.ts:21), [src/hooks/useProject.ts](D:/Dev/fairtranslator/src/hooks/useProject.ts:78), [src/components/layout/Sidebar.tsx](D:/Dev/fairtranslator/src/components/layout/Sidebar.tsx:44), and [src/components/layout/StatusBar.tsx](D:/Dev/fairtranslator/src/components/layout/StatusBar.tsx:39).

## Prioritized Backlog
- `P1-01` Real prompt budgeting and chunk planning across OpenAI, Codex, and External AI, including style guide, glossary, term hints, translation examples, file-context hints, metadata, JSON envelope, and response headroom.
- `P1-02` Make style-guide save truthful: persist to disk or fail loudly; add backward-safe save/load tests.
- `P1-03` Add real dirty-state tracking and discard protection so project mutations cannot be lost silently on open/create flow changes.
- `P1-04` Add a text-integrity guard for provider/apply/export paths covering control codes, placeholders, line breaks, and multiline drift with actionable warnings or blocking where needed.
- `P2-01` Bump `.fairtl` schema and add explicit backward-compat coverage for pre-style-guide projects.
- `P2-02` Repair progress contract: file counts must be real files, chunk counts explicit, duplicate events removed, empty batch path honest.
- `P2-03` Remove premature truncation of term hints/examples and make per-chunk relevance selection budget-aware.
- `P2-04` Align provider messaging with real capabilities; either improve provider context handling where possible or narrow UI promises per provider.
- `P2-05` Tighten provider-specific safeguards where weak implementations remain, especially Google glossary protection and provider-capability disclosure.
- `P3-01` Remove artificial inline-edit latency; make edits optimistic or visibly saving; collapse replace-all into a bulk path.
- `P3-02` Reduce global recomputation around manual editing and search/filter flows where it materially affects UX.

## Architectural Decisions
- Session memory will be persisted in this file plus `docs/AUDIT_TODO.md`.
- No critical product logic will be edited before baseline audit and prioritized backlog are written.
- Existing unrelated changes must not be reverted.
- The first implementation batch will target product-integrity failures before UX polish: budgeting/context integrity, truthful persistence, and progress semantics.
- Existing dirty changes will be treated as the current working baseline. Fixes will integrate with them rather than attempt to restore a previous tree state.

## Verifications Executed
- 2026-04-11: inspected repo root, key manifests, and git dirty state.
- 2026-04-11: traced current code paths for batch translation, provider prompts, external AI export/apply, project save/load, style guide CRUD, editor/manual editing, exporter, and QA helpers.
- 2026-04-11: reviewed QA inventory and sample roots in `QA_TRACKER.md`; local real-game campaign inputs exist for later validation.
- 2026-04-11: revalidated the intended removable-disk campaign target at `G:\Leyo\Buebito\My Summer with Onee-chans (ぼくと夏のおねえちゃん達) v1.1 - Okeyutei`; inventory reports RPG Maker `Mz` with `42` files and `6008` entries, and `package.json` confirms it is the correct NW.js game root.
- 2026-04-11: confirmed the real-campaign helper path `qa_translate_provider` uses `CodexProvider` only, with report output hardcoded as `provider: "codex"` and default model `gpt-5.4`; the first long run against `G:\...Buebito...` was interrupted manually before completion and must be rerun.
- 2026-04-11: re-audited remaining perf/debt items with read-only explorers:
  - `P3-01` is effectively resolved after immediate inline editing, optimistic single-entry saves, and batched replace-all IPC/backend application.
  - `P3-02` is only partially improved; deferred search and memoized stats landed, but project-wide recomputation on committed edits and column-resize persistence churn remain.
  - `P2-06` is now closed on code re-audit: unshrinkable oversize chunks fail fast before provider execution and External AI export; the remaining debt is only that the ceiling is conservative and glossary relevance is not yet trimmed per chunk.
- 2026-04-11: upgraded `qa_translate_provider` for long-running campaign visibility by adding startup/progress logging and a dedicated `qa_translate.progress.log` file inside the workspace so background runs can be monitored without looking hung.
- 2026-04-11: started the real removable-disk Buebito campaign translation in `D:\FairTranslatorRuns\buebito-codex-20260411-spark` using `gpt-5.3-codex-spark`; the live progress log confirms runtime validation completed and contextual planning has begun.
- 2026-04-11: extended the Codex runtime integration and QA helper with explicit `reasoning_effort` passthrough, workspace checkpoint saves, and auto-resume from `workspace/project.fairtl`.
- 2026-04-11: validated the new recovery flow on the real Buebito campaign:
  - first `spark-low` run created `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\project.fairtl` immediately and checkpointed after each completed chunk,
  - the run was intentionally interrupted,
  - relaunching the same workspace reported `resume=si` and `ya_traducidas=56`,
  - the remaining workload dropped from `6008/448` to `5952/444`, proving resume continued from persisted project state instead of restarting from zero.
- 2026-04-11: revalidated the Codex path after adding explicit reasoning support with:
  - `cargo test --manifest-path src-tauri/Cargo.toml codex -- --nocapture`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
- 2026-04-11: added a sidecar plan-dump helper `qa_dump_plan` and generated the current contextual plan snapshot at `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\qa_translate.plan.json` so the live campaign plan can be inspected outside the running process.
- 2026-04-11: upgraded `qa_translate_provider` with automatic fallback config (`--fallback-model`, `--fallback-profile`, `--fallback-reasoning-effort`) that switches models inside the same checkpointed workspace if the primary Codex run fails.
- 2026-04-11: rebuilt and relaunched the real Buebito campaign with automatic fallback enabled:
  - primary: `gpt-5.3-codex-spark` + `reasoning=low`
  - fallback: `gpt-5.4-mini` + `reasoning=low`
  - workspace: `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low`
  - resume state at launch: `ya_traducidas=729`
- 2026-04-11: ran `cargo fmt --manifest-path src-tauri/Cargo.toml`.
- 2026-04-11: ran targeted Rust tests for batch-A changes:
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_orchestrator -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml openai -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml codex -- --nocapture`
- 2026-04-11: ran `cargo check --manifest-path src-tauri/Cargo.toml` after batch-B persistence and dirty-state changes.
- 2026-04-11: reran targeted Rust tests after batch-B changes:
  - `cargo test --manifest-path src-tauri/Cargo.toml persistence -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_orchestrator -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml codex -- --nocapture`
- 2026-04-11: ran targeted frontend tests:
  - `npm run test:run -- src/__tests__/projectStore.test.ts src/__tests__/useTranslation.test.tsx`
- 2026-04-11: ran frontend production validation with `npm run build`.
- 2026-04-11: ran targeted Rust tests for batch-D integrity work:
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_integrity -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_apply -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml exporter -- --nocapture`
- 2026-04-11: reran backend/frontend validation after batch-D and QA-helper alignment:
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run test:run -- src/__tests__/projectStore.test.ts src/__tests__/useTranslation.test.tsx`
  - `npm run build`
- 2026-04-11: ran provider-hardening validation for Google safeguards:
  - `cargo test --manifest-path src-tauri/Cargo.toml google -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_integrity -- --nocapture`
  - `cargo check --manifest-path src-tauri/Cargo.toml`
- 2026-04-11: implemented and validated compact prompt serialization + parser remapping:
  - new shared module `src-tauri/src/services/compact_prompt.rs`
  - compact structured payloads for `Codex`, `OpenAI`, and `External AI`
  - local chunk IDs (`k`) remapped back to real `entryId`/`filePath` before apply
  - control-code masking/restoration plus multiline normalization before validation/apply
  - verification:
    - `cargo test --manifest-path src-tauri/Cargo.toml compact_prompt -- --nocapture`
    - `cargo test --manifest-path src-tauri/Cargo.toml codex -- --nocapture`
    - `cargo test --manifest-path src-tauri/Cargo.toml openai -- --nocapture`
    - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
- 2026-04-11: implemented chunk/planning upgrades oriented to larger, steadier LLM batches:
  - raised LLM chunk ceiling from `14` to `24` entries, still gated by real prompt budget
  - added compact previous/next chunk hints into `TranslationBatchContext`
  - precomputed `PreparedTranslationExample` lexical terms and top-k example selection without sorting the full translation-memory set on every chunk
  - stored `estimated_prompt_tokens` per planned chunk instead of recomputing budget checks later
  - verification:
    - `cargo test --manifest-path src-tauri/Cargo.toml translation_orchestrator -- --nocapture`
- 2026-04-11: implemented QA-runner robustness upgrades in `qa_translate_provider`:
  - `planner_cache.json` sidecar with conservative fingerprint invalidation
  - `qa_translate.status.json` sidecar with planning/translation phase, current file/chunk, retries, unresolved entries, and active fallback policy
  - `translation_failures.json` manifest for unresolved entries after isolation attempts
  - chunk retry exhaustion no longer hard-fails immediately; it now:
    - restores the file snapshot,
    - applies the individually valid subset,
    - retries the remaining entries as isolated single-entry operations,
    - leaves only persistent failures in the manifest
  - export is blocked when unresolved manifest entries remain
  - verification:
    - `cargo check --manifest-path src-tauri/Cargo.toml`
    - `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
    - `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_dump_plan`
- 2026-04-11: revalidated frontend/build wiring after `External AI` compact-response mapping changes with:
  - `npm run build`
- 2026-04-11: relaunched the real Buebito campaign with the upgraded runner and confirmed the new runtime behavior in production:
  - workspace: `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low`
  - target: `G:\Leyo\Buebito\My Summer with Onee-chans (ぼくと夏のおねえちゃん達) v1.1 - Okeyutei`
  - provider: `gpt-5.3-codex-spark` + `reasoning=low`
  - fallback policy stayed `usage_limit_only`; fallback is configured but not eligible unless Codex returns an explicit quota/usage-limit error
  - planning sidecar/log confirmed a materially denser plan: `4482` pending entries into `205` chunks instead of the earlier ~`363`-chunk shape
  - first live translation chunks are now running at `items=24`, proving the compact payload + larger-chunk planner changes are active in the real campaign
  - `planner_cache.json` is now being written in the workspace and `qa_translate.status.json` is exposing planning vs translating state in real time
- 2026-04-11: validated the new chunk-isolation recovery path in the live Buebito run:
  - `data/CommonEvents.json` chunk `15/129` failed three times on preserved control codes for `CommonEvents:132:text:137`
  - the runner applied the valid subset (`+23`), retried the isolated entry, then applied that remaining entry successfully
  - no `translation_failures.json` manifest was emitted, so the campaign continued without a hard stop
- 2026-04-11: validated the browser-based playtest path ahead of the final export:
  - served the original Buebito root over `python -m http.server 8765 --bind 127.0.0.1`
  - opened `http://127.0.0.1:8765/index.html` with Playwright and confirmed the game booted to the title screen
  - confirmed the input gesture needed for browser playtest: click the canvas to focus it, then `Enter` advances from the menu into dialogue
  - captured evidence screenshots (`buebito-original-menu.png`, `buebito-original-after-enter.png`, `buebito-original-after-canvas-enter.png`)
  - this removes the earlier uncertainty around relying on the exported `Game.exe`; the final one-hour playtest can use the same browser-served path against `export-es`
- 2026-04-11: revalidated the oversize fail-fast contract and closed `P2-06` with targeted tests:
  - `cargo test --manifest-path src-tauri/Cargo.toml test_translation_plan_reports_unshrinkable_oversized_codex_chunk -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml test_export_ai_batch_rejects_unshrinkable_oversized_prompt -- --nocapture`
  - command-path audit confirmed `commands/translation.rs` and `ai_batch.rs` both call the fail-fast budget guard before provider execution/export
- 2026-04-11: validated live usage-limit recovery during the Buebito campaign:
  - `gpt-5.3-codex-spark low` hit an explicit usage-limit error on `data/Map005.json chunk 1/2`
  - the runner activated fallback exactly once, switching to `gpt-5.4-mini low`
  - translation resumed from the same checkpoint without emitting `translation_failures.json`
- 2026-04-11: rehydrated session state from persistent notes and inspected the live Buebito workspace after compaction:
  - the prior runner process was still alive but stalled for ~9 minutes at `data/Map020.json chunk 3/4`
  - `qa_translate.status.json` and `qa_translate.progress.log` had both stopped updating at `05:55:19`
  - no `translation_failures.json` manifest existed, so the run was treated as a hung provider call rather than a completed failure state
- 2026-04-11: force-stopped the hung runner and relaunched the campaign from the same checkpoint in a visible PowerShell window:
  - new runner PID: `28776`
  - primary model restored to `gpt-5.3-codex-spark low`
  - fallback remains `gpt-5.4-mini low` with `usage_limit_only`
  - restart resumed from `5261` already translated entries, leaving `625` pending
  - fresh planning output and status updates resumed immediately, confirming the workspace/checkpoint remained healthy
- 2026-04-11: hardened placeholder/control-code protection for attached honorific compounds:
  - `translation_integrity` now treats spans like `\N[2]-onee-chan` and `\N[1]-kun` as single protected tokens instead of split placeholder + suffix fragments
  - the same protection feeds compact prompt masking/restoration, so provider payloads and apply/export validation now agree on compound preservation
  - verification:
    - `cargo test --manifest-path src-tauri/Cargo.toml compact_prompt -- --nocapture`
    - `cargo test --manifest-path src-tauri/Cargo.toml translation_integrity -- --nocapture`
    - `cargo check --manifest-path src-tauri/Cargo.toml`
- 2026-04-11: re-export validation exposed `7` latent broken checkpoint entries that had slipped through the older live run; they were repaired directly in the Buebito workspace checkpoint:
  - `CommonEvents:61:text:46`
  - `Map013:6:0:choice:18:0`
  - `Map013:6:0:choice:18:1`
  - `Map013:6:0:choice:18:2`
  - `Map020:4:0:text:53`
  - `Map020:4:0:text:80`
  - `Map056:1:0:text:148`
  - representative fixes restored broken honorific compounds and misplaced punctuation around protected names, e.g. `\N[2]-onee-chan` and `¡\N[4]-onee-chan!`
- 2026-04-11: implemented missing `js/plugins.js` extraction/export support and validated it end to end:
  - parser now classifies/scans `js/plugins.js`, extracts translatable plugin description/parameter strings (including JSON-encoded parameter blobs), and exporter writes them back through `render_plugins_js`
  - `translation_orchestrator` now tags `js/plugins.js` chunks as `FileKind::Plugins`
  - verification:
    - `cargo test --manifest-path src-tauri/Cargo.toml rpgmaker -- --nocapture`
    - `cargo test --manifest-path src-tauri/Cargo.toml exporter -- --nocapture`
    - `cargo check --manifest-path src-tauri/Cargo.toml`
- 2026-04-11: patched `qa_translate_provider` so reruns on the same workspace regenerate `export-es` instead of failing on a non-empty export folder:
  - the runner now removes its own prior `workspace/export-es` before exporting and logs that cleanup explicitly
  - verification:
    - `cargo test --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider -- --nocapture`
    - `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
- 2026-04-11: integrated `js/plugins.js` into the live Buebito checkpoint and reran only that file through the real provider flow:
  - copied the freshly extracted `entries/plugins.js` probe into `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\project.fairtl`
  - added `js/plugins.js` to the checkpoint `project.json`
  - translated `55/55` plugin entries with `gpt-5.3-codex-spark low`; Spark then hit an explicit usage-limit error on chunk `1/3`, and the runner correctly activated the configured fallback to `gpt-5.4-mini low` without restarting the workspace
  - no `translation_failures.json` manifest was emitted
- 2026-04-11: regenerated `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\export-es` successfully after the plugin-file update:
  - final rerun log ended cleanly with `warnings=244`
  - workspace state at re-export:
    - `43` files in the checkpoint
    - `6063` total entries
    - `5941` translated entries
    - `122` remaining `untranslated` entries, all confirmed to have blank `original` text rather than missed content
  - `qa_translate.status.json` is back to `phase=completed`
- 2026-04-11: browser playtest on the regenerated export confirmed the plugin/menu fix and broader runtime integrity:
  - served `export-es` from `http://127.0.0.1:8768/index.html`
  - confirmed `window.$plugins` now loads translated `MenuSubCommand` labels (`Introducción de personajes`, `Cambio de hora`, `Salto`, `Gestión de interruptores iniciales`, `Para depuración`)
  - verified the live playable menu now shows `Introducción de personajes` in Spanish in a natural map state
  - documented screenshots:
    - `buebito-playtest-menu-pluginfix-confirmed.png`
    - `buebito-playtest-menu-natural-state.png`
    - `buebito-playtest-character-intro-scene.png`
    - `buebito-playtest-character-intro-rina.png`
    - `buebito-playtest-greeting-after-intro.png`
    - `buebito-playtest-laundry-event.png`
    - `buebito-playtest-map06-free-control.png`
- 2026-04-11: extended the real export playtest far beyond the title/menu smoke path:
  - advanced through the opening arrival, house-entry greetings, shared meal, laundry event chain, bath event chain, and back into free movement
  - observed stable translated dialogue across maps `34`, `51`, `53`, `56`, `57`, and eventual free control on map `6`
  - verified that the previously broken runtime line around `\N[2]-onee-chan` food dialogue now renders correctly in the exported build
  - Playwright console showed `0` runtime errors; the only repeated warnings were legacy `CanvasTextAlign` warnings from `rmmz_core.js`, which appear unrelated to the translation/export changes
- 2026-04-11: resumed the exported Buebito playtest in a fresh browser session at `http://127.0.0.1:8770/index.html?fresh=7` and expanded the evidence with explicitly visual scene notes:
  - recaptured the house-entry greeting, laundry chain, next-morning bedroom, free-control room, and natural in-game menu with new screenshots:
    - `buebito-playtest-v2-house-greeting.png`
    - `buebito-playtest-v2-laundry-hana.png`
    - `buebito-playtest-v2-morning-room.png`
    - `buebito-playtest-v2-map06-free-control.png`
    - `buebito-playtest-v2-menu-natural.png`
  - confirmed again that the exported runtime keeps `Introducción de personajes` in the menu and preserves compound honorific placeholders in scenes that previously broke
  - extended documented wall-clock playtest coverage from `~17 min` to `~27 min`, still below the one-hour target but now with stronger scene-by-scene visual evidence instead of only technical state snapshots

## Baseline Audit

### Severity Summary
- `P1`: prompt budgeting/context overflow risk across LLM and external AI; style-guide save false positive causing unsaved-on-disk state; no dirty-state guard; no text-integrity gate before apply/export.
- `P2`: schema/versioning gap; progress contract still misleading; Google/UI capability mismatch; external AI weaker than in-app path; premature context-memory truncation; provider-specific robustness gaps.
- `P3`: manual editing/search/resize performance issues that materially affect responsiveness but do not currently corrupt data.

### Broken UI/Backend Contracts
- UI says "Save rules"; backend only mutates in-memory project state.
- Provider tab copy suggests internal chunking/project memory/glossary orchestration broadly, but Google ignores context and external AI is not using the same contextual selection model.
- Progress UI shows "files" but backend currently counts chunks as files and emits duplicate boundary updates.
- Project replacement flows do not warn about unsaved changes because the product has no dirty-state concept.

### Immediate Technical Debt
- Shared translation-context budgeting does not exist yet; prompt construction is duplicated between providers and external AI.
- Editing path updates the whole project tree for single-entry mutations and serializes bulk replace operations into many IPC roundtrips.
- There is no centralized translation integrity validator for control codes, placeholder preservation, line-count drift, or whitespace-only export filtering.

## Blockers
- None confirmed yet.

## Attack Plan
- Batch A: shared prompt budgeting/context selection for OpenAI, Codex, and External AI; remove premature global truncation of term/examples; add targeted Rust tests.
- Batch B: truthful persistence and project-integrity flow: style-guide save persistence, schema bump/backward compatibility, dirty-state and unsaved-change guards.
- Batch C: progress contract and provider honesty: unique file counts, chunk metadata, no duplicate events, localized/actionable warnings, provider capability messaging.
- Batch D: text-integrity safeguards: preserve control codes/placeholders/line breaks, export/apply warnings, multiline handling, whitespace-only export filter, RPG heuristics.
- Batch E: editing/perf cleanup: remove artificial edit delay, optimistic/bulk updates, reduce whole-project recomputation hot spots.
- Campaign QA: run the corrected end-to-end flow on the removable-disk Buebito MZ game, then perform a documented one-hour playtest with explicit notes for elapsed time, visible text, actions taken, warnings/errors, and any screenshots or runtime blockers.

## Playtest Logging Protocol
- Real-campaign playtest target: `G:\Leyo\Buebito\My Summer with Onee-chans (ぼくと夏のおねえちゃん達) v1.1 - Okeyutei`
- Minimum target duration: `1 hour` of actual game time unless blocked by a technical limitation that is itself documented with evidence.
- The playtest record must capture:
  - start/end timestamps and total elapsed time,
  - launch method used,
  - menu/navigation/actions performed,
  - visible translated text reviewed,
  - layout/overflow/control-code issues observed,
  - runtime warnings/errors/blockers,
  - export/build artifact paths used for the session.
- Detailed notes will be written in `docs/PLAYTEST_BUEBITO_LOG.md`.

## Batch F: Compact Prompts, Larger Chunks, Faster Resume

### Scope
- Replace verbose per-provider prompt construction with one compact structured serializer.
- Push chunk sizes upward safely so `spark low` and `5.4-mini` spend less time/prompt on tiny batches.
- Add previous/next chunk hints without dragging full neighboring blocks into every request.
- Speed up replanning/resume by caching expensive translation-example preparation instead of rebuilding everything on each run.
- Prevent long QA runs from dying on a handful of stubborn entries; continue with a failure manifest instead.

### Changes Implemented
- Added `compact_prompt.rs` as a shared compact serializer/parser for `Codex`, `OpenAI`, and `External AI`.
- Switched provider/external-AI payloads to minified structured JSON with local chunk IDs (`k`) and compact metadata/context fields.
- Masked control codes/placeholders before sending to the model and restored them before validation/apply.
- Extended `TranslationBatchContext` with compact previous/next chunk hints.
- Reworked `translation_orchestrator` to:
  - use larger LLM chunks (`24` max entries),
  - precompute example features,
  - keep chunk-level estimated prompt tokens,
  - surface a reusable `PlannerCacheData` plus planning progress snapshots.
- Reworked `qa_translate_provider` to:
  - persist planner cache/status/failure manifests,
  - show explicit planning progress,
  - isolate bad entries after chunk-level retries,
  - block clean export when unresolved failures remain.
- Reworked `External AI` export/apply tooling to stay backward-compatible while accepting the new compact response shape with `responseMap`.

### Micro-Audit After Batch F
- The compact serializer now remaps to real IDs before the existing integrity/apply path, so the validation contract stayed centralized instead of being reimplemented ad hoc in each provider.
- The compact external-AI parser initially misclassified legacy `projectId/batchId` envelopes as compact because Serde ignores unknown keys by default; this was fixed by explicitly detecting legacy shape before compact parsing.
- Increasing chunk size without also compacting payload would have just moved the bottleneck; the current implementation changes both together and keeps budget checks tied to the real compact payload estimator.
- Neighbor context was intentionally limited to short hints; full adjacent blocks would have erased most token savings and made retries noisier.

### Residual Risk
- Budgeting is now materially better and chunks are larger, but the planner still uses a single conservative ceiling for the provider mode rather than a true model-specific window profile. That is acceptable for the current `spark low` run but remains a product debt if we want to fully exploit model-specific context windows later.
- The QA runner can now continue past isolated failures, but UI-driven batch translation still uses the simpler in-app batch path and does not yet expose the same failure-manifest semantics.

## Campaign Delta: Buebito Export + Runtime

### Resolved Since Last Checkpoint
- `P1` The live Buebito checkpoint no longer contains the broken honorific-placeholder compounds uncovered during re-export. The new compound-token integrity rule plus direct checkpoint repairs closed the concrete runtime corruption that had shown up as malformed lines like `\N[2]¡La comida de -onee-chan...`.
- `P2` The product no longer drops `js/plugins.js` on import/export. Real runtime verification confirmed the previously untranslated `Character Introduction` menu label now comes from the translated export as `Introducción de personajes`.
- `P2` QA reruns on the same workspace no longer fail just because `export-es` already exists. The runner now clears its own export directory and completes the re-export path cleanly.

### Current QA Assessment
- The Buebito export is structurally healthy again:
  - no unresolved failure manifest
  - no meaningful untranslated content left in the checkpoint
  - safe font fallback still applied for Latin extended glyph coverage
- The runtime export now feels materially more coherent as a product:
  - title/game name renders correctly
  - early dialogue is legible and accent-safe
  - menu/plugin labels are no longer half-English
  - protected names/honorific compounds survive into the browser runtime
- The remaining export warnings are heuristic overflow warnings, not integrity failures. They need follow-up triage, but they did not block the current campaign run or the observed early-game flow.
- The playtest log is now materially stronger as user-facing evidence:
  - it no longer records only map IDs and dialogue snippets
  - it now also describes what the player actually sees on screen in each checkpointed scene

### Real Residual Risks
- The one-hour wall-clock playtest target is still not complete in this session. The current documented runtime coverage is meaningful and includes several maps/scenes plus recovered free control, but total real elapsed playtest time is still only about `~27 min`.
- The exported build still produces many browser warnings of the form `CanvasTextAlign undefined` from upstream `rmmz_core.js`. I observed `0` browser errors, but those warnings should be treated as ambient engine noise until separately investigated.
- `244` export warnings remain, most of them short-field growth heuristics. They are not proven regressions yet, but they are real residual review debt before claiming full visual-fit confidence across every menu/item field.

## Next Exact Step
- Continue the Buebito export playtest from free control on map `6`, add more movement/menu/NPC coverage until the documented wall-clock session gets materially closer to the one-hour target, and use the accumulated screenshots plus visual scene descriptions to triage whether any of the remaining heuristic overflow warnings turn into real on-screen problems.

## Batch A Implementation

### Scope
- Real prompt-budget estimation for OpenAI, Codex, and External AI based on the actual prompt payload rather than only entry text size.
- Context-aware chunk planning that now accounts for style guide, glossary section, term hints, translation examples, file-context hints, metadata, and JSON/request wrapper overhead.
- Removal of premature global truncation of `term_hints` and `translation_examples`; selection now happens against the full candidate set and trims only per chunk when the prompt budget requires it.

### Changes Implemented
- Added a shared approximate token estimator in `src-tauri/src/services/provider.rs`.
- Exposed prompt builders plus total-token estimators for `OpenAI` and `Codex` so planning uses the same real prompt shape the providers send.
- Reworked `translation_orchestrator` to:
  - plan chunks with provider-aware prompt budgeting,
  - build per-chunk contextual memory,
  - trim least-relevant context blocks when estimated prompt size would overflow,
  - preserve late-but-relevant term hints/examples that were previously dropped by global truncation.
- Reworked `External AI` export batching to reuse the same contextual chunk plan instead of slicing only by `max_entries`, and expanded its prompt to include style guide, glossary, term hints, translation memory examples, and chunk context metadata.

### Micro-Audit After Batch A
- Confirmed the new chunk planner measures actual prompt builders instead of a hand-wavy char count.
- Found and fixed a prompt-bloat bug while re-reading the diff: glossary terms were being duplicated inside `term_hints`; they are now only represented in the glossary section plus non-glossary established terms.
- Revalidated after the fix with a second targeted test pass on `translation_orchestrator` and `ai_batch`.

### Evidence
- `translation_orchestrator` tests now cover:
  - context-aware chunk planning,
  - context trimming under oversized prompt pressure,
  - retention of late relevant term hints,
  - retention of late relevant translation examples.
- `ai_batch` tests now cover:
  - contextual prompt content,
  - prompt-budget trimming for oversized external-AI exports,
  - legacy split behavior by batch size.

### Residual Risk
- A single unusually large entry combined with an unusually large full glossary can still trip the conservative estimated budget after optional context blocks are trimmed, because glossary terms are still passed as a full section rather than relevance-trimmed per chunk. That now fails fast explicitly instead of running silently over budget, but it still leaves some valid borderline cases unsent until we add model-specific ceilings or glossary relevance trimming.

## Batch B Implementation

### Scope
- Make the style-guide save action truthful instead of in-memory-only.
- Bump `.fairtl` schema version and add backward-compatible load coverage for pre-style-guide project files.
- Add explicit unsaved-changes tracking and discard confirmation on destructive open/create flows.

### Changes Implemented
- `style_guide_replace` now persists the full project snapshot to disk atomically before returning success, instead of only mutating in-memory state.
- `.fairtl` schema version was bumped from `1` to `2`, and persistence tests now cover both current-schema writes and loading a schema-`1` project that lacks `styleGuide`.
- Frontend project state now tracks `isDirty` explicitly instead of inferring from timestamps.
- Dirty-state semantics were wired through create/open/save/manual edit/glossary/style-guide/provider-apply flows.
- Open/create replacement flows now prompt before discarding unsaved changes.
- `useGlossary.refresh()` and `useStyleGuide.refresh()` no longer mutate `updatedAt` or accidentally manufacture fake local dirtiness on read.

### Micro-Audit After Batch B
- Verified that style-guide save now uses the same atomic project persistence path as full `project_save`; success is only reported after disk write succeeds.
- Re-read project replacement flows and confirmed the only active UI paths that can replace the current project snapshot are the toolbar open flow and new-project creation flow; both are now guarded.
- Tried to expose `commands/*` to the Rust test harness for direct command-layer tests. That revealed a real tooling problem: enabling command modules in `lib` tests crashes the Windows harness with `STATUS_ENTRYPOINT_NOT_FOUND`. The product fix remains valid, but command-glue testing stays a debt item until that harness problem is resolved.

### Evidence
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml persistence -- --nocapture`
- `npm run test:run -- src/__tests__/projectStore.test.ts src/__tests__/useTranslation.test.tsx`

### Residual Risk
- The style-guide persistence path is now implemented in the live command code and validated indirectly via compile checks plus persistence tests, but direct command-layer tests remain blocked by the current Windows lib-test harness limitation noted above.

## Batch C Implementation

### Scope
- Repair progress semantics so files and chunks stop being conflated.
- Remove duplicate progress emissions.
- Fix the empty-run path so it does not fail on provider validation when there is nothing to translate.
- Align provider-facing UI messaging with what each provider actually does today.

### Changes Implemented
- Extended `BatchProgress` to carry both file-level and chunk-level counters.
- `translate_batch_start` now plans work before validating the provider and exits cleanly when there is nothing to translate.
- `run_batch_translation` now:
  - counts unique files separately from total chunks,
  - emits one progress event per completed chunk instead of two,
  - reports per-file chunk progress for the currently processed file.
- Batch-translate UI now shows entry progress plus explicit file and chunk counters.
- Provider messaging in the batch panel was rewritten to stop promising identical internal context/memory support across all providers; OpenAI/Codex, DeepL, and Google now expose different capability notes.

### Micro-Audit After Batch C
- Confirmed the previous duplicate-event jitter path in `translation.rs` is gone.
- Confirmed the zero-work flow now returns without requiring API validation or keys for an empty selection.
- Re-read provider copy against backend behavior:
  - OpenAI/Codex are described as full-context providers.
  - DeepL is described as best-effort glossary/style guidance only.
  - Google is described as basic MT with placeholder protection only.

### Evidence
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `npm run test:run -- src/__tests__/projectStore.test.ts src/__tests__/useTranslation.test.tsx`
- `npm run build`

### Residual Risk
- Progress semantics are now truthful in payload shape and UI copy, but command-layer Rust tests still cannot exercise `commands/translation.rs` directly because of the Windows lib-test harness limitation already recorded under `DEBT-01`.

## Batch D Implementation

### Scope
- Add a centralized translation-integrity guard for automatic provider apply, external-AI apply, and export preflight.
- Stop whitespace-only translations from being treated as valid export content.
- Surface actionable warnings instead of silently reshaping risky text.
- Align QA helpers with the same guard so end-to-end validation does not bypass the live product rules.

### Changes Implemented
- Added `src-tauri/src/services/translation_integrity.rs` as the shared validation layer for:
  - RPG Maker control codes and placeholder preservation,
  - line-count drift detection,
  - compact-field overflow heuristics,
  - export preflight summary generation.
- Added `src-tauri/src/services/translation_apply.rs` so provider-response application is both centralized and testable outside the Tauri command harness.
- Provider batch apply in `commands/translation.rs` now routes through the shared integrity layer:
  - invalid automatic translations are skipped instead of being written into the project,
  - failed-entry counts and warnings now reflect integrity failures rather than pretending success.
- External AI apply in `services/ai_batch.rs` now:
  - blocks invalid translations with the same guard,
  - returns `invalidEntries` plus warning messages to the UI,
  - avoids mutating notes when a translation is rejected by integrity checks.
- Export in `services/exporter.rs` now:
  - trims whitespace-only translations out of the export set,
  - runs integrity preflight before copying files,
  - blocks export on real errors such as control-code drift or multiline event drift,
  - returns non-blocking warning messages for heuristic review items.
- UI flows were updated so the new backend truth is visible:
  - external-AI apply now shows integrity warnings and counts blocked entries,
  - export now surfaces warning summaries when preflight allows export with heuristics.
- QA helpers were aligned with the same rules:
  - `qa_apply` now fails if external-AI batches produce invalid entries and records warning output in the report,
  - `qa_translate_provider` now uses `translation_apply::apply_provider_responses` instead of its own bypass path.

### Micro-Audit After Batch D
- Re-read the new integrity layer and confirmed it is now the single source of truth for control-code, placeholder, line-count, and compact-field heuristics across provider apply, external-AI apply, and export.
- Found and fixed one partial-mutation issue during review: external-AI notes were being merged before integrity rejection. Notes now update only for skipped or accepted rows, not for rejected translations.
- Re-audited the QA helpers after integration and found they were still bypassing or under-reporting integrity results. Both helpers now follow the same acceptance rules as the product path.

### Evidence
- New Rust coverage:
  - `translation_integrity` tests cover control-code mismatch, automated line-break drift blocking, compact-field overflow warnings, and blank-translation export ignore rules.
  - `translation_apply` tests cover valid provider apply and rejected provider apply when integrity breaks.
  - `ai_batch` tests cover rejected external-AI apply when integrity breaks.
  - `exporter` tests cover whitespace-only export filtering and export blocking on multiline drift.
- Revalidated after QA-helper alignment with:
  - `cargo check --manifest-path src-tauri/Cargo.toml`
  - `npm run test:run -- src/__tests__/projectStore.test.ts src/__tests__/useTranslation.test.tsx`
  - `npm run build`

### Residual Risk
- The current heuristics still do not simulate real in-game visual wrapping or per-window pixel width; they flag likely overflow in compact fields, but final UI fit still needs playtest validation.
- Provider-specific MT paths still remain weaker than OpenAI/Codex on contextual quality, even though the new guard now blocks the most dangerous structural corruption cases.

## Batch E Implementation

### Scope
- Remove artificial manual-edit latency and cut expensive bulk-edit IPC chatter.
- Reduce obvious whole-project recomputation and settings-write churn on hot editing/search paths.
- Improve QA helper observability so long-running campaign translation no longer appears hung.

### Changes Implemented
- Manual editing now enters immediately on single click instead of waiting behind an artificial delay.
- Single-entry saves are now optimistic in the frontend and only roll back on backend failure.
- Replace-all now sends one batched project-entry update instead of a long series of sequential IPC writes.
- Sidebar and status-bar statistics were collapsed into single memoized passes, global search now uses a deferred query, and sidebar resize only persists after drag end rather than on every mousemove.
- `qa_translate_provider` now emits startup/progress checkpoints and writes them to `qa_translate.progress.log` inside the chosen workspace so large end-to-end runs can be monitored live.
- `qa_translate_provider` now creates `workspace/project.fairtl` immediately, saves a checkpoint after each completed chunk, and resumes from that checkpoint on relaunch instead of reparsing the whole game from scratch.
- `workspace/project.fairtl` was revalidated as a full project snapshot directory (`project.json` + `entries/`), so resume semantics are currently chunk-level: only fully applied chunks are checkpointed.
- The Codex provider/helper path now supports explicit `reasoning_effort` passthrough, and the live logs/reports disclose both `model` and `reasoning`.
- `qa_translate_provider` now supports automatic model fallback on translation/runtime failure while staying inside the same checkpointed workspace.
- `qa_translate_provider` now treats each chunk application transactionally during campaign runs:
  - it snapshots the target file state before apply,
  - reverts partial mutations if the provider returns missing IDs, unexpected IDs, or integrity-invalid translations,
  - retries the same chunk up to `3` times on the active model,
  - escalates to the configured fallback model if repeated invalid chunk output persists.
- Translation planning/export/UI progress now ignore entries whose `original` text is blank:
  - empty-source entries no longer enter provider/external-AI batches,
  - untranslated counts and progress denominators no longer overstate blank placeholders as pending work,
  - the Buebito checkpoint was measured with `122` blank-source entries that previously inflated backlog and caused provider-integrity failures.
- Provider/external-AI apply now normalize literal escaped line breaks (`\\n`, `\\r\\n`) back to real newlines when the source entry is actually multiline. This prevents JSON-escaped line breaks from being misread as bogus control codes during automated apply.
- Added `qa_dump_plan` so the current contextual chunk plan can be exported as JSON for inspection without disturbing the live run.

### Micro-Audit After Batch E
- Confirmed `P3-01` is now resolved: the original close criteria for immediate editing, optimistic state, and batched replace-all are met.
- Confirmed `P3-02` is improved but not closed: the grid still re-derives project-wide flattened/filter data after committed edits, and column resize still persists on every drag tick.
- Found an execution-observability gap during the first real Buebito rerun: background `stderr` redirection was not reliable enough to prove live progress. The helper now writes to a dedicated progress file with per-line flushes.
- Found a real campaign-recovery gap during the live Buebito `spark-low` run: `data/CommonEvents.json` chunk `3/252` returned two whitespace-only translations and the helper aborted even though the checkpoint was clean. The helper now reverts partial chunk mutations, retries invalid chunk responses, and only advances the checkpoint after a clean chunk apply.
- Revalidated the `CommonEvents` failure root cause locally: the repeating invalid IDs were blank-source entries (`original: ""`) that should never have entered translation planning in the first place. The pipeline now skips blank-source entries centrally instead of asking providers to "translate" empty strings.
- Revalidated the second live `CommonEvents` failure pattern locally: Spark was sometimes returning literal `\\n` text for multiline dialogue inside JSON, which the integrity guard previously surfaced as odd placeholder/control-code drift (`unexpected \\nMmm`). Apply paths now normalize escaped newlines before validation, so genuine multiline dialogue is judged against the intended line structure.

### Evidence
- `cargo check --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
- `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
- `cargo fmt --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml codex -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml --bin qa_dump_plan`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin qa_dump_plan -- --project D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\project.fairtl --provider codex --output D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\qa_translate.plan.json`
- live campaign monitor file: `D:\FairTranslatorRuns\buebito-codex-20260411-spark\qa_translate.progress.log`
- live checkpoint/resume monitor file: `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low\qa_translate.progress.log`
- 2026-04-11: after the invalid `CommonEvents` chunk failure, rebuilt `qa_translate_provider`, relaunched the same workspace with `gpt-5.3-codex-spark low` + `gpt-5.4-mini low` fallback, and confirmed the resumed run starts from `ya_traducidas=1040` instead of restarting from zero.
- 2026-04-11: validated the blank-entry fix with:
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_orchestrator -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
  - `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
  - `npm run build`
- 2026-04-11: validated newline-normalization + fallback-policy changes with:
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_integrity -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml translation_apply -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml ai_batch -- --nocapture`
  - `cargo build --manifest-path src-tauri/Cargo.toml --bin qa_translate_provider`
- 2026-04-11: relaunched the Buebito campaign in a visible PowerShell console using `gpt-5.3-codex-spark` + `reasoning=low` only, reusing the same checkpoint workspace `D:\FairTranslatorRuns\buebito-codex-20260411-spark-low`.
- 2026-04-11: relaunched the Buebito campaign in background monitoring mode with:
  - primary `gpt-5.3-codex-spark` + `reasoning=low`
  - fallback `gpt-5.4-mini` + `reasoning=low`
  - fallback policy narrowed to explicit CLI usage-limit/quota errors only
  - resumed state `ya_traducidas=1054`
  - confirmed progress through `data/CommonEvents.json` chunks `1-4`; chunk `2` now applies cleanly, and chunk `3` succeeded after one retry on a transient line-count drift.

### Residual Risk
- Large-project contextual planning still has a silent “planning in progress” window until the next chunk-level checkpoint appears; the new progress file narrows the observability gap, but not every internal planner step is yet streamed.

## Provider Hardening

### Scope
- Remove Google's naive substring glossary replacement so protected terms stop corrupting unrelated words.
- Improve Google's preservation of control codes, placeholders, and literal line breaks before the MT step.
- Keep provider messaging aligned with the actual protection now in place.

### Changes Implemented
- Added boundary-aware glossary protection in `src-tauri/src/services/google.rs`:
  - longer glossary terms win before shorter overlapping ones,
  - standalone terms are protected without replacing substrings inside unrelated words,
  - token prefixes are randomized per request to avoid accidental collisions with source text.
- Google now protects control codes, placeholder tokens, and literal newlines before translation and restores them after translation.
- Updated provider capability copy so Google is described as basic MT with glossary plus control-code placeholder protection rather than only naive glossary placeholders.

### Micro-Audit After Provider Hardening
- Re-read the Google protection path and confirmed it now shares the same token grammar as the central integrity validator for control-code/placeholders.
- Verified the new boundary logic does not replace `art` inside `start`/`artful`, while still protecting the standalone glossary term.
- Confirmed the provider still does not claim contextual memory or chunk-aware project context; only the literal-preservation guarantee changed.

### Evidence
- `cargo test --manifest-path src-tauri/Cargo.toml google -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml translation_integrity -- --nocapture`
- `cargo check --manifest-path src-tauri/Cargo.toml`

### Residual Risk
- Google still remains a basic MT path without project-memory context or translation-memory reasoning, so terminology quality may still lag behind OpenAI/Codex even though corruption risk is lower.
- DeepL still relies on best-effort style/glossary context rather than the full contextual orchestration path; this is disclosed in-product but not upgraded technically yet.

## Runtime Playtest Findings
- `P1` The Buebito map-`63` recall hub still exposes untranslated English labels inside the runtime custom memory menus even after the export/menu/plugin fixes. Observed live in Hana/Hina recall lists with entries such as `Sleeping Blowjob.`, `Sleeping Missionary.`, `Side Handjob.`, `Chair Blowjob.`, and `Back.` while adjacent UI chrome remains Spanish.
- `P2` The same recall hub still has mixed-language helper text and weak scene-localization lines in live play. Confirmed visible examples include `¡Este es un recuerdo frente a Candy Store!`, `Umm～... ¿Cuánto duraba de nuevo el tiempo～?`, `Otra vez... se me está poniendo todo hormigueante...`, and the awkward split `Esta es una memoria de Hana-onee-chan, ¡que siempre es tan amable!`.
- `P2` Event inspection in the map-`63` explanation helper (`ev_説明`) revealed an embedded Cyrillic token `хотя` inside supposed Spanish copy. That exact line still needs direct on-screen reproduction before source triage/fix work starts.
- `P2` The Mama branch reached from the same hub is structurally stable but still contains very literal/self-referential Spanish in a visually exposed black-screen segment. Live examples include `Mama se siente tan feliz♡`, `¿Qué debería hacer Mama?`, and the especially awkward `Mama no me importa ni de esta ni de la otra♡`.

## Runtime Evidence
- 2026-04-11: resumed the real exported game from `http://127.0.0.1:8770/index.html?fresh=8` using the visible `Autoguardado` (`00:46:21`) instead of replaying the prologue.
- 2026-04-11: reached live free-control map `63` and documented the memory-hub layout with three sisters visible around Yuu plus persistent HUD/menu chrome.
- 2026-04-11: played Hana's recall interaction end-to-end with automation-assisted pathfinding plus normal `ok` activation:
  - base prompt in Spanish,
  - recall menu list in English,
  - internal bedroom scene choices in Spanish,
  - internal dialogue partially translated but stylistically uneven.
- 2026-04-11: continued Hina's active scene from black-screen dialogue intro into a fully illustrated night corridor/veranda scene, then back into the hub's time-of-day selector.
- 2026-04-11: continued the Mama route from the same hub into map `61`:
  - initial standing frame showed Mama and Yuu together with a strong height/body-size contrast,
  - several dialogue boxes played over total black with only portraits and HUD visible,
  - the route then opened into a fully illustrated blue-lit bedroom CG with a visible choice overlay,
  - no runtime/control-code failures appeared during this branch, but the black-screen text exposed multiple weak literal Spanish lines.
- 2026-04-11: screenshots captured during this pass include:
  - `buebito-current-state-before-next-action.png`
  - `buebito-map63-hana-memory-prompt.png`
  - `buebito-map63-hana-memory-menu.png`
  - `buebito-hana-scene1-loaded.png`
  - `buebito-current-scene-continue-request.png`
  - `buebito-current-scene-step2.png`
  - `buebito-current-scene-step3.png`
  - `buebito-current-scene-step4-fuerte.png`
  - `buebito-menu-hold-before-continue.png`
  - `buebito-hub-pre-interaction-2.png`
  - `buebito-mama-scene-step2.png`
  - `buebito-mama-scene-step3-choice.png`
  - `buebito-mama-scene-step4-choice-list.png`
  - `buebito-mama-scene-step5-cg.png`
  - `buebito-mama-scene-step6.png`
  - `buebito-mama-scene-step7.png`
  - `buebito-mama-scene-step8.png`
  - `buebito-mama-scene-post-dialogue.png`
  - `buebito-mama-scene-step9-after-choice.png`
  - detailed runplay note: `docs/PLAYTEST_BUEBITO_RUNPLAY_MAMA_MAP61.md`

## Exact Next Step
- Resume the live export from the current map-`61` Mama CG branch after selecting `Con fuerza.`, finish that memory until control returns to the map-`63` hub, then reproduce the explanation helper `ev_説明` on-screen to confirm whether the inspected `хотя` corruption is player-visible, and finally check the remaining sister recall menu to see whether the English-label defect affects the full trio or only Hana/Hina/Mama.

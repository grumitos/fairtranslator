# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Fitted translated text to each engine's windows on export: messages are
  rewrapped to the real window width (face graphics, font size changes, message
  plugins and the width the original text already uses) and split into extra
  pages when needed, with native paging on MV/MZ/VX Ace/VX and new show-text
  commands on XP. The export result, the Export panel and
  `qa_validate_translation --json` report rewrapped entries, added pages and
  what still does not fit, and the Export panel can run the same analysis
  before exporting.
- Added "Save as" (Ctrl+Shift+S), a default project location under
  `Documents\FairTranslator`, the project path in the project menu and a
  recent-projects list on the welcome screen.
- Added game inspection to the new-project dialog: engine, text files and
  entries, and a clear block for packed RPG Maker games.
- Added background updates of the standard Codex CLI install at startup
  ("Update on startup" in Settings, on by default) and Codex model discovery
  through `codex debug models`.

### Changed
- Redesigned the interface on the shared `@nosky/design-kit` (vendored in
  `src/styles/kit`): dark only, frameless window with its own title bar, a
  monochrome logo, side drawers for batch translation, project context, export
  and settings, control codes shown as chips, and in-app confirmation dialogs
  instead of the browser's.
- Find and replace now counts and replaces only the rows the grid shows.
- Editing the style guide now writes only `project.json` and keeps unsaved entry
  edits pending.
- Aligned Tauri at 2.11 on both sides (`tauri` 2.11.6, `@tauri-apps/api`
  2.11.1, `@tauri-apps/cli` 2.11.5, dialog plugin 2.7.3); 2.12 waits for the
  7-day npm release-age rule.
- Made OpenCode provider chunks adaptive up to 128 entries, with independent
  prompt and estimated-response token budgets instead of a fixed 16-entry cap.
- Made the headless OpenCode runner reuse the credential saved by FairTranslator
  in the operating-system secure store by default; key files are compatibility
  overrides only.
- CI now runs `cargo clippy --all-targets --locked -- -D warnings` (with
  `too_many_arguments` allowed) instead of `cargo check`.

### Fixed
- Extracted and exported the first line of every RPG Maker XP message (show-text
  command 101); existing XP projects gain the missing entries when opened.
- Stopped squeezing MV/MZ text that did not fit the original lines into one
  overlong last line.
- Kept the project marked as unsaved when an edit lands while a save is
  running, so closing the app still warns about it.
- Promoted an untranslated entry to draft when a translation is typed in the
  entry drawer, so the export no longer skips it.
- Fixed Enter, arrows and Home inside the entry drawer, trailing spaces in style
  guide rules, literal replacement text and IME composition in the grid.
- Allowed the main window to close when the unsaved-changes guard is active
  (missing `core:window:allow-destroy` permission).
- "Save as" onto an existing folder that is not a project no longer deletes it.
- Surfaced write errors of VX Ace/VX/XP exports instead of reporting success,
  and flushed saved and exported files to disk before replacing them.
- Kept nested default values of MV/MZ plugin parameters on export.
- Strengthened OpenCode readiness checks to require a valid structured
  translation and negotiate/cache the API family that satisfies the schema.
- Preserved completed checkpoints when planner-budget changes rebuild a chunk
  plan during resume.
- Found the Codex CLI on `PATH`: a bare `codex` was never searched there, so a
  standard install is now used, discovered and auto-updated without setting
  its path. The runtime order is now: configured path, sidecar bundled next to
  the app, `PATH`, and only in debug builds the checkout's ignored
  `src-tauri/binaries` copy; release builds no longer embed the build
  machine's checkout path.
- Stopped rejecting AI translations that merge or re-break lines in messages,
  scrolling text and descriptions, which the exporter rewraps anyway; prompts
  mark those entries (`rw`), other multi-line entries still keep their line
  count, and a speaker label line (`【Name】`, `Name:`) must stay on its own
  line. A 19-entry Codex batch now needs one call instead of eight or nine.

### Removed
- Removed the light theme and its setting.
- Removed the `.fairtl` file association: projects are folders, which Windows
  file associations cannot open.

### Security
- Removed webview permissions the interface does not use (`store:default`, which
  let page scripts read the legacy `api-keys.json`, plus unused dialog
  permissions) and `https:` from the `connect-src` content security policy.

## [0.9.2] - 2026-05-04

### Added
- Added Japanese-to-Spanish prompt guidance for natural RPG Maker dialogue,
  concise UI labels, Spanish punctuation, credits handling, and concrete
  Japanese calque examples.
- Added export QA hardening that restores RPG Maker asset references to the
  exact copied filenames when translated or normalized data changes whitespace
  width or ASCII/full-width forms.

### Fixed
- Preserved RPG Maker choice/plugin condition prefixes such as `if(s[12])` and
  `en(s[20])` during AI prompting and integrity validation.
- Wrapped long MV/MZ message translations into safe text-window lines and split
  oversized text into multiple message pages.
- Inserted a final choice prompt when wrapped multi-page dialogue would
  otherwise let choices appear before the player finishes reading.
- Patched exported prompt-based name-input plugins to use RPG Maker's name scene
  instead of unsupported browser `prompt()` calls.
- Translated visible plugin default menu fallbacks such as the game-exit command
  when plugin parameters are empty.
- Corrected contextual QA issues found in the RJ01568532 Spanish export,
  including Japanese punctuation drift, literal UI wording, typo patterns,
  compact choice labels, and malformed control characters.

## [0.9.1] - 2026-05-02

### Added
- Added `scripts/clean-fairtranslator-runs.ps1` for safe dry-run/apply hygiene of `D:\FairTranslatorRuns`.
- Added release notes for `v0.9.1` with the Codex QA-runner hardening scope.

### Changed
- Reused cached chunk plans across resumed provider runs when the source/context fingerprint is unchanged.
- Updated Codex CLI reasoning-effort invocation to the current `model_reasoning_effort` config key.
- Bounded Codex CLI provider calls with a five-minute timeout to prevent silent hangs.

### Fixed
- Retried malformed provider response shapes and isolated failing entries instead of aborting long translation campaigns.
- Made provider batch context and planner cache structures serializable for durable checkpoint reuse.

## [0.9.0] - 2026-03-31

### Added
- Initial public release for RPG Maker MZ, MV, VX Ace, VX, and XP workflows.
- `.fairtl` project persistence, glossary management, and translation status tracking.
- Batch translation through OpenAI, DeepL, Google Translate, and Codex.
- Structured external AI batch export/apply flows and QA helper binaries.
- Public project documentation in `README.md`, `docs/versioning-and-roadmap.md`, and `docs/releases/v0.9.0.md`.

### Changed
- Updated desktop package metadata to describe current RPG Maker engine support more accurately.
- Bumped the app version to `0.9.0`.
- Aligned CI and release workflows with the current repository branch and release scope.

### Fixed
- Cleared stale API provider `model` and `endpoint` overrides when saving empty values.
- Made legacy engine fallback detection deterministic for mixed extension folders.
- Aligned CI frontend job label with the checks that are actually executed.

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- Made OpenCode provider chunks adaptive up to 128 entries, with independent
  prompt and estimated-response token budgets instead of a fixed 16-entry cap.
- Made the headless OpenCode runner reuse the credential saved by FairTranslator
  in the operating-system secure store by default; key files are compatibility
  overrides only.

### Fixed
- Strengthened OpenCode readiness checks to require a valid structured
  translation and negotiate/cache the API family that satisfies the schema.
- Preserved completed checkpoints when planner-budget changes rebuild a chunk
  plan during resume.

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

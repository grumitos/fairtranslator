# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

# Versioning And Roadmap

## Release Starting Point

The first public release line for FairTranslator is `0.9.x`, starting with `0.9.0`.

That recommendation is valid only if the product is positioned honestly as:

> A desktop translator for RPG Maker games.

This repo is already beyond an early prototype:

- Import, save, reopen, and export flows exist
- The parser/exporter covers RPG Maker MZ, MV, VX Ace, VX, and XP
- Glossary support exists
- Batch translation exists
- External AI batch export/apply exists
- OpenAI, OpenCode Go, DeepL, Google, and Codex CLI provider flows exist

At the same time, the project is not yet broad enough to justify claims beyond the supported RPG Maker engine set.

## Why Not 1.0.0 Yet

Do not use `1.0.0` yet if you want that version to mean "stable, clearly supported, and honestly scoped."

The remaining gaps are mostly product and release maturity gaps:

- Real-world QA coverage across multiple shipped games per supported engine
- Stable release packaging and support policy
- Broader validation of export round-trips on legacy engines
- A clearer release stance for Codex sidecar bundling versus external CLI dependency
- Documentation and support expectations aligned with the actual engine matrix

## If The Scope Expands Beyond RPG Maker

If the product promise later becomes:

> Translate games beyond the supported RPG Maker engine set

then the current codebase will still need new engine adapters, fixtures, and QA outside the RPG Maker family.

## Practical Release Ladder

Use this versioning path:

- `0.9.0`: First public release for RPG Maker support
- `0.9.x`: Bug fixes, parser/exporter hardening, packaging cleanup, documentation alignment
- `1.0.0`: Stable RPG Maker release after real-game validation and release hardening
- `1.1+`: Incremental quality improvements, performance, UI polish, release automation
- `2.0.0` or a later major: Only if the product scope expands materially beyond RPG Maker and changes the support model

## Current Release Position

The repository is currently aligned on `0.9.2` across `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

`0.9.2` is still part of the `0.9.x` hardening line. It adds focused RPG Maker
MV/MZ export correctness and Japanese-to-Spanish QA hardening, including
choice/plugin condition preservation, message wrapping, name-input plugin
patching, asset-reference restoration, and visible plugin fallback translation.

## Current 0.9.x Hardening Baseline

As of 2026-04-25, the local hardening baseline includes:

- API keys stored in Windows Credential Manager through the Rust `keyring` integration, with legacy JSON-store migration on load.
- Explicit Tauri CSP instead of a null policy.
- Provider endpoint overrides constrained to HTTPS URLs without embedded credentials, query strings, or fragments.
- Frontend accessibility fixes for the New Project dialog, Settings sheet, Style Guide sheet, and provider API-key fields.
- Production Vite output split into stable vendor chunks, with stale generated Vite config artifacts removed from source control.
- Manual editing/search hot-path improvements for active-file grid/find views and resize persistence.
- `npm audit --audit-level=high` returning zero vulnerabilities after lockfile updates.

This does not make `1.0.0` automatic. The remaining blocker is still real-game validation breadth and release support maturity, not the absence of basic local build/test hardening.

## What 1.0.0 Should Mean

Before tagging `1.0.0`, the repo should have:

- Verified import/export round-trips for real projects in MZ, MV, VX Ace, VX, and XP
- Stable packaging instructions and repeatable release artifacts
- Clear public documentation for supported engines and provider requirements
- Regression fixtures for known parser/exporter edge cases
- A documented support policy for Codex and other providers

## Roadmap Toward Broader Engine Support

To move from "RPG Maker translator" toward "translator for RPG games in general", the next architectural steps are:

1. Separate engine detection, parsing, and export behind a more explicit adapter boundary.
2. Add parser/exporter pairs for non-RPG Maker engines instead of growing RPG Maker-specific logic as if it were generic.
3. Build fixture packs and QA campaigns per engine family.
4. Define a compatibility matrix so releases can state exactly what is supported.
5. Revisit version positioning only after support extends beyond the RPG Maker family in a repeatable way.

# FairTranslator

FairTranslator is a desktop application for translating supported game formats without locking core workflows behind subscriptions or hosted accounts. It scans a compatible game, extracts translatable text into a `.fairtl` project, lets you edit or batch-translate entries, and exports a translated copy without modifying the source folder.

This repository is private and proprietary. Source code access is limited to authorized collaborators. Commercial builds are intended to be distributed to customers under a paid perpetual end-user license rather than a subscription model.

## Current Scope

RPG Maker is the supported product family. Unity support is limited to one experimental compatibility profile; it is not a generic Unity translator.

- Supported engines: RPG Maker MZ, MV, VX Ace, VX, and XP
- Experimental engine profile: Unity (Loose JSON v0) for Windows Mono builds with exactly one flat localization JSON under `*_Data/StreamingAssets`
- Project format: `.fairtl`
- Translation workflows: manual editing, batch translation through built-in providers, and external AI batch export/apply
- Built-in providers: OpenAI, OpenCode Go, DeepL, Google Translate, and Codex
- Desktop stack: Tauri 2, Rust, React, and TypeScript
- Release posture: Windows-first at the moment

## What The App Already Does

- Detects supported game roots, including nested RPG Maker and Unity Loose JSON folders
- Extracts translatable text from database files, maps, common events, troops, and system terms where supported by the engine parser/exporter
- Stores translation progress, notes, statuses, and glossary terms inside the project
- Applies glossary guidance during provider-based translation
- Supports external AI workflows by exporting an editor-friendly translation sheet plus prompt guidance, then applying the structured JSON response
- Exports a translated copy of the game to a separate output directory

The exporter copies the original game to the target folder first and only writes translations into the copied output. The source game directory is not modified.

## Provider Notes

FairTranslator supports two provider styles today:

- Direct HTTP providers that use an API key: OpenAI, OpenCode Go, DeepL, Google Translate
- Codex through the Codex CLI runtime

Provider API keys are stored in the operating-system credential store on Windows. Non-secret provider settings, such as selected model, profile, organization/project identifiers, and endpoint override metadata, remain in the local app store. Legacy API keys found in the older JSON-backed store are migrated into secure storage when loaded and then removed from the JSON store.

Custom provider endpoint overrides are intentionally restricted: they must be HTTPS URLs without embedded credentials, query strings, or fragments. This keeps API keys from being sent to malformed or ambiguous endpoints.

For Codex, authentication is managed by the installed Codex CLI. FairTranslator does not implement ChatGPT OAuth itself and does not store ChatGPT OAuth tokens. The runtime can be resolved from:

1. A user-configured executable path
2. A bundled sidecar binary
3. `codex` from `PATH`

For the standard release workflow, assume option `1` or `3`. Sidecar builds are supported by the codebase, but they are not the default public release artifact.

## Honest Product Positioning

The truthful product promise today is:

> A desktop translator for supported RPG Maker games, with experimental Unity Loose JSON v0 support for a narrow Windows Mono layout.

What it should not claim yet:

- It is not a translator for arbitrary RPG engines or arbitrary Unity games
- Unity support does not cover AssetBundles, Addressables, serialized assets, managed DLL strings, IL2CPP, or platforms beyond Windows
- It does not ship with a generalized parser/exporter plugin system
- A Codex sidecar can be prepared for release builds, but the distribution story still needs release-policy decisions

The Unity Loose JSON v0 exporter follows the same source-safety rule as RPG Maker: it copies the game first, changes only the single recognized JSON in the output, and reopens the result for readback verification. Broader engine claims require additional parser/exporter pairs, real fixtures, and QA coverage before they are defensible.

## First Public Release

The first public release line is `0.9.x`, starting with `0.9.0`, which launched with positioning explicitly limited to RPG Maker support. Unity Loose JSON v0 remains experimental and does not broaden the stable support promise.

Why `0.9.0` is the right release point:

- The app already has an end-to-end workflow: import, edit, translate, review, save, and export
- It supports five RPG Maker engine variants
- It has a copy-first experimental Unity path for one strict loose-JSON layout
- It has multiple provider integrations, including a Codex runtime path
- The repo already contains Rust and frontend test coverage plus QA helper binaries

Why not `1.0.0` yet:

- The supported engine matrix still needs broader validation on real projects before being called fully stable
- Release hardening still needs broader validation on real shipped games per supported engine
- Packaging, distribution policy, and support expectations should be clearer before calling the surface stable

## Path To 1.0

The practical gate for `1.0.0` should be:

- Repeated import/export validation on multiple real projects for MZ, MV, VX Ace, VX, and XP
- Continued real-build validation of Unity Loose JSON v0 before considering it stable
- A stable release packaging story for the supported desktop platform
- Clear provider support and failure-recovery expectations in the UI and docs
- Regression fixtures for parser and exporter edge cases
- Documentation that matches the real engine matrix and release support policy

## Development

Requirements:

- Node.js
- Rust toolchain
- Tauri prerequisites for your platform
- Windows release/build validation: Visual Studio Build Tools 2022 with the C++ toolchain

Common commands:

```bash
npm install
npm run tauri dev
npm run test:run
cargo test --manifest-path src-tauri/Cargo.toml --no-fail-fast
```

On Windows, if `cargo` cannot find `link.exe`, run Rust/Tauri validation from a Visual Studio developer environment:

```powershell
& $env:ComSpec /c '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 && cargo test --manifest-path src-tauri\Cargo.toml --no-fail-fast'
```

Release-quality local checks used for the current hardening pass:

```powershell
npx tsc --noEmit
npm run test:run
npm run build
npm audit --audit-level=high
cargo check --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml --no-fail-fast
npm run tauri -- build
```

QA run hygiene for local campaign artifacts:

```powershell
.\scripts\clean-fairtranslator-runs.ps1
.\scripts\clean-fairtranslator-runs.ps1 -Apply
```

Optional Codex sidecar preparation:

```bash
npm run prepare:codex-sidecar
npm run tauri:build:codex-sidecar
```

## Documentation

- Release/version strategy: [docs/versioning-and-roadmap.md](./docs/versioning-and-roadmap.md)
- Run hygiene: [docs/run-hygiene.md](./docs/run-hygiene.md)
- AI agent translation workflow: [docs/AI_AGENT_TRANSLATION_WORKFLOW.md](./docs/AI_AGENT_TRANSLATION_WORKFLOW.md)
- Experimental Unity support boundary: [docs/UNITY_TRANSLATION_SUPPORT.md](./docs/UNITY_TRANSLATION_SUPPORT.md)
- Latest release notes: [docs/releases/v0.9.2.md](./docs/releases/v0.9.2.md)
- Previous release notes: [docs/releases/v0.9.1.md](./docs/releases/v0.9.1.md)
- Initial release notes: [docs/releases/v0.9.0.md](./docs/releases/v0.9.0.md)
- Changelog: [CHANGELOG.md](./CHANGELOG.md)

## Licensing

- Source code in this repository is proprietary. See [LICENSE](./LICENSE).
- Commercial binary distributions should include [EULA.md](./EULA.md).
- Third-party components keep their own licenses. See [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

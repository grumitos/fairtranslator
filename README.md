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
- Fits translated text to each engine's windows: messages are rewrapped to the real window width (face graphics, font size, message plugins and the game's own text are taken into account) and split into extra pages when needed (native paging on MV/MZ/VX Ace/VX, new show-text commands on XP). Whatever still does not fit, such as choices, one-line help windows or long names, is listed in the export result and in `qa_validate_translation --json` (`textFit`)
- Saves projects to `Documents\FairTranslator\<name>.fairtl` by default, always outside the game folder; "Save as" picks another location

The exporter copies the original game to the target folder first and only writes translations into the copied output. The source game directory is not modified. Packed RPG Maker games (`Game.rgssad`, `.rgss2a` or `.rgss3a` without a `Data` folder) are not supported: unpack them first.

## Provider Notes

FairTranslator supports two provider styles today:

- Direct HTTP providers that use an API key: OpenAI, OpenCode Go, DeepL, Google Translate
- Codex through the Codex CLI runtime

Provider API keys are stored in the operating-system credential store on Windows. Non-secret provider settings, such as selected model, profile, organization/project identifiers, and endpoint override metadata, remain in the local app store. Legacy API keys found in the older JSON-backed store are migrated into secure storage when loaded and then removed from the JSON store.

Custom provider endpoint overrides are intentionally restricted: they must be HTTPS URLs without embedded credentials, query strings, or fragments. This keeps API keys from being sent to malformed or ambiguous endpoints.

For Codex, authentication is managed by the installed Codex CLI. FairTranslator does not implement ChatGPT OAuth itself and does not store ChatGPT OAuth tokens. The runtime can be resolved from:

1. A user-configured executable path
2. A sidecar binary bundled next to the app
3. `codex` from `PATH`
4. Debug builds only: the ignored `src-tauri/binaries/codex-*` sidecar of the checkout they were compiled from

For the standard release workflow, assume option `1` or `3`. Sidecar builds are supported by the codebase, but they are not the default public release artifact. Release builds never look at the checkout folder.

FairTranslator keeps a standard Codex CLI install up to date: at startup it checks the npm registry and runs `codex update` when a newer version exists ("Update on startup" in Settings, on by default; the change applies on the next start). It never touches a bundled sidecar or a custom executable path, and a translation run that needs Codex waits until the check finishes.

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

- Node.js 22 LTS and npm 11.10 or newer. `package.json` declares `devEngines`, so every npm command in this repository fails with `EBADDEVENGINES` on npm 10 (the version bundled with Node 22). `.npmrc` also relies on `min-release-age`, which older npm ignores. Upgrade with `npm install --global npm@11.16.0`, the version CI uses.
- Rust toolchain
- Tauri prerequisites for your platform
- Windows release/build validation: Visual Studio Build Tools (2022 or newer) with the C++ toolchain

Common commands:

```bash
npm install
npm run tauri dev
npm run test:run
cargo test --manifest-path src-tauri/Cargo.toml --no-fail-fast
```

On Windows, if `cargo` cannot find `link.exe`, run Rust/Tauri validation from a Visual Studio developer environment. `vswhere` finds the installed version (2022, 18, …):

```powershell
$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
& $env:ComSpec /c "`"$vs\Common7\Tools\VsDevCmd.bat`" -arch=x64 -host_arch=x64 && cargo test --manifest-path src-tauri\Cargo.toml --no-fail-fast"
```

Release-quality local checks used for the current hardening pass:

```powershell
npx tsc --noEmit
node src/styles/kit/check.mjs --brand-from src/styles/brand.css
npm run test:run
npm run build
npm audit --audit-level=high
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml --no-fail-fast --locked
npm run tauri -- build
```

Interface and design system:

- The UI is dark only. Colors, type, radii and motion come from the shared design kit `@nosky/design-kit` (also used by ChronoRift), vendored in `src/styles/kit` with its `KIT_VERSION`. Do not edit the vendored files; FairTranslator's accent lives in `src/styles/brand.css`.
- Keep the Tauri family aligned: the `tauri` crate and `@tauri-apps/api`/`@tauri-apps/cli` must share major.minor (the CLI refuses mismatches), and each plugin crate matches its npm package. `.npmrc` rejects npm releases younger than 7 days, so move both sides together once the npm packages qualify. The Tauri crates are pinned with `=` and their transitive dependencies are held by `Cargo.lock`; use `--locked`.
- Update the vendored copy from a kit checkout with `node <design-kit>/scripts/sync.mjs src/styles/kit --brand-from src/styles/brand.css`, then run the kit check above. `src/styles/kit/DESIGN.md` holds the rules (closed list of accent uses, monochrome logos, copy limited to labels and data).
- To review the UI without Tauri, run `npm run dev` and open `http://localhost:1420/?visualDemo=1`. It loads a sample project; `&panel=batch|context|styleGuide|settings|export|newProject` opens a panel directly. Development builds only.

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

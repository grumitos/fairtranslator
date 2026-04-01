# FairTranslator — Copilot Instructions

## Overview

FairTranslator is a desktop app for translating RPG Maker MV/MZ games. It replaces Translator++ with a modern UI and no paywalled features. Users connect their own API keys (OpenAI, DeepL, Google Translate) to translate game text via a grid editor. Built with Tauri 2.x (Rust) + React + TypeScript.

## Tech Stack

- **Runtime:** Tauri 2.x, Rust edition 2021 (1.77+). Frontend served by Vite 6.
- **Frontend:** React 19, TypeScript 5.5+, Zustand 5 (state), TanStack Table v8 (grid), react-i18next (i18n).
- **UI:** shadcn/ui (Radix primitives) + Tailwind CSS 4. Install components via `npx shadcn@latest add <component>`.
- **Testing:** Vitest + @testing-library/react (frontend), `cargo test` (Rust).

## Project Structure

```
src-tauri/
  src/
    lib.rs                  # tauri::Builder, command registration, state init
    main.rs                 # Desktop entry — calls lib::run(). Do not modify.
    commands/               # One file per domain: project.rs, translation.rs, glossary.rs, export.rs
    models/                 # Serde structs for IPC + persistence
    parser/                 # rpgmaker.rs — read/write MV/MZ /data/*.json
    services/               # provider.rs (trait), openai.rs, deepl.rs, google.rs, persistence.rs, exporter.rs
    error.rs                # AppError enum (thiserror + Serialize)
  capabilities/default.json # Tauri v2 permissions — only built-in plugin perms go here
  tauri.conf.json
src/
  components/
    ui/                     # shadcn/ui primitives (Button, Dialog, Sheet, Tabs, Progress, etc.)
    editor/                 # TranslationGrid.tsx, EditEntryDialog.tsx, StatusBadge.tsx,
                            # FindReplaceBar.tsx, HighlightedText.tsx
    layout/                 # AppShell.tsx, Sidebar.tsx, Toolbar.tsx, StatusBar.tsx, WelcomeScreen.tsx,
                            # NewProjectDialog.tsx, SettingsPanel.tsx, GlossaryPanel.tsx, BatchTranslatePanel.tsx
  hooks/                    # useProject.ts, useApiKeys.ts, useGlossary.ts, useTranslation.ts
  stores/                   # projectStore.ts, editorStore.ts, settingsStore.ts
  lib/
    ipc.ts                  # Typed invoke() wrappers — ALL Tauri calls go through here
    i18n.ts                 # i18next configuration
    utils.ts                # cn() helper, formatters
  locales/
    en/                     # common.json, editor.json, settings.json, glossary.json
    es/                     # Same namespaces
  types/
    domain.ts               # TS interfaces mirroring Rust models
  App.tsx
  main.tsx
```

## Rust Backend

- **Errors:** Define `AppError` in `error.rs` with `#[derive(Debug, thiserror::Error, serde::Serialize)]`. Use `#[serde(tag = "kind", content = "message")]`. Every command returns `Result<T, AppError>`.
- **Commands:** `#[tauri::command] async fn cmd_name(state: tauri::State<'_, AppState>, ...) -> Result<T, AppError>`. Register all in `lib.rs` via `tauri::generate_handler![...]`. Prefix by domain: `project_open`, `project_save`, `translate_batch_start`, `glossary_add`. Currently 15 commands registered (6 project/export + 4 apikey + 4 glossary + 1 batch translate).
- **Serde:** `#[serde(rename_all = "camelCase")]` on all IPC structs to match TypeScript naming.
- **Parser:** `parser/rpgmaker.rs` reads MV/MZ `/data/*.json` files. Translatable text lives in: event command code 401 (Show Text) parameters, `name`/`description`/`profile` fields in Actors/Items/Skills/etc., and `System.json` terms. Preserve the full original JSON structure — only mutate translatable string fields on export.
- **HTTP:** `reqwest` (with `json` feature) + `tokio` (with `fs`, `sync`, `time` features) + `async-trait` for async translation provider trait. Three providers implemented: OpenAI, DeepL, Google Translate.
- **Translation providers:** `services/provider.rs` defines `TranslationProvider` async trait with `translate()` and `name()`. Each provider (`openai.rs`, `deepl.rs`, `google.rs`) implements this trait. Factory function `create_provider()` in `commands/translation.rs` selects the provider.
- **Batch translation:** `translate_batch_start` command spawns an async task, processes entries in chunks, and emits `translation:progress` / `translation:complete` events via `tauri::Emitter`. Frontend listens with `@tauri-apps/api/event`.
- **API key storage:** Uses `tauri-plugin-store` v2 — `app.store("api-keys.json")` with `.set()/.get()/.delete()/.save()`. Keys stored as `serde_json::Value`. Validation hits the provider's API with a minimal request.

## Frontend

- **Components:** PascalCase filenames (`TranslationGrid.tsx`). Keep components under 200 lines; extract sub-components into the same folder.
- **State:** One Zustand store per domain in `src/stores/`. Use `immer` middleware for nested updates. Stores hold client state only — no `invoke()` calls inside stores.
- **IPC layer:** `src/lib/ipc.ts` exports typed wrappers around `invoke` from `@tauri-apps/api/core`. Example: `export const projectOpen = (path: string) => invoke<Project>("project_open", { path });`. Components and hooks call these wrappers, never raw `invoke`.
- **Hooks:** `use`-prefixed, in `src/hooks/`. Combine Zustand selectors + IPC calls + loading/error state. Example: `useProject()` returns `{ project, open, save, isLoading, error }`.

## i18n

- Default language: `es`. Fallback: `en`. Namespaces: `common`, `editor`, `settings`, `glossary`.
- All user-facing strings use `t()`. Zero hardcoded display text.
- Key format: `namespace:area.element.state` — e.g., `t("editor:toolbar.batchTranslate")`, `t("settings:apiKeys.openai.placeholder")`.
- Wrap pages/sections in `<Suspense>` to handle async namespace loading.

## UI

- shadcn/ui components live in `src/components/ui/`. Tailwind only — no CSS modules or styled-components.
- Dark mode via Tailwind `class` strategy. Persist preference in `settingsStore`. Toggle applies/removes `.dark` on `<html>`.
- Grid: TanStack Table with `@tanstack/react-virtual` for row virtualization (RPG Maker maps can have 10k+ entries).
- Use `cn()` from `src/lib/utils.ts` for conditional class merging (`clsx` + `tailwind-merge`).

## Domain Model

```typescript
type Engine = "mv" | "mz";
type EntryStatus = "untranslated" | "draft" | "reviewed" | "final";
type ProviderType = "openai" | "deepl" | "google";

interface Project {
  id: string; name: string; sourceLang: string; targetLang: string;
  engine: Engine; gameDir: string; files: TranslationFile[];
  glossary: GlossaryTerm[]; createdAt: string; updatedAt: string;
}
interface TranslationFile { relativePath: string; entries: TranslationEntry[]; }
interface TranslationEntry {
  id: string; fileContext: string; original: string;
  translation: string; status: EntryStatus; notes: string;
}
interface GlossaryTerm { source: string; target: string; caseSensitive: boolean; }
interface ProviderConfig {
  type: ProviderType; apiKey: string; model?: string;
  endpoint?: string;                              // Custom/self-hosted LLM URL
  rateLimit?: { requestsPerMinute: number };      // Per-provider throttling
}
```

Mirror these as Rust structs in `src-tauri/src/models/` with identical field names (camelCase via serde).

## Project Persistence

- Projects are saved as a `.fairtl` directory:
  ```
  MyGame.fairtl/
    project.json        # Metadata: name, engine, languages, glossary, timestamps
    entries/
      Map001.json       # TranslationEntry[] for each source game file
      Actors.json
      System.json
  ```
- Directory-based (not zipped) for git-friendliness and easy diffing.
- `project_save` writes atomically: write to a temp directory first, then rename. Never write directly to the live project path.
- Include `schemaVersion: number` in `project.json` for forward compatibility.

## Export (Phase 4)

- **Exporter** (`services/exporter.rs`): Reads original game JSON from `project.game_dir`, injects translations into a copy written to `output_dir`. Original files are never modified.
- Translations are applied only when `status != Untranslated` and `translation` is non-empty.
- Multiline text (codes 401/405) distributes translated lines 1:1 across commands. Excess lines are joined into the last command; fewer lines clear excess commands.
- The exporter imports parser constants (`DB_TEXT_FIELDS`, event command codes) to avoid duplication.
- `export_project` Tauri command takes `output_dir` string and returns `ExportResult { filesExported, entriesApplied, totalEntries }`.
- Frontend shows a directory picker dialog then displays a success toast with export stats.

## AI Translation (Phase 5)

- **Provider trait:** `TranslationProvider` in `services/provider.rs` — async trait with `translate(request: TranslationRequest) -> Result<TranslationResponse>` and `name() -> &'static str`.
- **OpenAI** (`services/openai.rs`): Uses Chat Completions API (`/v1/chat/completions`). Default model: `gpt-4o-mini`. System prompt instructs JSON output with RPG Maker control code preservation. Processes entries in chunks (50 per request). Handles markdown fence stripping from responses. Supports custom `model` and `endpoint` for self-hosted LLMs.
- **DeepL** (`services/deepl.rs`): Uses v2 translate API. Auto-detects free vs pro keys (`:fx` suffix). Language code normalization (e.g., `en` → `EN`, `pt` → `PT-BR`). Sends entries as batch texts.
- **Google** (`services/google.rs`): Uses Cloud Translation v2 API. Sends entries as individual `q` parameters. Language codes used as-is.
- **Batch translation flow:** `translate_batch_start` spawns `tokio::spawn` task. Processes selected files sequentially, entries in provider-appropriate chunks. Emits `translation:progress` events with `{ filesProcessed, totalFiles, entriesTranslated, totalEntries, currentFile }`. Emits `translation:complete` when done. Glossary terms are passed to the provider for context-aware translation.
- **API key commands:** `apikey_save`, `apikey_load`, `apikey_delete`, `apikey_validate`. Validation sends a minimal API request to verify the key works.
- **Glossary commands:** `glossary_add`, `glossary_update`, `glossary_remove`, `glossary_list`. Stored in the project's glossary array. Glossary terms are included in OpenAI system prompts and as context hints for other providers.
- **Frontend panels:** Settings (Sheet + Tabs for API keys and appearance), Glossary (Sheet with CRUD table), BatchTranslate (Sheet with provider selector, file checkboxes, progress bar). All in `src/components/layout/`.
- **Frontend hooks:** `useApiKeys` (CRUD + validation), `useGlossary` (CRUD), `useTranslation` (batch start + Tauri event listeners for progress/complete).
- **Test coverage:** 43 lib tests total (11 parser + 7 persistence + 12 exporter + 7 OpenAI + 5 DeepL + 4 Google = 46, adjusted for actual count of 43).

## Quality of Life (Phase 6)

- **Find & Replace** (`FindReplaceBar.tsx`): Collapsible bar toggled via Ctrl+H or toolbar button. Supports regex, case-sensitive, and search target (original/translation/both). Replace one or all. Exports `buildFindRegex()` utility used by `TranslationGrid.tsx` for match highlighting. State managed in `editorStore.ts` (`showFindReplace`, `findQuery`, `replaceText`, `matchCase`, `useRegex`, `searchTarget`, `currentMatchIndex`). Rendered conditionally in `AppShell.tsx`.
- **Highlighted Text** (`HighlightedText.tsx`): Extracted component that renders text with `<mark>` tags for regex matches. Used by `TranslationGrid` to highlight find matches in grid rows (yellow background on matching rows).
- **Visual Progress**: `Sidebar.tsx` shows a global progress bar and per-file mini progress bars with color coding (red→orange→yellow→green based on translation completion %). `StatusBar.tsx` shows an inline progress bar with the same color coding.
- **Word Wrap**: `settingsStore.ts` has `wordWrapColumn` (0 = disabled). `TranslationGrid.tsx` reads it and applies `wrapStyle` to cells. `SettingsPanel.tsx` appearance tab has a numeric input for configuring the column limit.
- **Dark Mode Quick Toggle**: Sun/Moon icon button in `Toolbar.tsx` right side. Cycles between light/dark themes instantly.
- **i18n Keys Added**: `findReplace.*` and `toolbar.toggleTheme` in editor namespace. `wordWrap.*` in settings namespace. Both `es` and `en` locales.

## Packaging & Distribution (Phase 7)

- **Bundle targets:** NSIS installer (`.exe`) + MSI for Windows. Configured in `tauri.conf.json` `bundle` section.
- **Identifier:** `com.fairtranslator.desktop` — avoid `.app` suffix (conflicts with macOS bundle extension).
- **File associations:** `.fairtl` directories registered as `application/x-fairtranslator-project`.
- **NSIS config:** `installMode: "both"` (per-user or per-machine), language selector with Spanish + English.
- **Metadata:** publisher, copyright, category (`DeveloperTool`), short/long descriptions all set in `tauri.conf.json`.
- **License:** MIT license in `LICENSE` file at project root.
- **CI/CD:** Two GitHub Actions workflows in `.github/workflows/`:
  - `ci.yml` — runs on push/PR to `main`: frontend tsc check (ubuntu) + Rust check + lib tests (windows).
  - `release.yml` — runs on `v*` tag push: full build via `tauri-apps/tauri-action`, creates draft GitHub Release with NSIS + MSI artifacts.
- **Build command:** `npm run tauri build` runs `tsc -b && vite build` then Rust release compilation + bundling.
- **tsconfig.node.json:** Required `emitDeclarationOnly: true` alongside `composite: true` for `tsc -b` to work with `allowImportingTsExtensions`.
- **@types/node:** Added as devDependency for `vite.config.ts` (uses `path`, `process`, `__dirname`).

## Error Handling (Frontend)

- `ErrorBoundary` component (`src/components/ErrorBoundary.tsx`) wraps `AppShell` in `App.tsx` to catch rendering crashes gracefully.
- Extracted `StatusBadge` component (`src/components/editor/StatusBadge.tsx`) keeps `TranslationGrid.tsx` under the 200-line guideline.
- Extracted `NewProjectDialog` component (`src/components/layout/NewProjectDialog.tsx`) is shared between `WelcomeScreen` and `Toolbar` — avoids code duplication.

## API Key Storage

- Use `tauri-plugin-store` for encrypted local storage in the app's config directory.
- Keys are stored per-provider — never inside project files (projects must be shareable without leaking keys).
- Keys must never be logged, included in error reports, or sent to any endpoint other than the provider's own API.

## Testing

- **Frontend:** Vitest in `src/__tests__/`. Test stores with mock state. Mock `@tauri-apps/api/core` for IPC wrapper tests. Test grid rendering with Testing Library.
- **Rust:** `#[cfg(test)] mod tests` in each source file. Integration tests in `src-tauri/tests/`. Critical: test parser round-trip — read JSON, extract entries, reinsert translations, write JSON, verify structure is preserved.

## Dev Commands

```bash
npm run tauri dev              # Dev server with hot reload
npm run tauri build            # Production build
npm run test                   # Vitest (frontend)
cd src-tauri && cargo test     # Rust tests
npx shadcn@latest add <name>  # Install shadcn component
```

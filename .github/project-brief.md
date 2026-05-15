# project-brief: FairTranslator

## Product Positioning

FairTranslator is a local desktop application for translating RPG Maker games.
It is positioned as a modern alternative to Translator++ for users who want a
straightforward local workflow, direct provider credentials, and no artificial
credit system between them and their translation providers.

The defensible product promise today is narrow and explicit:

> A desktop translator for RPG Maker games.

Do not describe the product as a general-purpose RPG translator until non-RPG
Maker adapters, fixtures, and QA evidence exist.

## Current Implemented Scope

- Supported engines: RPG Maker MZ, MV, VX Ace, VX, and XP.
- Project format: directory-based `.fairtl` projects.
- Core workflow: import supported game root, edit extracted text in a grid,
  save/reopen project state, and export a translated copy without modifying the
  source game.
- Translation workflows: manual editing, glossary-assisted batch translation,
  built-in providers, and structured external AI batch export/apply.
- Providers: OpenAI, DeepL, Google Translate, and Codex through the local Codex
  CLI/runtime.
- UI: English and Spanish localization.
- Packaging posture: Windows-first Tauri desktop builds.

## Product Principles

- Never modify the source game directory during export.
- Do not lose project edits silently; destructive project replacement requires a
  discard decision.
- Keep `.fairtl` schema changes explicit and backward-compatible.
- Treat provider output as untrusted until integrity checks pass.
- Protect API keys; projects must stay shareable without provider secrets.
- Keep provider capability copy honest: OpenAI/Codex have the full contextual
  orchestration path; DeepL/Google are more limited.

## Technical Shape

- Frontend: React 19, TypeScript, Vite 6, Zustand, TanStack Table/Virtual,
  Tailwind CSS 4, Radix/shadcn-style components, react-i18next.
- Desktop/backend: Tauri 2, Rust 2021, async provider integrations, and Rust
  parser/exporter services.
- Secrets: Windows Credential Manager through `keyring`; `tauri-plugin-store`
  keeps only non-secret provider settings and migrates legacy JSON API keys on
  load.
- Validation: Vitest/Testing Library for frontend behavior, Rust unit tests for
  parser/services/QA helpers, plus Tauri build validation for release gates.

## Competitive Context

Translator++ remains the reference tool for this domain because of its broad
engine support and grid-based editing model. FairTranslator competes by keeping
the local workflow clear, modern, and direct: users bring their own provider
keys or local Codex runtime instead of buying translation add-ons through a
credit/subscription layer.

## Release Boundary

The current release line is `0.9.x`. `1.0.0` should wait until import/export
round trips and smoke testing have broader real-game coverage across the
supported RPG Maker engines, and until packaging/support expectations are clear.

# AI Agent Translation Workflow

This runbook is the authoritative flow for an external AI agent translating a
game with FairTranslator. It is intentionally operational: use the commands,
inspect the generated artifacts, and stop only when the output is playable and
validated.

## Scope

Supported product flow today:

- RPG Maker MV, MZ, VX Ace, VX, and XP through FairTranslator parsers/exporters.
- Experimental Unity (Loose JSON v0) for the exact Windows Mono layout defined
  in `docs/UNITY_TRANSLATION_SUPPORT.md`.
- UI provider translation through OpenAI, OpenCode Go, Codex CLI, DeepL,
  Google, or the External AI export/apply flow.
- Headless QA runners under `src-tauri/src/bin`. `qa_translate_provider`
  currently supports Codex CLI and OpenCode Go. Use `qa_prepare`/`qa_apply` for
  external AI agents and the app UI for OpenAI, DeepL, and Google runs.

Unity support is not generic: AssetBundles, Addressables, DLL strings, IL2CPP,
and platforms beyond Windows remain unsupported. Read
`docs/UNITY_TRANSLATION_SUPPORT.md` before inventorying or promising Unity work.

## Inputs

Required:

- Original game folder. Prefer the folder containing the launcher, but the
  resolver can handle nested supported game roots.
- Source and target language.
- Provider credentials available locally. Never print keys.
- Workspace outside the source game, usually under `D:\FairTranslatorRuns`.

Recommended:

- One short style guide for names, honorifics, tone, and UI length.
- A note identifying whether the deliverable should be a technical translated
  tree, a playable copy, or both.

## Fast Path

Inventory first:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_inventory -- --json "<game-folder>"
```

Translate with Codex CLI:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_translate_provider -- `
  --provider codex `
  --game-dir "<game-folder>" `
  --workspace "D:\FairTranslatorRuns\<run-name>" `
  --source-lang "Japanese" `
  --target-lang "es" `
  --model "<dynamic-or-configured-model>" `
  --strict-target-language
```

Translate with OpenCode Go:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_translate_provider -- `
  --provider opencode `
  --game-dir "<game-folder>" `
  --workspace "D:\FairTranslatorRuns\<run-name>" `
  --source-lang "Japanese" `
  --target-lang "es" `
  --api-key-file "<api-key-file>" `
  --endpoint "https://opencode.ai/zen/go/v1" `
  --model "<model-from-/models>" `
  --strict-target-language
```

Resume the same command with the same `--workspace`. The runner reloads
`project.fairtl`, refreshes newly extractable entries, skips completed entries,
and checkpoints after successful chunks.

## Required Checks

After translation:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_validate_translation -- `
  --project "D:\FairTranslatorRuns\<run-name>\project.fairtl" `
  --strict-unchanged `
  --json
```

The run is not done until:

- `complete` is `true`.
- `missingTranslations`, `sourceScriptResidue`, and strict unchanged errors are
  zero, except for manually justified names/codes.
- `translation_failures.json` is absent or fully resolved.
- Exported JSON/MARSHAL files can be parsed by the app.
- A playable copy exists when the original game has a launcher outside the data
  root. For nested MV/MZ `www` exports, the QA runner creates `playable-es`.

## Output Contract

Keep:

- `project.fairtl`
- `qa_translate.status.json`
- `qa_translate.progress.log`
- `translation_failures.json`, when present
- `export-es`
- `playable-es`, when created
- final notes or playtest reports

Do not hand off only a bare translated data tree when the user asked to play the
game. Prefer the playable folder when it exists.

## Repair Loop

If strict validation fails:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_translate_provider -- `
  --provider <same-provider> `
  --game-dir "<game-folder>" `
  --workspace "D:\FairTranslatorRuns\<run-name>" `
  --repair-coverage `
  --strict-target-language `
  <same auth/model options>
```

Then rerun `qa_validate_translation`.

## External AI Flow

Use when the provider is outside the app or when a separate agent should perform
the translation:

- UI path: `Batch translation -> External AI`
- IPC: `translate_ai_batch_export`, then `translate_ai_batch_apply`
- Headless preparation: `qa_prepare`
- Headless apply/export/reopen: `qa_apply`

Headless preparation:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_prepare -- `
  --workspace "D:\FairTranslatorRuns\<run-name>" `
  --source-lang "Japanese" `
  --target-lang "es" `
  --chunk-size 200 `
  "<game-folder>"
```

For each sample, `qa_prepare` writes:

- `manifest.json`: authoritative list of batches, paths, and the versioned source-inventory fingerprint.
- `batches\<n>.prompt.txt`: prompt to give the external agent.
- `batches\<n>.entries.json`: source/reference entries for inspection.
- `batches\<n>.response.template.json`: response skeleton.

The external agent must write a sibling file named
`<same-name>.response.json`, replacing `.response.template.json` with
`.response.json`. Then apply:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin qa_apply -- `
  --manifest "D:\FairTranslatorRuns\<run-name>\<sample>\manifest.json" `
  --status reviewed
```

Rules:

- `qa_apply` rejects legacy/unknown fingerprints and any project or current source-inventory drift before applying responses.
- The AI response must echo the project and batch IDs.
- It must return every requested entry exactly once.
- Legacy bare arrays are accepted only for older batches without expected IDs.
- `skip: true` is a candidate exclusion, not final QA truth.
- Do not rewrite the source text field. Fill only translation/status/notes fields
  defined by the template.

## Known Edge Cases

- RPG Maker MV/MZ games may be nested under `www`; deliver `playable-es` if the
  launcher lives one level above.
- Plugins can store visible defaults in `js/plugins.js` or plugin source files.
- Visible text can be hidden in `Control Variables` quoted strings, name-input
  plugins, pictures, or custom message plugins.
- Speaker names may require their own line. Do not flatten title/name lines into
  dialogue.
- Asset references must not be translated; export normalizes copied asset names
  where safe.
- Image text is a separate OCR/asset-editing pass and must be reported as such.

## Definition Of Done

An agent can mark the translation done only with current evidence for:

- Import/inventory succeeded.
- Translation coverage passed.
- Export succeeded.
- Playable output exists when applicable.
- At least a smoke launch or parse-level validation ran.
- Any excluded/unsafe text class is documented.

# Product Hardening Spec

Status: active implementation plan (2026-07-10)

## Objective

Make FairTranslator safer and clearer across the complete workflow:

1. discover the correct game and translatable text;
2. translate through a configured provider or an external AI batch;
3. review the result deliberately;
4. export only to a copied destination with enough evidence to trust it.

This plan also delivers the first intentionally narrow Unity path without
presenting unsupported Unity formats as compatible.

## Product facts and constraints

- RPG Maker MZ, MV, VX Ace, VX and XP remain the supported engines.
- Source game folders are immutable. Every export writes to a separate target.
- Provider models come from discovery or active local configuration; model IDs
  are never hardcoded.
- Credentials and authentication file contents never enter logs or UI payloads.
- Provider prompts and responses use one compact JSON contract with strict
  response mapping, completeness checks and recovery for malformed output.
- Unity support starts only where parser and exporter are symmetric and tested.
  AssetBundles, Addressables, managed DLL strings, IL2CPP, encrypted content and
  mobile/WebGL builds remain unsupported in this iteration.

## Prioritized increments

### 1. Trust boundaries and tooling

- Reject an external-AI response when its project does not match the project
  currently open.
- Require complete, unique coverage of the exported response map; an empty
  translation is valid only when the model explicitly marks the entry skipped.
- Do not overwrite reviewed/final work through a stale external response without
  an explicit product rule and test.
- Upgrade the existing frontend tooling within compatible release lines until
  `npm audit` reports no known vulnerabilities.
- Restore executable tests around command-layer safety without weakening the
  production command boundary.

### 2. Review and export UX

- Warn before exporting unfinished or draft work, before opening the destination
  picker, and let the user jump to drafts.
- Keep the complete export result and all warnings visible until dismissed.
- Protect unsaved dialog edits from Escape, close and outside-click dismissal.
- Allow a running provider batch to be cancelled between requests and expose a
  distinct cancelled completion state.

### 3. RPG Maker recognition and writeback

- Remove unsafe global source-text replacement in JavaScript plugin sources.
- Treat legacy translatable byte strings as UTF-8 only: invalid bytes fail with
  the entry ID and byte offset before entries or exports are produced. Shift-JIS
  and other legacy encodings are not inferred or silently converted.
- Skip non-UTF-8 plugin source defaults because they are optional metadata;
  never inject replacement characters into otherwise valid `plugins.js` data.
- Cover omitted system vocabulary and MZ speaker-name commands with parser and
  exporter fixtures.
- Detect source drift at stable entry IDs before replacing copied game content.

### 4. Unity narrow slice

- Detect a desktop Unity root through a single `*_Data` directory.
- Accept one unambiguous UTF-8 JSON localization dictionary under
  `StreamingAssets` whose path is clearly named for localization.
- Use JSON Pointer-derived stable entry IDs and scalar string values only.
- Copy the whole source game, verify each original value, patch only the copy,
  reopen the JSON and verify translated values.
- Reject IL2CPP-only content, multiple candidates, nested/non-dictionary JSON,
  unsupported encodings and source drift with actionable errors.
- Add `Engine::Unity` only when detection, parsing, export, readback and source
  immutability tests all pass together.

### 5. Prompt-contract hardening

- Add regression coverage for delimiter-like and instruction-like game text so
  untrusted source text remains data rather than prompt instructions.
- Record representative compact-JSON payload size and round-trip behavior.
- Keep one versioned compact-JSON contract across integrated providers and the
  external-agent flow so validation and recovery do not diverge by transport.

## Success criteria

- Targeted tests demonstrate each changed failure mode before and after its fix.
- Frontend test, typecheck/build and audit gates pass.
- Rust library, command-focused and QA tests pass; ignored campaign tests remain
  reported separately rather than described as complete.
- Export tests prove the source folder remains byte-identical.
- UI copy in English and Spanish describes unfinished, cancelled, unsupported
  and completed states truthfully.
- Documentation does not claim general Unity support.

## Non-goals

- Modifying a source game in place.
- Generic Unity asset-container or binary patching.
- Guessing through encrypted/obfuscated formats.
- Broad dependency major-version upgrades unrelated to a verified defect.
- Completing the local Buebito one-hour playtest without its external campaign
  artifacts and runtime evidence.

## Primary references

- Unity Streaming Assets: <https://docs.unity3d.com/6000.0/Documentation/Manual/StreamingAssets.html>
- Unity Text Assets: <https://docs.unity3d.com/6000.0/Documentation/Manual/class-TextAsset.html>
- Vite release policy: <https://vite.dev/releases>

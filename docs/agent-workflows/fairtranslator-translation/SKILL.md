---
name: fairtranslator-translation
description: Run a full FairTranslator game translation campaign from import through playable export, validation, repair, and handoff.
---
# FairTranslator Translation

Use this skill when translating a game with FairTranslator or when validating a
translated output.

## Flow

1. Read `docs/AI_AGENT_TRANSLATION_WORKFLOW.md`.
2. Run `qa_inventory` on the selected game folder.
3. Choose the provider/model from dynamic provider discovery or existing local
   config. Do not hardcode model IDs.
4. For headless provider translation, use `qa_translate_provider` only with
   Codex CLI or OpenCode Go. OpenCode uses the credential saved by the app unless
   an explicit compatibility key file is supplied. Use the app UI for OpenAI,
   DeepL, and Google.
5. For external AI translation, run `qa_prepare`, translate every
   `.response.template.json` into a sibling `.response.json`, then run
   `qa_apply`.
   On interruption, rerun `qa_prepare` with the identical workspace and
   options, keep every response that validates completely, and continue at the
   first missing or invalid response. Publish each response through a temporary
   sibling plus rename so a partial chunk never looks complete.
   For parallel execution, use a bounded worker pool that claims independent
   batches; do not assign one long-lived agent to the whole campaign or merge
   several batch prompts into one request.
6. If validation fails, rerun the same provider flow with `--repair-coverage`
   when available, or regenerate/apply a corrected external AI response.
7. Validate with `qa_validate_translation --strict-unchanged --json`.
8. Hand off `project.fairtl`, `export-es`, and `playable-es` when present.

## Rules

- Never modify the source game directly.
- Never print API keys or auth files.
- Keep generated run artifacts outside the repository.
- Do not use local machine paths from examples; replace them with the active
  campaign workspace and key file.
- Keep source/reference text read-only. Fill translation fields only.
- Treat image text, encrypted archives, and unsupported engines as separate
  documented gaps.
- For Unity work, read `docs/UNITY_TRANSLATION_SUPPORT.md` before promising
  parser/exporter support.

## Done

Done means validated coverage, clean export, playable output when applicable,
and a short report naming the provider/model and any residual unsupported text.

# FairTranslator Agent Notes

This repo includes an agent workflow for translation campaigns:

- `docs/agent-workflows/fairtranslator-translation/SKILL.md`
- `docs/AI_AGENT_TRANSLATION_WORKFLOW.md`

Rules for agents:

- Never modify a source game folder directly.
- Keep generated campaign artifacts outside this repository.
- Never print API keys or auth file contents.
- Do not hardcode provider model IDs. Use dynamic provider discovery or the
  active local config.
- For headless provider translation, `qa_translate_provider` currently supports
  Codex CLI and OpenCode Go. Use `qa_prepare`/`qa_apply` for external AI agents.
- Unity is not a first-class product engine yet; read
  `docs/UNITY_TRANSLATION_SUPPORT.md` before promising support.
- Tauri versions follow the policy shared with ChronoRift (README, "Version
  policy"): same exact Tauri, `tauri-build` and shared plugin versions in both
  apps, bumped together after one session proposes and the other confirms, and
  only once every package is at least 7 days old. Never run a general
  `cargo update` while `tauri` is pinned to an older minor: it pulls the newer
  `tauri-runtime`/`tauri-utils` into `Cargo.lock`.

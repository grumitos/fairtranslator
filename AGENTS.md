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

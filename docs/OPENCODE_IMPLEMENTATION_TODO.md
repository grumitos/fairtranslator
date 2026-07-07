# OpenCode API Implementation Notes (Archived)

Status: archived implementation record. This is not the active runbook for new
translation campaigns. Use `docs/AI_AGENT_TRANSLATION_WORKFLOW.md` and
`docs/agent-workflows/fairtranslator-translation/SKILL.md` for current agent
execution.

## Goal

Record the OpenCode API implementation: secure API-key storage, dynamic model
refresh, model selection in Settings and AI translation flow, and OpenAI dynamic
model refresh.

## Requirements

- Add OpenCode as a first-class provider separate from the existing Codex CLI provider.
- Do not hardcode model IDs for OpenCode or OpenAI.
- Fetch models dynamically from provider `/models` endpoints.
- Support OpenCode Go endpoint bases only for now.
- Support OpenCode API families used by the published model endpoints:
  - OpenAI-compatible `chat/completions`
  - OpenAI `responses`
  - Anthropic-compatible `messages`
  - Google-style model endpoints where available
- Keep API keys in secure keyring storage.
- Show provider and model clearly before each AI translation run.
- Keep the existing Codex CLI flow visible in UI for CLI-based translations.
- Integrate the existing Codex CLI provider into the new model/provider UX instead of duplicating parallel flows.
- Evaluate the current OpenAI Codex SDK/Python SDK route before deciding whether to replace the CLI-backed provider.
- Evaluate OpenCode's programmatic server/SDK route before deciding whether direct gateway API calls are enough.
- Validate the OpenCode key with a real authenticated request before using it for game translation.
- Implemented boundary: use OpenCode Go endpoint bases only until non-Go support
  is explicitly enabled.

## Architecture Decisions

- Treat the current `CodexProvider` as a CLI-backed AI translation backend, not as OpenCode API support.
- Keep direct OpenAI/OpenCode translation calls on HTTP APIs because FairTranslator needs deterministic JSON translation results, not a code-editing agent session.
- Do not replace the existing Codex CLI backend with the OpenAI Codex Python SDK in this implementation. The Codex Python SDK controls a local Codex app-server/CLI runtime and is designed for Codex agent workflows; replacing the current `codex exec --output-schema` path would add a Python runtime dependency and more orchestration without improving the current translation JSON contract.
- Track OpenCode `opencode serve` as an optional future backend for programmatic agent sessions. It exposes an OpenAPI server and provider/session endpoints, but the direct Zen/Go gateway endpoints are the simpler path for batch translation.
- Consolidate shared UI concepts around provider, auth status, dynamic model list, selected model, and execution backend, while preserving provider-specific auth and request details behind backend services.
- Default OpenCode endpoint is OpenCode Go (`https://opencode.ai/zen/go/v1`); Zen is intentionally blocked until the user asks to enable it.

## Task List

### Phase 1: Backend Foundation

- [x] Add `ProviderType::Opencode` to Rust domain types and regenerated TypeScript bindings.
- [x] Introduce a common provider-model listing contract that can serve OpenAI, OpenCode, and Codex CLI metadata without forcing all providers to use the same transport.
- [x] Audit `CodexProvider` for reusable prompt/schema parsing logic before adding OpenCode, so the new provider reuses compact prompt parsing rather than duplicating behavior.
- [x] Add OpenCode provider ID, secure key storage, config persistence, and deletion paths.
- [x] Add `ProviderModel` response type for dynamic model lists.
- [x] Add IPC command `apikey_models(provider)` to fetch model lists.
- [x] Implement dynamic OpenAI model listing via `GET /v1/models`.
- [x] Implement dynamic OpenCode model listing via `{base}/models`.
- [x] Implement Codex CLI model discovery through `codex models` when available, or return current configured/default metadata when not available.
- [x] Normalize OpenCode endpoint base securely: HTTPS only, no embedded credentials, no query or fragment.

### Phase 2: OpenCode Provider

- [x] Create `src-tauri/src/services/opencode.rs`.
- [x] Implement translation prompt construction using the same compact context as OpenAI.
- [x] Implement response parsing back into `TranslationResponse`.
- [x] Implement API-family probing without hardcoded model IDs.
- [x] Treat auth/account errors as terminal and surface them clearly.
- [x] Retry only when the endpoint shape is unsupported for the selected model.
- [x] Implement validation with the selected model and selected endpoint base.
- [x] Add unit tests for endpoint derivation, model parsing, response parsing, route/account error classification, and error-body sanitization.
- [x] Add adaptive handling for chat responses that spend output on reasoning and return empty content, retrying with provider `thinking` disabled only after observing that response shape.
- [x] Retry transient OpenCode 5xx/network failures and normalize request temperature for strict gateways.
- [x] Tolerate recoverable provider JSON defects such as reasoning prefixes, duplicate translation keys, and redundant speaker prefixes.

### Phase 3: Planning and Translation Integration

- [x] Include OpenCode in chunk planning as an LLM provider with OpenAI-style prompt budget.
- [x] Include OpenCode in `translate_batch_start`.
- [x] Include OpenCode in QA/diagnostic provider parsing where relevant.
- [x] Ensure project glossary, style guide, memory, and neighboring chunk context are passed.
- [x] Keep Codex CLI translation in the same provider selection flow as OpenAI/OpenCode, but clearly label it as CLI-backed.
- [x] Avoid any duplicate Codex/OpenCode provider naming that could make users confuse OpenAI Codex CLI with OpenCode Go.

### Phase 4: Frontend IPC and Hooks

- [x] Add `ProviderModel` type in frontend domain types.
- [x] Add `apikeyModels(provider)` IPC wrapper.
- [x] Extend `useApiKeys` with `listModels`.
- [x] Keep API key values masked and avoid logging secrets.

### Phase 5: Settings UI

- [x] Add OpenCode provider card using existing `ApiKeySection` styles.
- [x] Add endpoint base field for OpenCode.
- [x] Add dynamic model selector for OpenAI, OpenCode, and Codex CLI where discovery is available.
- [x] Add refresh button for model lists.
- [x] Preserve manual model entry fallback if refresh fails or the model is absent.
- [x] Add validation button using stored key plus selected model.
- [x] Keep Codex CLI status/login UI separate and unchanged in meaning.
- [x] Make the Codex CLI card show executable/profile controls plus selected model.

### Phase 6: Batch Translation UI

- [x] Add OpenCode to the provider list.
- [x] Show selected provider and model before starting translation.
- [x] Allow choosing a model for each OpenAI/OpenCode translation run.
- [x] Allow choosing a model for each Codex CLI translation run when discovery/config allows it, and show model/profile/status for CLI flow.
- [x] Update provider capability notes.
- [x] Ensure disabled/loading/empty/error states fit the existing product UI.

### Phase 6.5: Backend Alternative Review

- [x] Document why OpenAI Codex Python SDK is or is not replacing `CodexProvider`.
- [x] Document why OpenCode `opencode serve` is or is not used for FairTranslator's batch translation flow.
- [x] If either alternative is chosen later, design it behind the same provider/model contract rather than adding another UI-only flow.

### Phase 7: i18n

- [x] Add English and Spanish settings labels for OpenCode.
- [x] Add English and Spanish labels for model refresh and selected model state.
- [x] Add English and Spanish batch-flow provider/model text.
- [x] Add validation and error text for missing model/key and refresh failures.

### Phase 8: Verification

- [x] Run targeted frontend tests for IPC and UI behavior.
- [x] Run Rust tests for provider and endpoint logic.
- [x] Run `npm run test:run`.
- [x] Run `npm run build`.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] Manually verify Settings and Batch Translation UI if a dev server/browser path is viable.

### Phase 9: OpenCode Key and Game Translation

- [x] Read a local API key file without printing the key.
- [x] Fetch OpenCode Go dynamic model list. Zen was fetched before the Go-only instruction and remains available through endpoint override.
- [x] Benchmark current non-Qwen Go candidates with real NSFW dialogue.
- [x] Select `glm-5.2`: valid JSON, correct NSFW translation, and better latency/quality than `mimo-v2.5-pro`; Kimi and MiniMax were not viable for full batches.
- [x] Validate the selected OpenCode Go model with a real authenticated request.
- [x] Create/open a FairTranslator project for `Breeding City Welcomes You!`.
- [x] Run batch translation until all 4,984 eligible entries are translated.
- [x] Save checkpoints after every successful chunk.
- [x] Verify strict coverage: 4,984/4,984 translated, 0 missing, 0 source-script residue, 0 unchanged, 0 errors/warnings.
- [x] Export the translated `www` tree and create a runnable game copy.

Credential note: API key files are local secrets and must never be printed.
Historical validation artifacts were generated outside the repository and are
not part of this implementation note.

## Source Notes

- OpenCode Go documents dynamic models at `https://opencode.ai/zen/go/v1/models`.
- OpenCode Zen documents dynamic models at `https://opencode.ai/zen/v1/models`.
- OpenAI documents dynamic model listing via `GET /v1/models`.

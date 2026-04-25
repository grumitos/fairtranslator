/// Tauri commands for managing translation API keys and batch translation.
///
/// API keys are stored in the operating system secure credential store.
/// Non-secret provider configuration is stored via `tauri-plugin-store`.
///
/// Batch translation emits progress events via `app.emit()` so the
/// frontend can display a progress bar.
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tauri_plugin_store::StoreExt;

use crate::commands::project::AppState;
use crate::error::AppError;
use crate::models::{
    AiBatchApplyResult, AiBatchExport, AiBatchResponseRef, BatchProgress, EntryStatus,
    GlossaryTerm, Project, ProviderType,
};
use crate::services::provider::{TranslationBatchContext, TranslationProvider, TranslationRequest};
use crate::services::{
    ai_batch, endpoint::normalize_https_endpoint_override, secrets, translation_apply,
    translation_orchestrator,
};

/// Name of the store file for API settings
const STORE_NAME: &str = "api-keys.json";

/// Store key prefix for API keys
const KEY_PREFIX: &str = "api_key_";

/// Store key prefix for model override
const MODEL_PREFIX: &str = "model_";

/// Store key prefix for endpoint override
const ENDPOINT_PREFIX: &str = "endpoint_";

/// Store key prefix for OpenAI organization override
const ORGANIZATION_PREFIX: &str = "organization_";

/// Store key prefix for OpenAI project override
const PROJECT_PREFIX: &str = "project_";

/// Store key prefix for command path overrides (used by CLI-backed providers)
const COMMAND_PATH_PREFIX: &str = "command_path_";

/// Store key prefix for CLI profile overrides
const PROFILE_PREFIX: &str = "profile_";

/// Derive the store key for a provider's API key
fn api_key_for(provider: &ProviderType) -> String {
    format!("{}{}", KEY_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's model setting
fn model_key_for(provider: &ProviderType) -> String {
    format!("{}{}", MODEL_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's endpoint setting
fn endpoint_key_for(provider: &ProviderType) -> String {
    format!("{}{}", ENDPOINT_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's organization setting
fn organization_key_for(provider: &ProviderType) -> String {
    format!("{}{}", ORGANIZATION_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's project setting
fn project_key_for(provider: &ProviderType) -> String {
    format!("{}{}", PROJECT_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's command path
fn command_path_key_for(provider: &ProviderType) -> String {
    format!("{}{}", COMMAND_PATH_PREFIX, provider_id(provider))
}

/// Derive the store key for a provider's CLI profile
fn profile_key_for(provider: &ProviderType) -> String {
    format!("{}{}", PROFILE_PREFIX, provider_id(provider))
}

fn provider_requires_api_key(provider: &ProviderType) -> bool {
    !matches!(provider, ProviderType::Codex)
}

fn api_key_secret_id(provider: &ProviderType) -> String {
    format!("fairtranslator:{}", api_key_for(provider))
}

fn normalize_provider_endpoint_override(
    provider: &ProviderType,
    endpoint: Option<String>,
) -> Result<Option<String>, AppError> {
    if matches!(provider, ProviderType::Codex) {
        return Ok(None);
    }

    normalize_https_endpoint_override(endpoint)
}

/// Stable string ID for each provider (used as store keys)
fn provider_id(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::Openai => "openai",
        ProviderType::Deepl => "deepl",
        ProviderType::Google => "google",
        ProviderType::Codex => "codex",
    }
}

/// Save an API key for a given provider.
/// Optionally store model and endpoint overrides.
#[tauri::command]
pub async fn apikey_save(
    app: tauri::AppHandle,
    provider: ProviderType,
    api_key: String,
    model: Option<String>,
    endpoint: Option<String>,
    organization: Option<String>,
    project: Option<String>,
    command_path: Option<String>,
    profile: Option<String>,
    _auth_mode: Option<String>,
) -> Result<(), AppError> {
    let endpoint = normalize_provider_endpoint_override(&provider, endpoint)?;

    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    let trimmed_api_key = api_key.trim();
    if provider_requires_api_key(&provider) || !trimmed_api_key.is_empty() {
        secrets::save_secret(&api_key_secret_id(&provider), trimmed_api_key)?;
    } else {
        secrets::delete_secret(&api_key_secret_id(&provider))?;
    }
    let _ = store.delete(api_key_for(&provider));

    if let Some(m) = model {
        store.set(model_key_for(&provider), Value::String(m));
    } else {
        let _ = store.delete(model_key_for(&provider));
    }

    if let Some(ep) = endpoint {
        store.set(endpoint_key_for(&provider), Value::String(ep));
    } else {
        let _ = store.delete(endpoint_key_for(&provider));
    }

    if let Some(org) = organization {
        store.set(organization_key_for(&provider), Value::String(org));
    } else {
        let _ = store.delete(organization_key_for(&provider));
    }

    if let Some(project) = project {
        store.set(project_key_for(&provider), Value::String(project));
    } else {
        let _ = store.delete(project_key_for(&provider));
    }

    if let Some(command_path) = command_path {
        store.set(command_path_key_for(&provider), Value::String(command_path));
    } else {
        let _ = store.delete(command_path_key_for(&provider));
    }

    if let Some(profile) = profile {
        store.set(profile_key_for(&provider), Value::String(profile));
    } else {
        let _ = store.delete(profile_key_for(&provider));
    }

    store
        .save()
        .map_err(|e| AppError::Project(format!("Failed to save key store: {}", e)))?;

    Ok(())
}

/// Load a saved API key for a given provider.
/// Returns the key string, or null/empty if not set.
#[tauri::command]
pub async fn apikey_load(
    app: tauri::AppHandle,
    provider: ProviderType,
) -> Result<ApiKeyInfo, AppError> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    let legacy_api_key = store
        .get(api_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let api_key = if provider_requires_api_key(&provider) {
        match secrets::load_secret(&api_key_secret_id(&provider))? {
            Some(secret) => secret,
            None if !legacy_api_key.trim().is_empty() => {
                let migrated = legacy_api_key.trim().to_string();
                secrets::save_secret(&api_key_secret_id(&provider), &migrated)?;
                let _ = store.delete(api_key_for(&provider));
                store.save().map_err(|e| {
                    AppError::Project(format!("Failed to remove legacy API key from store: {}", e))
                })?;
                migrated
            }
            None => String::new(),
        }
    } else {
        String::new()
    };

    let model = store
        .get(model_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let endpoint = store
        .get(endpoint_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let organization = store
        .get(organization_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let project = store
        .get(project_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let command_path = store
        .get(command_path_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let profile = store
        .get(profile_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    let auth_mode = if provider_requires_api_key(&provider) {
        "apiKey".to_string()
    } else {
        "codexManaged".to_string()
    };
    let is_configured = if provider_requires_api_key(&provider) {
        !api_key.trim().is_empty()
    } else {
        model.is_some() || command_path.is_some() || profile.is_some()
    };

    Ok(ApiKeyInfo {
        provider,
        api_key,
        model,
        endpoint,
        organization,
        project,
        command_path,
        profile,
        auth_mode: Some(auth_mode),
        is_configured,
    })
}

/// Delete a saved API key (and model/endpoint overrides) for a provider.
#[tauri::command]
pub async fn apikey_delete(app: tauri::AppHandle, provider: ProviderType) -> Result<(), AppError> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    secrets::delete_secret(&api_key_secret_id(&provider))?;
    let _ = store.delete(api_key_for(&provider));
    let _ = store.delete(model_key_for(&provider));
    let _ = store.delete(endpoint_key_for(&provider));
    let _ = store.delete(organization_key_for(&provider));
    let _ = store.delete(project_key_for(&provider));
    let _ = store.delete(command_path_key_for(&provider));
    let _ = store.delete(profile_key_for(&provider));

    store
        .save()
        .map_err(|e| AppError::Project(format!("Failed to save key store: {}", e)))?;

    Ok(())
}

/// Validate an API key by sending a test request to the provider.
#[tauri::command]
pub async fn apikey_validate(
    app: tauri::AppHandle,
    provider: ProviderType,
) -> Result<(), AppError> {
    // Load the stored key info
    let info = apikey_load(app.clone(), provider).await?;

    if provider_requires_api_key(&info.provider) && info.api_key.is_empty() {
        return Err(AppError::Api(
            "No API key configured for this provider".to_string(),
        ));
    }

    create_provider(&app, &info)?.validate().await
}

/// Inspect the local Codex runtime without requiring saved settings.
#[tauri::command]
pub async fn codex_status(
    app: tauri::AppHandle,
    command_path: Option<String>,
    model: Option<String>,
    profile: Option<String>,
) -> Result<crate::services::codex::CodexRuntimeStatus, AppError> {
    use crate::services::codex::CodexProvider;

    let provider = CodexProvider::with_app_handle(app, command_path, model, profile, None);
    provider.runtime_status().await
}

/// Info returned by `apikey_load`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyInfo {
    pub provider: ProviderType,
    pub api_key: String,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub command_path: Option<String>,
    pub profile: Option<String>,
    pub auth_mode: Option<String>,
    pub is_configured: bool,
}

/// Prepare a structured batch that can be sent to an external AI and later re-imported.
#[tauri::command]
pub async fn translate_ai_batch_export(
    state: tauri::State<'_, AppState>,
    file_paths: Vec<String>,
    max_entries: Option<usize>,
) -> Result<AiBatchExport, AppError> {
    let proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_ref()
        .ok_or_else(|| AppError::Project("No project is currently open".into()))?;

    ai_batch::export_ai_batch(project, &file_paths, max_entries)
}

/// Apply a structured JSON response from an external AI back into the current project.
#[tauri::command]
pub async fn translate_ai_batch_apply(
    state: tauri::State<'_, AppState>,
    raw_response: String,
    mark_status: Option<EntryStatus>,
    expected_project_id: Option<String>,
    expected_batch_id: Option<String>,
    response_map: Option<Vec<AiBatchResponseRef>>,
) -> Result<AiBatchApplyResult, AppError> {
    let translations = ai_batch::parse_ai_batch_response_with_refs(
        &raw_response,
        expected_project_id.as_deref(),
        expected_batch_id.as_deref(),
        response_map.as_deref(),
    )?;

    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No project is currently open".into()))?;

    Ok(ai_batch::apply_ai_batch_updates(
        project,
        &translations,
        mark_status.unwrap_or(EntryStatus::Draft),
    ))
}

/// Event name for batch translation progress
const BATCH_PROGRESS_EVENT: &str = "translation:progress";

/// Event name for batch translation completion
const BATCH_COMPLETE_EVENT: &str = "translation:complete";

#[derive(Debug, Clone)]
struct FileTranslationBatch {
    relative_path: String,
    entries: Vec<TranslationRequest>,
    context: TranslationBatchContext,
}

/// Create a boxed TranslationProvider from API key info
fn create_provider(
    app: &tauri::AppHandle,
    info: &ApiKeyInfo,
) -> Result<Box<dyn TranslationProvider>, AppError> {
    let endpoint = normalize_provider_endpoint_override(&info.provider, info.endpoint.clone())?;

    match info.provider {
        ProviderType::Openai => {
            use crate::services::openai::OpenAiProvider;
            Ok(Box::new(OpenAiProvider::new(
                info.api_key.clone(),
                info.model.clone(),
                endpoint,
                info.organization.clone(),
                info.project.clone(),
            )))
        }
        ProviderType::Deepl => {
            use crate::services::deepl::DeepLProvider;
            Ok(Box::new(DeepLProvider::new(info.api_key.clone(), endpoint)))
        }
        ProviderType::Google => {
            use crate::services::google::GoogleProvider;
            Ok(Box::new(GoogleProvider::new(
                info.api_key.clone(),
                endpoint,
            )))
        }
        ProviderType::Codex => {
            use crate::services::codex::CodexProvider;
            Ok(Box::new(CodexProvider::with_app_handle(
                app.clone(),
                info.command_path.clone(),
                info.model.clone(),
                info.profile.clone(),
                None,
            )))
        }
    }
}

fn make_progress(
    files_processed: usize,
    total_files: usize,
    chunks_processed: usize,
    total_chunks: usize,
    entries_translated: usize,
    total_entries: usize,
    current_file: String,
    current_file_chunks_processed: usize,
    current_file_total_chunks: usize,
    failed_entries: usize,
    failed_batches: usize,
    warnings: &[String],
) -> BatchProgress {
    BatchProgress {
        files_processed,
        total_files,
        chunks_processed,
        total_chunks,
        entries_translated,
        total_entries,
        current_file,
        current_file_chunks_processed,
        current_file_total_chunks,
        failed_entries,
        failed_batches,
        warnings: warnings.to_vec(),
    }
}

fn push_warning(warnings: &mut Vec<String>, message: String) {
    const MAX_WARNINGS: usize = 5;
    if warnings.len() < MAX_WARNINGS {
        warnings.push(message);
    }
}

async fn run_batch_translation(
    app: tauri::AppHandle,
    project_state: Arc<Mutex<Option<Project>>>,
    provider: Box<dyn TranslationProvider>,
    provider_name: String,
    source_lang: String,
    target_lang: String,
    glossary: Vec<GlossaryTerm>,
    batches: Vec<FileTranslationBatch>,
    total_entries: usize,
) {
    let total_chunks = batches.len();
    let total_files = batches
        .iter()
        .map(|batch| batch.relative_path.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let mut file_chunk_totals = std::collections::HashMap::new();
    for batch in &batches {
        *file_chunk_totals
            .entry(batch.relative_path.clone())
            .or_insert(0usize) += 1;
    }

    log::info!(
        "Starting batch translation with provider {} ({} files, {} chunks, {} entries)",
        provider_name,
        total_files,
        total_chunks,
        total_entries
    );
    let mut files_processed = 0usize;
    let mut chunks_processed = 0usize;
    let mut entries_translated = 0usize;
    let mut failed_entries = 0usize;
    let mut failed_batches = 0usize;
    let mut warnings: Vec<String> = Vec::new();
    let mut file_chunks_processed = std::collections::HashMap::new();

    for file_batch in batches {
        match provider
            .translate_batch(
                &file_batch.entries,
                &source_lang,
                &target_lang,
                &glossary,
                &file_batch.context,
            )
            .await
        {
            Ok(responses) => {
                let mut chunk_had_failure = false;
                let response_ids = responses
                    .iter()
                    .map(|response| response.entry_id.as_str())
                    .collect::<std::collections::HashSet<_>>();
                let missing_response_count = file_batch
                    .entries
                    .iter()
                    .filter(|request| !response_ids.contains(request.entry_id.as_str()))
                    .count();
                let requested_ids = file_batch
                    .entries
                    .iter()
                    .map(|request| request.entry_id.as_str())
                    .collect::<std::collections::HashSet<_>>();

                match {
                    let proj_guard = project_state
                        .lock()
                        .map_err(|e| AppError::Project(format!("Lock error: {e}")));

                    proj_guard.and_then(|mut guard| {
                        let project = guard.as_mut().ok_or_else(|| {
                            AppError::Project("No project is currently open".into())
                        })?;
                        translation_apply::apply_provider_responses(
                            project,
                            &file_batch.relative_path,
                            &responses,
                        )
                    })
                } {
                    Ok(result) => {
                        entries_translated += result.applied_entries;

                        if missing_response_count > 0 {
                            failed_entries += missing_response_count;
                            chunk_had_failure = true;
                            let message = format!(
                                "Provider '{}' returned {} fewer translations than requested for '{}'.",
                                provider_name,
                                missing_response_count,
                                file_batch.relative_path
                            );
                            log::error!("{message}");
                            push_warning(&mut warnings, message);
                        }

                        if !result.missing_project_entries.is_empty() {
                            failed_entries += result.missing_project_entries.len();
                            chunk_had_failure = true;
                            let message = format!(
                                "Translated responses could not be matched back into '{}' ({} entry IDs).",
                                file_batch.relative_path,
                                result.missing_project_entries.len()
                            );
                            log::error!("{message}");
                            push_warning(&mut warnings, message);
                        }

                        if !result.invalid_entries.is_empty() {
                            failed_entries += result.invalid_entries.len();
                            chunk_had_failure = true;
                            let message = format!(
                                "Skipped {} translated entries in '{}' because integrity checks failed.",
                                result.invalid_entries.len(),
                                file_batch.relative_path
                            );
                            log::error!("{message}");
                            push_warning(&mut warnings, message);
                        }

                        let unexpected_response_count = responses
                            .iter()
                            .filter(|response| !requested_ids.contains(response.entry_id.as_str()))
                            .count();
                        if unexpected_response_count > 0 {
                            chunk_had_failure = true;
                            let message = format!(
                                "Provider '{}' returned {} unexpected entry IDs for '{}'.",
                                provider_name, unexpected_response_count, file_batch.relative_path
                            );
                            log::error!("{message}");
                            push_warning(&mut warnings, message);
                        }

                        for warning in result.warnings {
                            push_warning(&mut warnings, warning);
                        }

                        if chunk_had_failure {
                            failed_batches += 1;
                        }
                    }
                    Err(error) => {
                        failed_entries += file_batch.entries.len();
                        failed_batches += 1;
                        let message = format!(
                            "Failed to apply translated responses for '{}' (provider {}): {}",
                            file_batch.relative_path, provider_name, error
                        );
                        log::error!("{message}");
                        push_warning(&mut warnings, message);
                    }
                }
            }
            Err(error) => {
                failed_entries += file_batch.entries.len();
                failed_batches += 1;
                let message = format!(
                    "Translation failed for '{}' with provider '{}': {}",
                    file_batch.relative_path, provider_name, error
                );
                log::error!(
                    "Translation failed (provider {}) for file {} ({} entries): {}",
                    provider_name,
                    file_batch.relative_path,
                    file_batch.entries.len(),
                    error
                );
                push_warning(&mut warnings, message);
            }
        }

        chunks_processed += 1;
        let current_file_chunks_processed = {
            let processed = file_chunks_processed
                .entry(file_batch.relative_path.clone())
                .or_insert(0usize);
            *processed += 1;
            *processed
        };
        let current_file_total_chunks = file_chunk_totals
            .get(&file_batch.relative_path)
            .copied()
            .unwrap_or(1);
        if current_file_chunks_processed == current_file_total_chunks {
            files_processed += 1;
        }

        let progress = make_progress(
            files_processed,
            total_files,
            chunks_processed,
            total_chunks,
            entries_translated,
            total_entries,
            file_batch.relative_path.clone(),
            current_file_chunks_processed,
            current_file_total_chunks,
            failed_entries,
            failed_batches,
            &warnings,
        );
        let _ = app.emit(BATCH_PROGRESS_EVENT, &progress);
    }

    let final_progress = make_progress(
        total_files,
        total_files,
        total_chunks,
        total_chunks,
        entries_translated,
        total_entries,
        String::new(),
        0,
        0,
        failed_entries,
        failed_batches,
        &warnings,
    );
    let _ = app.emit(BATCH_COMPLETE_EVENT, &final_progress);
    log::info!(
        "Batch translation complete with provider {} ({} files, {} chunks, {} entries translated, {} failed entries)",
        provider_name,
        total_files,
        total_chunks,
        entries_translated,
        failed_entries
    );
}

/// Start batch translation for selected files (or all files if `file_paths` is empty).
///
/// The command returns immediately with initial progress and runs translation
/// work in a background task that emits progress/complete events.
#[tauri::command]
pub async fn translate_batch_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    provider: ProviderType,
    file_paths: Vec<String>,
) -> Result<BatchProgress, AppError> {
    let info = apikey_load(app.clone(), provider).await?;
    let (source_lang, target_lang, glossary, batches, total_entries, total_files, total_chunks) = {
        let proj_guard = state
            .project
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

        let project = proj_guard
            .as_ref()
            .ok_or_else(|| AppError::Project("No project is currently open".into()))?;

        let source_lang = project.source_lang.clone();
        let target_lang = project.target_lang.clone();
        let glossary = project.glossary.clone();
        let planned_chunks =
            translation_orchestrator::build_translation_plan(project, &file_paths, &info.provider);
        translation_orchestrator::ensure_translation_plan_within_budget(
            project,
            &planned_chunks,
            &info.provider,
        )?;
        let total_entries = planned_chunks.iter().map(|chunk| chunk.entries.len()).sum();
        let total_files = planned_chunks
            .iter()
            .map(|chunk| chunk.relative_path.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let total_chunks = planned_chunks.len();
        let batches: Vec<FileTranslationBatch> = planned_chunks
            .into_iter()
            .map(|chunk| FileTranslationBatch {
                relative_path: chunk.relative_path,
                entries: chunk.entries,
                context: chunk.context,
            })
            .collect();

        (
            source_lang,
            target_lang,
            glossary,
            batches,
            total_entries,
            total_files,
            total_chunks,
        )
    };

    let initial_progress = make_progress(
        0,
        total_files,
        0,
        total_chunks,
        0,
        total_entries,
        String::new(),
        0,
        0,
        0,
        0,
        &[],
    );

    if total_chunks == 0 || total_entries == 0 {
        let progress = make_progress(
            total_files,
            total_files,
            total_chunks,
            total_chunks,
            0,
            total_entries,
            String::new(),
            0,
            0,
            0,
            0,
            &[],
        );
        let _ = app.emit(BATCH_COMPLETE_EVENT, &progress);
        return Ok(initial_progress);
    }

    if provider_requires_api_key(&info.provider) && info.api_key.is_empty() {
        return Err(AppError::Api(
            "No API key configured for this provider".to_string(),
        ));
    }

    let translation_provider = create_provider(&app, &info)?;
    translation_provider.validate().await?;
    let provider_name = translation_provider.name().to_string();

    let project_state = state.project.clone();
    tauri::async_runtime::spawn(async move {
        run_batch_translation(
            app,
            project_state,
            translation_provider,
            provider_name,
            source_lang,
            target_lang,
            glossary,
            batches,
            total_entries,
        )
        .await;
    });

    Ok(initial_progress)
}

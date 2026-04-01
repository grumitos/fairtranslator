/// Tauri commands for managing translation API keys and batch translation.
///
/// API keys are stored via `tauri-plugin-store` in a local JSON file
/// (never in project files). Keys are identified by provider name.
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
    AiBatchApplyResult, AiBatchExport, BatchProgress, EntryStatus, GlossaryTerm, Project,
    ProviderType,
};
use crate::services::provider::{TranslationProvider, TranslationRequest, TranslationResponse};
use crate::services::{ai_batch, persistence};

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
    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    if provider_requires_api_key(&provider) || !api_key.trim().is_empty() {
        store.set(api_key_for(&provider), Value::String(api_key));
    } else {
        let _ = store.delete(api_key_for(&provider));
    }

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

    let api_key = store
        .get(api_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

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

    // Create the appropriate provider and validate
    match info.provider {
        ProviderType::Openai => {
            use crate::services::openai::OpenAiProvider;
            use crate::services::provider::TranslationProvider;
            let p = OpenAiProvider::new(
                info.api_key,
                info.model,
                info.endpoint,
                info.organization,
                info.project,
            );
            p.validate().await
        }
        ProviderType::Deepl => {
            use crate::services::deepl::DeepLProvider;
            use crate::services::provider::TranslationProvider;
            let p = DeepLProvider::new(info.api_key, info.endpoint);
            p.validate().await
        }
        ProviderType::Google => {
            use crate::services::google::GoogleProvider;
            use crate::services::provider::TranslationProvider;
            let p = GoogleProvider::new(info.api_key, info.endpoint);
            p.validate().await
        }
        ProviderType::Codex => {
            use crate::services::codex::CodexProvider;
            use crate::services::provider::TranslationProvider;
            let p =
                CodexProvider::with_app_handle(app, info.command_path, info.model, info.profile);
            p.validate().await
        }
    }
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

    let provider = CodexProvider::with_app_handle(app, command_path, model, profile);
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
) -> Result<AiBatchApplyResult, AppError> {
    let translations = ai_batch::parse_ai_batch_response(
        &raw_response,
        expected_project_id.as_deref(),
        expected_batch_id.as_deref(),
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

/// Number of entries to send to the provider in each batch
const ENTRIES_PER_BATCH: usize = 20;

#[derive(Debug, Clone)]
struct FileTranslationBatch {
    relative_path: String,
    entries: Vec<TranslationRequest>,
}

#[derive(Debug, Default)]
struct BatchApplyResult {
    applied_entries: usize,
    missing_project_entries: Vec<String>,
}

/// Create a boxed TranslationProvider from API key info
fn create_provider(app: &tauri::AppHandle, info: &ApiKeyInfo) -> Box<dyn TranslationProvider> {
    match info.provider {
        ProviderType::Openai => {
            use crate::services::openai::OpenAiProvider;
            Box::new(OpenAiProvider::new(
                info.api_key.clone(),
                info.model.clone(),
                info.endpoint.clone(),
                info.organization.clone(),
                info.project.clone(),
            ))
        }
        ProviderType::Deepl => {
            use crate::services::deepl::DeepLProvider;
            Box::new(DeepLProvider::new(
                info.api_key.clone(),
                info.endpoint.clone(),
            ))
        }
        ProviderType::Google => {
            use crate::services::google::GoogleProvider;
            Box::new(GoogleProvider::new(
                info.api_key.clone(),
                info.endpoint.clone(),
            ))
        }
        ProviderType::Codex => {
            use crate::services::codex::CodexProvider;
            Box::new(CodexProvider::with_app_handle(
                app.clone(),
                info.command_path.clone(),
                info.model.clone(),
                info.profile.clone(),
            ))
        }
    }
}

fn make_progress(
    files_processed: usize,
    total_files: usize,
    entries_translated: usize,
    total_entries: usize,
    current_file: String,
    failed_entries: usize,
    failed_batches: usize,
    warnings: &[String],
) -> BatchProgress {
    BatchProgress {
        files_processed,
        total_files,
        entries_translated,
        total_entries,
        current_file,
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

fn apply_batch_responses(
    project_state: &Arc<Mutex<Option<Project>>>,
    file_path: &str,
    responses: &[TranslationResponse],
) -> Result<BatchApplyResult, AppError> {
    let mut proj_guard = project_state
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No project is currently open".into()))?;

    let file = project
        .files
        .iter_mut()
        .find(|f| f.relative_path == file_path)
        .ok_or_else(|| AppError::NotFound(format!("File not found: {file_path}")))?;

    let mut result = BatchApplyResult::default();

    for resp in responses {
        if let Some(entry) = file.entries.iter_mut().find(|e| e.id == resp.entry_id) {
            entry.translation = resp.translated_text.clone();
            entry.status = EntryStatus::Draft;
            result.applied_entries += 1;
        } else {
            result.missing_project_entries.push(resp.entry_id.clone());
        }
    }

    if result.applied_entries > 0 {
        project.updated_at = persistence::now_iso8601();
    }

    Ok(result)
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
    log::info!(
        "Starting batch translation with provider {} ({} files, {} entries)",
        provider_name,
        batches.len(),
        total_entries
    );
    let total_files = batches.len();
    let mut files_processed = 0usize;
    let mut entries_translated = 0usize;
    let mut failed_entries = 0usize;
    let mut failed_batches = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for file_batch in batches {
        if file_batch.entries.is_empty() {
            files_processed += 1;
            let progress = make_progress(
                files_processed,
                total_files,
                entries_translated,
                total_entries,
                file_batch.relative_path.clone(),
                failed_entries,
                failed_batches,
                &warnings,
            );
            let _ = app.emit(BATCH_PROGRESS_EVENT, &progress);
            continue;
        }

        for chunk in file_batch.entries.chunks(ENTRIES_PER_BATCH) {
            let requested_ids = chunk
                .iter()
                .map(|request| request.entry_id.as_str())
                .collect::<std::collections::HashSet<_>>();

            match provider
                .translate_batch(chunk, &source_lang, &target_lang, &glossary)
                .await
            {
                Ok(responses) => {
                    let mut chunk_had_failure = false;
                    let response_ids = responses
                        .iter()
                        .map(|response| response.entry_id.as_str())
                        .collect::<std::collections::HashSet<_>>();
                    let missing_response_count = chunk
                        .iter()
                        .filter(|request| !response_ids.contains(request.entry_id.as_str()))
                        .count();

                    match apply_batch_responses(
                        &project_state,
                        &file_batch.relative_path,
                        &responses,
                    ) {
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

                            let unexpected_response_count = responses
                                .iter()
                                .filter(|response| {
                                    !requested_ids.contains(response.entry_id.as_str())
                                })
                                .count();
                            if unexpected_response_count > 0 {
                                chunk_had_failure = true;
                                let message = format!(
                                    "Provider '{}' returned {} unexpected entry IDs for '{}'.",
                                    provider_name,
                                    unexpected_response_count,
                                    file_batch.relative_path
                                );
                                log::error!("{message}");
                                push_warning(&mut warnings, message);
                            }

                            if chunk_had_failure {
                                failed_batches += 1;
                            }
                        }
                        Err(error) => {
                            failed_entries += chunk.len();
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
                    failed_entries += chunk.len();
                    failed_batches += 1;
                    let message = format!(
                        "Translation failed for '{}' with provider '{}': {}",
                        file_batch.relative_path, provider_name, error
                    );
                    log::error!(
                        "Translation failed (provider {}) for file {} ({} entries): {}",
                        provider_name,
                        file_batch.relative_path,
                        chunk.len(),
                        error
                    );
                    push_warning(&mut warnings, message);
                }
            }

            let progress = make_progress(
                files_processed,
                total_files,
                entries_translated,
                total_entries,
                file_batch.relative_path.clone(),
                failed_entries,
                failed_batches,
                &warnings,
            );
            let _ = app.emit(BATCH_PROGRESS_EVENT, &progress);
        }

        files_processed += 1;
        let progress = make_progress(
            files_processed,
            total_files,
            entries_translated,
            total_entries,
            file_batch.relative_path.clone(),
            failed_entries,
            failed_batches,
            &warnings,
        );
        let _ = app.emit(BATCH_PROGRESS_EVENT, &progress);
    }

    let final_progress = make_progress(
        total_files,
        total_files,
        entries_translated,
        total_entries,
        String::new(),
        failed_entries,
        failed_batches,
        &warnings,
    );
    let _ = app.emit(BATCH_COMPLETE_EVENT, &final_progress);
    log::info!(
        "Batch translation complete with provider {} ({} files, {} entries translated, {} failed entries)",
        provider_name,
        total_files,
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
    if provider_requires_api_key(&info.provider) && info.api_key.is_empty() {
        return Err(AppError::Api(
            "No API key configured for this provider".to_string(),
        ));
    }

    let translation_provider = create_provider(&app, &info);
    translation_provider.validate().await?;
    let provider_name = translation_provider.name().to_string();

    let (source_lang, target_lang, glossary, batches, total_entries) = {
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

        let mut batches: Vec<FileTranslationBatch> = Vec::new();
        let mut total_entries = 0usize;

        for file in &project.files {
            if !file_paths.is_empty() && !file_paths.contains(&file.relative_path) {
                continue;
            }

            let entries: Vec<TranslationRequest> = file
                .entries
                .iter()
                .filter(|entry| entry.status == EntryStatus::Untranslated)
                .map(|entry| TranslationRequest {
                    entry_id: entry.id.clone(),
                    text: entry.original.clone(),
                })
                .collect();

            total_entries += entries.len();
            batches.push(FileTranslationBatch {
                relative_path: file.relative_path.clone(),
                entries,
            });
        }

        (source_lang, target_lang, glossary, batches, total_entries)
    };

    let total_files = batches.len();
    let initial_progress =
        make_progress(0, total_files, 0, total_entries, String::new(), 0, 0, &[]);

    if total_files == 0 || total_entries == 0 {
        let progress = make_progress(
            total_files,
            total_files,
            0,
            total_entries,
            String::new(),
            0,
            0,
            &[],
        );
        let _ = app.emit(BATCH_COMPLETE_EVENT, &progress);
        return Ok(initial_progress);
    }

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

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
use crate::services::provider::{secret_update, ApiKeyInfo, ProviderInfo, TranslationProvider};
use crate::services::{
    ai_batch,
    ai_models::ProviderModel,
    batch_lifecycle::{
        ensure_current_project_id, run_translation_chunk, BatchChunkOutcome, BatchRunControl,
        BatchRunLifecycle, FileTranslationBatch,
    },
    endpoint::normalize_https_endpoint_override,
    secrets, translation_orchestrator,
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
        ProviderType::Opencode => "opencode",
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
    let api_key_update = secret_update(&api_key).map(str::to_string);

    if api_key_update.is_none() && provider_requires_api_key(&provider) {
        let _ = load_provider_info(app.clone(), provider.clone()).await?;
    }

    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    if let Some(api_key) = api_key_update.as_deref() {
        secrets::save_secret(&secrets::api_key_secret_id(&provider), api_key)?;
    }
    if api_key_update.is_some() || provider_requires_api_key(&provider) {
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

async fn load_provider_info(
    app: tauri::AppHandle,
    provider: ProviderType,
) -> Result<ProviderInfo, AppError> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    let legacy_api_key = store
        .get(api_key_for(&provider))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let api_key = if provider_requires_api_key(&provider) {
        match secrets::load_secret(&secrets::api_key_secret_id(&provider))? {
            Some(secret) => secret,
            None if !legacy_api_key.trim().is_empty() => {
                let migrated = legacy_api_key.trim().to_string();
                secrets::save_secret(&secrets::api_key_secret_id(&provider), &migrated)?;
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

    Ok(ProviderInfo {
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

/// Load non-secret provider configuration for the frontend.
#[tauri::command]
pub async fn apikey_load(
    app: tauri::AppHandle,
    provider: ProviderType,
) -> Result<ApiKeyInfo, AppError> {
    Ok(load_provider_info(app, provider).await?.into())
}

/// Delete a saved API key (and model/endpoint overrides) for a provider.
#[tauri::command]
pub async fn apikey_delete(app: tauri::AppHandle, provider: ProviderType) -> Result<(), AppError> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| AppError::Project(format!("Failed to open key store: {}", e)))?;

    secrets::delete_secret(&secrets::api_key_secret_id(&provider))?;
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
    let info = load_provider_info(app.clone(), provider).await?;

    if provider_requires_api_key(&info.provider) && info.api_key.is_empty() {
        return Err(AppError::Api(
            "No API key configured for this provider".to_string(),
        ));
    }

    create_provider(&app, &info)?.validate().await
}

/// List models for providers that expose model discovery.
#[tauri::command]
pub async fn apikey_models(
    app: tauri::AppHandle,
    provider: ProviderType,
    api_key: Option<String>,
    model: Option<String>,
    endpoint: Option<String>,
    organization: Option<String>,
    project: Option<String>,
    command_path: Option<String>,
    profile: Option<String>,
) -> Result<Vec<ProviderModel>, AppError> {
    let mut info = load_provider_info(app.clone(), provider).await?;
    if let Some(api_key) = api_key.and_then(|value| secret_update(&value).map(str::to_string)) {
        info.api_key = api_key;
    }
    if model.is_some() {
        info.model = model;
    }
    if endpoint.is_some() {
        info.endpoint = endpoint;
    }
    if organization.is_some() {
        info.organization = organization;
    }
    if project.is_some() {
        info.project = project;
    }
    if command_path.is_some() {
        info.command_path = command_path;
    }
    if profile.is_some() {
        info.profile = profile;
    }
    let endpoint = normalize_provider_endpoint_override(&info.provider, info.endpoint.clone())?;

    match info.provider {
        ProviderType::Openai => {
            if info.api_key.trim().is_empty() {
                return Err(AppError::Api(
                    "No API key configured for this provider".to_string(),
                ));
            }
            crate::services::openai::OpenAiProvider::list_models(
                &info.api_key,
                endpoint,
                info.organization.clone(),
                info.project.clone(),
            )
            .await
        }
        ProviderType::Opencode => {
            crate::services::opencode::OpenCodeProvider::list_models(&info.api_key, endpoint).await
        }
        ProviderType::Codex => {
            let provider = crate::services::codex::CodexProvider::with_app_handle(
                app,
                info.command_path.clone(),
                info.model.clone(),
                info.profile.clone(),
                None,
            );
            provider.list_models().await
        }
        ProviderType::Deepl | ProviderType::Google => Ok(Vec::new()),
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

    let provider = CodexProvider::with_app_handle(app, command_path, model, profile, None);
    provider.runtime_status().await
}

/// Start the official Codex CLI login flow.
#[tauri::command]
pub async fn codex_login(
    app: tauri::AppHandle,
    command_path: Option<String>,
    model: Option<String>,
    profile: Option<String>,
    device_auth: Option<bool>,
) -> Result<crate::services::codex::CodexRuntimeStatus, AppError> {
    use crate::services::codex::CodexProvider;

    let provider = CodexProvider::with_app_handle(app, command_path, model, profile, None);
    provider.login(device_auth.unwrap_or(false)).await
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
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

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
    let expected_project_id = expected_project_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Parse("Falta el projectId del lote exportado".into()))?;
    let expected_batch_id = expected_batch_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Parse("Falta el batchId del lote exportado".into()))?;
    let response_map = response_map
        .as_deref()
        .ok_or_else(|| AppError::Parse("Falta el mapa de respuesta del lote exportado".into()))?;
    let translations = ai_batch::parse_ai_batch_response_with_refs(
        &raw_response,
        Some(expected_project_id),
        Some(expected_batch_id),
        Some(response_map),
    )?;
    ai_batch::validate_ai_batch_response_complete(response_map, &translations)?;

    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;
    ai_batch::ensure_ai_batch_project_matches(&project.id, expected_project_id)?;

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

static BATCH_RUN_CONTROL: BatchRunControl = BatchRunControl::new();

/// Create a boxed TranslationProvider from API key info
fn create_provider(
    app: &tauri::AppHandle,
    info: &ProviderInfo,
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
            )?))
        }
        ProviderType::Deepl => {
            use crate::services::deepl::DeepLProvider;
            Ok(Box::new(DeepLProvider::new(
                info.api_key.clone(),
                endpoint,
            )?))
        }
        ProviderType::Google => {
            use crate::services::google::GoogleProvider;
            Ok(Box::new(GoogleProvider::new(
                info.api_key.clone(),
                endpoint,
            )?))
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
        ProviderType::Opencode => {
            use crate::services::opencode::OpenCodeProvider;
            Ok(Box::new(OpenCodeProvider::new(
                info.api_key.clone(),
                info.model.clone(),
                endpoint,
            )?))
        }
    }
}

fn make_progress(
    run_id: u64,
    cancelled: bool,
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
        run_id: run_id.to_string(),
        cancelled,
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

async fn run_batch_translation<F>(
    project_state: Arc<Mutex<Option<Project>>>,
    expected_project_id: String,
    provider: Box<dyn TranslationProvider>,
    provider_name: String,
    source_lang: String,
    target_lang: String,
    glossary: Vec<GlossaryTerm>,
    batches: Vec<FileTranslationBatch>,
    total_entries: usize,
    control: &BatchRunControl,
    run_id: u64,
    mut on_progress: F,
) -> BatchProgress
where
    F: FnMut(&BatchProgress),
{
    let total_chunks = batches.len();
    let total_files = batches
        .iter()
        .map(|batch| batch.relative_path.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    let batch_paths = batches
        .iter()
        .map(|batch| batch.relative_path.clone())
        .collect::<Vec<_>>();
    let mut lifecycle = BatchRunLifecycle::new(control, run_id, &batch_paths);

    log::info!(
        "Starting batch translation with provider {} ({} files, {} chunks, {} entries)",
        provider_name,
        total_files,
        total_chunks,
        total_entries
    );
    let mut entries_translated = 0usize;
    let mut failed_entries = 0usize;
    let mut failed_batches = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for file_batch in batches {
        let outcome = run_translation_chunk(
            &mut lifecycle,
            &project_state,
            &expected_project_id,
            provider.as_ref(),
            &source_lang,
            &target_lang,
            &glossary,
            &file_batch,
        )
        .await;

        match outcome {
            BatchChunkOutcome::Cancelled => break,
            BatchChunkOutcome::ProjectChanged(error) => {
                let message = format!(
                    "Se canceló la traducción batch porque ya no se pudo validar el proyecto planificado: {error}"
                );
                log::warn!("{message}");
                push_warning(&mut warnings, message);
                break;
            }
            BatchChunkOutcome::Applied {
                responses,
                apply_result: result,
            } => {
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

                entries_translated += result.applied_entries;

                if missing_response_count > 0 {
                    failed_entries += missing_response_count;
                    chunk_had_failure = true;
                    let message = format!(
                                "El proveedor '{}' devolvió {} traducciones menos de las solicitadas para '{}'.",
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
                        "No se pudieron asociar respuestas traducidas en '{}' ({} ID de entrada).",
                        file_batch.relative_path,
                        result.missing_project_entries.len()
                    );
                    log::error!("{message}");
                    push_warning(&mut warnings, message);
                }

                if !result.skipped_non_pending_entries.is_empty() {
                    failed_entries += result.skipped_non_pending_entries.len();
                    chunk_had_failure = true;
                    let message = format!(
                                "Se omitieron {} entradas traducidas en '{}' porque dejaron de estar pendientes durante la solicitud.",
                                result.skipped_non_pending_entries.len(),
                                file_batch.relative_path
                            );
                    log::warn!("{message}");
                    push_warning(&mut warnings, message);
                }

                if !result.invalid_entries.is_empty() {
                    failed_entries += result.invalid_entries.len();
                    chunk_had_failure = true;
                    let message = format!(
                                "Se omitieron {} entradas traducidas en '{}' porque fallaron las comprobaciones de integridad.",
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
                        "El proveedor '{}' devolvió {} ID de entrada inesperados para '{}'.",
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
            BatchChunkOutcome::ProviderFailed(error) => {
                failed_entries += file_batch.entries.len();
                failed_batches += 1;
                let message = format!(
                    "Falló la traducción de '{}' con el proveedor '{}': {}",
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
            BatchChunkOutcome::ApplyFailed(error) => {
                failed_entries += file_batch.entries.len();
                failed_batches += 1;
                let message = format!(
                    "No se pudieron aplicar respuestas traducidas para '{}' (proveedor {}): {}",
                    file_batch.relative_path, provider_name, error
                );
                log::error!("{message}");
                push_warning(&mut warnings, message);
            }
        }

        let (current_file_chunks_processed, current_file_total_chunks) =
            lifecycle.record_processed_chunk(&file_batch.relative_path);

        let progress = make_progress(
            run_id,
            false,
            lifecycle.files_processed(),
            total_files,
            lifecycle.chunks_processed(),
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
        on_progress(&progress);
    }

    let final_progress = make_progress(
        run_id,
        lifecycle.cancelled(),
        lifecycle.files_processed(),
        total_files,
        lifecycle.chunks_processed(),
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
    log::info!(
        "Batch translation finished with provider {} (cancelled={}, {} files, {} chunks, {} entries translated, {} failed entries)",
        provider_name,
        lifecycle.cancelled(),
        lifecycle.files_processed(),
        lifecycle.chunks_processed(),
        entries_translated,
        failed_entries
    );
    final_progress
}

/// Start batch translation for selected files (or all files if `file_paths` is empty).
///
/// The command returns immediately with initial progress and runs translation
/// work in a background task that emits progress/complete events.
#[tauri::command]
pub fn translate_batch_cancel(run_id: String) -> Result<bool, AppError> {
    let run_id = run_id
        .parse::<u64>()
        .map_err(|_| AppError::Project("El ID de la traducción batch no es válido".into()))?;
    Ok(BATCH_RUN_CONTROL.cancel(run_id))
}

#[tauri::command]
pub async fn translate_batch_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    provider: ProviderType,
    file_paths: Vec<String>,
    model: Option<String>,
) -> Result<BatchProgress, AppError> {
    let run_guard = BATCH_RUN_CONTROL.try_acquire()?;
    let run_id = run_guard.run_id();
    let mut info = load_provider_info(app.clone(), provider).await?;
    if let Some(model) = model {
        let model = model.trim().to_string();
        info.model = if model.is_empty() { None } else { Some(model) };
    }
    let (
        project_id,
        source_lang,
        target_lang,
        glossary,
        batches,
        total_entries,
        total_files,
        total_chunks,
    ) = {
        let proj_guard = state
            .project
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

        let project = proj_guard
            .as_ref()
            .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

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
            project.id.clone(),
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
        run_id,
        false,
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
            run_id,
            false,
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
    let provider_name = translation_provider.name().to_string();

    let project_state = state.project.clone();
    tauri::async_runtime::spawn(async move {
        let _run_guard = run_guard;
        let batch_paths = batches
            .iter()
            .map(|batch| batch.relative_path.clone())
            .collect::<Vec<_>>();
        let mut lifecycle = BatchRunLifecycle::new(&BATCH_RUN_CONTROL, run_id, &batch_paths);
        let terminal = match lifecycle
            .wait_or_cancel(translation_provider.validate())
            .await
        {
            None => make_progress(
                run_id,
                true,
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
            ),
            Some(Err(error)) => make_progress(
                run_id,
                false,
                0,
                total_files,
                0,
                total_chunks,
                0,
                total_entries,
                String::new(),
                0,
                0,
                total_entries,
                total_chunks,
                &[format!(
                    "No se pudo validar el proveedor '{}': {error}",
                    provider_name
                )],
            ),
            Some(Ok(())) => match ensure_current_project_id(&project_state, &project_id) {
                Err(error) => make_progress(
                    run_id,
                    true,
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
                    &[format!(
                        "Se canceló la traducción batch porque el proyecto cambió: {error}"
                    )],
                ),
                Ok(()) => {
                    let progress_app = app.clone();
                    run_batch_translation(
                        project_state,
                        project_id,
                        translation_provider,
                        provider_name,
                        source_lang,
                        target_lang,
                        glossary,
                        batches,
                        total_entries,
                        &BATCH_RUN_CONTROL,
                        run_id,
                        move |progress| {
                            let _ = progress_app.emit(BATCH_PROGRESS_EVENT, progress);
                        },
                    )
                    .await
                }
            },
        };
        let _ = app.emit(BATCH_COMPLETE_EVENT, &terminal);
    });

    Ok(initial_progress)
}

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use app_lib::models::{GlossaryTerm, Project, ProviderType, TranslationEntry, TranslationFile};
use app_lib::parser::{parse_game, resolve_game_dir};
use app_lib::services::exporter;
use app_lib::services::persistence;
use app_lib::services::provider::{TranslationProvider, TranslationRequest, TranslationResponse};
use app_lib::services::translation_apply;
use app_lib::services::translation_coverage::{
    mark_project_coverage_issues_for_repair, validate_project_translation_coverage,
    CoverageOptions, TranslationCoverageReport,
};
use app_lib::services::translation_orchestrator;
use chrono::Local;
use serde::Serialize;
use tokio::runtime::Builder;

#[derive(Debug)]
struct Options {
    provider: ProviderType,
    game_dir: PathBuf,
    workspace: PathBuf,
    files: Vec<String>,
    source_lang: String,
    target_lang: String,
    style_guide: Vec<String>,
    model: Option<String>,
    profile: Option<String>,
    reasoning_effort: Option<String>,
    fallback_model: Option<String>,
    fallback_profile: Option<String>,
    fallback_reasoning_effort: Option<String>,
    api_key_file: Option<PathBuf>,
    endpoint: Option<String>,
    strict_target_language: bool,
    repair_coverage: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunReport {
    game_dir: String,
    project_path: String,
    export_dir: String,
    playable_dir: Option<String>,
    files_selected: usize,
    translated_entries: usize,
    unresolved_entries: usize,
    validation_warnings: Vec<String>,
    provider: String,
    model: Option<String>,
    profile: Option<String>,
    reasoning_effort: Option<String>,
    fallback_model: Option<String>,
    fallback_profile: Option<String>,
    fallback_reasoning_effort: Option<String>,
    fallback_triggered: bool,
    fallback_policy: String,
    retry_count: usize,
    models_used: Vec<String>,
    coverage_repair_marked: usize,
    strict_target_language: bool,
    coverage: TranslationCoverageReport,
}

#[derive(Debug, Clone)]
struct ProviderRunConfig {
    provider: ProviderType,
    model: Option<String>,
    profile: Option<String>,
    reasoning_effort: Option<String>,
    api_key_file: Option<PathBuf>,
    endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QaTranslateStatus {
    phase: String,
    provider: String,
    active_model: String,
    fallback_policy: String,
    files_total: usize,
    files_completed: usize,
    chunks_total: usize,
    chunks_completed: usize,
    entries_total: usize,
    entries_applied: usize,
    current_file: Option<String>,
    current_chunk_index: Option<usize>,
    current_file_chunk_index: Option<usize>,
    current_file_chunk_total: Option<usize>,
    retry_count: usize,
    unresolved_entries: usize,
    planner_cache_hit: bool,
    coverage_repair_marked: usize,
    strict_target_language: bool,
    coverage_complete: bool,
    coverage_errors: usize,
    coverage_warnings: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslationFailureRecord {
    file_path: String,
    entry_id: String,
    stage: String,
    reason: String,
    warnings: Vec<String>,
}

const MAX_RETRYABLE_CHUNK_ATTEMPTS: usize = 3;

fn print_usage() {
    eprintln!(
        "Uso: cargo run --bin qa_translate_provider -- --game-dir <ruta> [opciones]\n\
         \n\
         Opciones:\n\
           --provider <nombre>      Proveedor: codex u opencode (default: codex)\n\
           --workspace <ruta>       Directorio para artefactos (default: D:\\FairTranslatorRuns\\<timestamp>)\n\
           --files <csv>            Archivos relativos a traducir (ej. data/System.json,data/Map034.json)\n\
           --source-lang <texto>    Idioma fuente (default: mixed Japanese and English)\n\
           --target-lang <texto>    Idioma destino (default: es)\n\
           --style <texto>          Regla de guía de estilo; se puede repetir\n\
           --model <nombre>         Modelo del proveedor; requerido para OpenCode\n\
           --profile <nombre>       Perfil Codex CLI (opcional)\n\
           --reasoning-effort <v>   Esfuerzo de razonamiento Codex CLI (ej. low, medium, high)\n\
           --fallback-model <m>     Modelo de relevo si el primario falla\n\
           --fallback-profile <p>   Perfil Codex CLI para el relevo (opcional)\n\
           --fallback-reasoning-effort <v>  Esfuerzo de razonamiento del relevo\n\
           --api-key-file <ruta>    Archivo con la API key o auth.json de OpenCode (requerido para OpenCode)\n\
           --endpoint <url>         Endpoint/base URL del proveedor (opcional)\n\
           --strict-target-language Trata texto traducible idéntico al original como fallo de QA\n\
           --repair-coverage        Reintenta entradas que fallan cobertura estricta en un proyecto existente\n\
           --help, -h               Muestra esta ayuda"
    );
}

fn parse_provider(value: &str) -> Result<ProviderType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "codex" => Ok(ProviderType::Codex),
        "opencode" | "open-code" => Ok(ProviderType::Opencode),
        other => Err(format!(
            "Proveedor no soportado por este QA: {other}. Usa codex u opencode."
        )),
    }
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut provider = ProviderType::Codex;
    let mut game_dir = None;
    let mut workspace = PathBuf::from(format!(
        "D:\\FairTranslatorRuns\\{}",
        Local::now().format("qa-provider-%Y%m%d-%H%M%S")
    ));
    let mut files = Vec::new();
    let mut source_lang = "mixed Japanese and English".to_string();
    let mut target_lang = "es".to_string();
    let mut style_guide = Vec::new();
    let mut model = None;
    let mut profile = None;
    let mut reasoning_effort = None;
    let mut fallback_model = None;
    let mut fallback_profile = None;
    let mut fallback_reasoning_effort = None;
    let mut api_key_file = None;
    let mut endpoint = None;
    let mut strict_target_language = false;
    let mut repair_coverage = false;

    let mut args = argv.skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--provider" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --provider".to_string())?;
                provider = parse_provider(&value)?;
            }
            "--game-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --game-dir".to_string())?;
                game_dir = Some(PathBuf::from(value));
            }
            "--workspace" => {
                workspace = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "Falta valor para --workspace".to_string())?,
                );
            }
            "--files" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --files".to_string())?;
                files = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            "--source-lang" => {
                source_lang = args
                    .next()
                    .ok_or_else(|| "Falta valor para --source-lang".to_string())?;
            }
            "--target-lang" => {
                target_lang = args
                    .next()
                    .ok_or_else(|| "Falta valor para --target-lang".to_string())?;
            }
            "--style" => {
                style_guide.push(
                    args.next()
                        .ok_or_else(|| "Falta valor para --style".to_string())?,
                );
            }
            "--model" => {
                model = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --model".to_string())?,
                );
            }
            "--profile" => {
                profile = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --profile".to_string())?,
                );
            }
            "--reasoning-effort" => {
                reasoning_effort = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --reasoning-effort".to_string())?,
                );
            }
            "--fallback-model" => {
                fallback_model = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --fallback-model".to_string())?,
                );
            }
            "--fallback-profile" => {
                fallback_profile = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --fallback-profile".to_string())?,
                );
            }
            "--fallback-reasoning-effort" => {
                fallback_reasoning_effort =
                    Some(args.next().ok_or_else(|| {
                        "Falta valor para --fallback-reasoning-effort".to_string()
                    })?);
            }
            "--api-key-file" => {
                api_key_file =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "Falta valor para --api-key-file".to_string()
                    })?));
            }
            "--endpoint" => {
                endpoint = Some(
                    args.next()
                        .ok_or_else(|| "Falta valor para --endpoint".to_string())?,
                );
            }
            "--strict-target-language" => strict_target_language = true,
            "--repair-coverage" => repair_coverage = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("Argumento desconocido: {arg}")),
        }
    }

    Ok(Options {
        provider,
        game_dir: game_dir.ok_or_else(|| "Debes indicar --game-dir".to_string())?,
        workspace,
        files,
        source_lang,
        target_lang,
        style_guide,
        model,
        profile,
        reasoning_effort,
        fallback_model,
        fallback_profile,
        fallback_reasoning_effort,
        api_key_file,
        endpoint,
        strict_target_language,
        repair_coverage,
    })
}

fn build_project(
    game_dir: &Path,
    source_lang: &str,
    target_lang: &str,
    style_guide: Vec<String>,
) -> Result<Project, String> {
    let resolved_path = resolve_game_dir(game_dir).map_err(|error| error.to_string())?;
    let parsed = parse_game(&resolved_path).map_err(|error| error.to_string())?;
    let now = persistence::now_iso8601();

    Ok(Project {
        id: uuid::Uuid::new_v4().to_string(),
        name: resolved_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_string(),
        source_lang: source_lang.to_string(),
        target_lang: target_lang.to_string(),
        engine: parsed.engine,
        game_dir: resolved_path.display().to_string(),
        files: parsed.files,
        glossary: Vec::new(),
        style_guide,
        created_at: now.clone(),
        updated_at: now,
    })
}

fn merge_missing_entries(project: &mut Project, parsed_files: Vec<TranslationFile>) -> usize {
    let mut added = 0;
    for parsed_file in parsed_files {
        if let Some(project_file) = project
            .files
            .iter_mut()
            .find(|file| file.relative_path == parsed_file.relative_path)
        {
            let existing = project_file
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<HashSet<_>>();
            for entry in parsed_file.entries {
                if !existing.contains(&entry.id) {
                    project_file.entries.push(entry);
                    added += 1;
                }
            }
        } else {
            added += parsed_file.entries.len();
            project.files.push(parsed_file);
        }
    }
    added
}

fn format_elapsed(started_at: Instant) -> String {
    let elapsed = started_at.elapsed().as_secs();
    let hours = elapsed / 3600;
    let minutes = (elapsed % 3600) / 60;
    let seconds = elapsed % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn render_progress_bar(current: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return "-".repeat(width);
    }

    let ratio = (current as f64 / total as f64).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let filled = filled.min(width);
    format!(
        "{}{}",
        "#".repeat(filled),
        "-".repeat(width.saturating_sub(filled))
    )
}

fn print_progress_line(progress_log: &Path, message: &str) {
    eprintln!("{message}");
    let _ = io::stderr().flush();
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(progress_log)
    {
        let _ = writeln!(file, "{message}");
        let _ = file.flush();
    }
}

fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn load_planner_cache(path: &Path) -> Option<translation_orchestrator::PlannerCacheData> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_status(path: &Path, status: &QaTranslateStatus) {
    let _ = write_json_file(path, status);
}

fn save_failures(path: &Path, failures: &[TranslationFailureRecord]) {
    let _ = write_json_file(path, failures);
}

fn prepare_workspace_export_dir(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    if !path.is_dir() {
        return Err(format!(
            "La ruta de export '{}' existe pero no es un directorio.",
            path.display()
        ));
    }

    std::fs::remove_dir_all(path).map_err(|error| {
        format!(
            "No se pudo limpiar export previo '{}': {error}",
            path.display()
        )
    })?;
    Ok(true)
}

fn create_playable_copy_if_nested_mv_mz_www(
    game_dir: &Path,
    export_dir: &Path,
    workspace: &Path,
) -> Result<Option<PathBuf>, String> {
    let Some(name) = game_dir.file_name().and_then(|value| value.to_str()) else {
        return Ok(None);
    };
    if !name.eq_ignore_ascii_case("www") {
        return Ok(None);
    }

    let Some(parent) = game_dir.parent() else {
        return Ok(None);
    };
    if !["Game.exe", "nw.exe", "package.json"]
        .iter()
        .any(|marker| parent.join(marker).is_file())
    {
        return Ok(None);
    }

    let playable_dir = workspace.join("playable-es");
    guard_copy_destination(parent, &playable_dir)?;
    prepare_workspace_export_dir(&playable_dir)?;
    copy_dir_recursive_skipping(parent, &playable_dir, Some(name))?;
    copy_dir_recursive_skipping(export_dir, &playable_dir.join(name), None)?;
    Ok(Some(playable_dir))
}

fn guard_copy_destination(source_dir: &Path, destination_dir: &Path) -> Result<(), String> {
    let source = std::fs::canonicalize(source_dir).map_err(|error| {
        format!(
            "No se pudo resolver origen jugable '{}': {error}",
            source_dir.display()
        )
    })?;
    let destination_parent = destination_dir
        .parent()
        .ok_or_else(|| {
            format!(
                "La ruta jugable '{}' no tiene directorio padre.",
                destination_dir.display()
            )
        })
        .and_then(|parent| {
            std::fs::canonicalize(parent).map_err(|error| {
                format!(
                    "No se pudo resolver destino jugable '{}': {error}",
                    parent.display()
                )
            })
        })?;
    let destination = destination_parent.join(
        destination_dir
            .file_name()
            .ok_or_else(|| "La ruta jugable no tiene nombre final.".to_string())?,
    );
    if destination.starts_with(&source) {
        return Err(format!(
            "La copia jugable '{}' no puede estar dentro del juego original '{}'.",
            destination.display(),
            source.display()
        ));
    }
    Ok(())
}

fn copy_dir_recursive_skipping(
    source_dir: &Path,
    target_dir: &Path,
    skip_dir_name: Option<&str>,
) -> Result<(), String> {
    std::fs::create_dir_all(target_dir).map_err(|error| {
        format!(
            "No se pudo crear directorio '{}': {error}",
            target_dir.display()
        )
    })?;

    for entry in std::fs::read_dir(source_dir).map_err(|error| {
        format!(
            "No se pudo leer directorio '{}': {error}",
            source_dir.display()
        )
    })? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let file_name = entry.file_name();
        if skip_dir_name
            .and_then(|skip| {
                file_name
                    .to_str()
                    .map(|value| value.eq_ignore_ascii_case(skip))
            })
            .unwrap_or(false)
        {
            continue;
        }
        let target_path = target_dir.join(file_name);
        if source_path.is_dir() {
            copy_dir_recursive_skipping(&source_path, &target_path, None)?;
        } else if source_path.is_file() {
            std::fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "No se pudo copiar '{}' a '{}': {error}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn translate_with_usage_limit_fallback(
    runtime: &tokio::runtime::Runtime,
    provider: &mut Box<dyn TranslationProvider>,
    active_config: &mut ProviderRunConfig,
    fallback_config: Option<&ProviderRunConfig>,
    fallback_triggered: &mut bool,
    models_used: &mut Vec<String>,
    progress_log: &Path,
    started_at: Instant,
    file_path: &str,
    chunk_index: usize,
    total_chunks: usize,
    requests: &[TranslationRequest],
    source_lang: &str,
    target_lang: &str,
    glossary: &[GlossaryTerm],
    context: &app_lib::services::provider::TranslationBatchContext,
) -> Result<Vec<TranslationResponse>, String> {
    loop {
        match runtime.block_on(provider.translate_batch(
            requests,
            source_lang,
            target_lang,
            glossary,
            context,
        )) {
            Ok(responses) => return Ok(responses),
            Err(primary_error) => {
                if !*fallback_triggered && is_usage_limit_error(&primary_error.to_string()) {
                    if let Some(fallback) = fallback_config {
                        print_progress_line(
                            progress_log,
                            &format!(
                                "[{}] Falló por límite de uso en {} lote {}/{} con {}: {}",
                                format_elapsed(started_at),
                                file_path,
                                chunk_index,
                                total_chunks,
                                run_config_label(active_config),
                                primary_error
                            ),
                        );
                        print_progress_line(
                            progress_log,
                            &format!(
                                "[{}] Intentando modelo alternativo {}",
                                format_elapsed(started_at),
                                run_config_label(fallback)
                            ),
                        );
                        let fallback_provider = create_provider(fallback)?;
                        if let Err(fallback_error) = runtime.block_on(fallback_provider.validate())
                        {
                            return Err(format!(
                                "Falló la traducción en {} lote {}/{}. Primario: {}. Modelo alternativo inválido: {}",
                                file_path, chunk_index, total_chunks, primary_error, fallback_error
                            ));
                        }
                        *provider = fallback_provider;
                        *active_config = fallback.clone();
                        *fallback_triggered = true;
                        models_used.push(run_config_label(active_config));
                        print_progress_line(
                            progress_log,
                            &format!(
                                "[{}] Modelo alternativo activado | {}",
                                format_elapsed(started_at),
                                run_config_label(active_config)
                            ),
                        );
                        continue;
                    }
                }

                return Err(format!(
                    "Falló la traducción en {} lote {}/{}: {}",
                    file_path, chunk_index, total_chunks, primary_error
                ));
            }
        }
    }
}

fn provider_label(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::Codex => "codex",
        ProviderType::Opencode => "opencode",
        ProviderType::Openai => "openai",
        ProviderType::Deepl => "deepl",
        ProviderType::Google => "google",
    }
}

fn run_config_label(config: &ProviderRunConfig) -> String {
    let base = format!(
        "provider={} model={}",
        provider_label(&config.provider),
        config.model.as_deref().unwrap_or("default")
    );
    match config.provider {
        ProviderType::Codex => format!(
            "{} profile={} reasoning={}",
            base,
            config.profile.as_deref().unwrap_or("default"),
            config.reasoning_effort.as_deref().unwrap_or("default")
        ),
        ProviderType::Opencode => format!(
            "{} endpoint={}",
            base,
            config.endpoint.as_deref().unwrap_or("default")
        ),
        _ => base,
    }
}

fn create_provider(config: &ProviderRunConfig) -> Result<Box<dyn TranslationProvider>, String> {
    match config.provider {
        ProviderType::Codex => Ok(Box::new(app_lib::services::codex::CodexProvider::new(
            None,
            config.model.clone(),
            config.profile.clone(),
            config.reasoning_effort.clone(),
        ))),
        ProviderType::Opencode => {
            let key_path = config
                .api_key_file
                .as_ref()
                .ok_or_else(|| "OpenCode requiere --api-key-file".to_string())?;
            let api_key = read_opencode_api_key_file(key_path, &config.provider)?;
            Ok(Box::new(
                app_lib::services::opencode::OpenCodeProvider::new(
                    api_key,
                    config.model.clone(),
                    config.endpoint.clone(),
                )
                .map_err(|error| error.to_string())?,
            ))
        }
        _ => Err(format!(
            "Proveedor no soportado por este QA: {}",
            provider_label(&config.provider)
        )),
    }
}

fn read_opencode_api_key_file(key_path: &Path, provider: &ProviderType) -> Result<String, String> {
    let text = std::fs::read_to_string(key_path).map_err(|error| {
        format!(
            "No se pudo leer API key de '{}': {error}",
            key_path.display()
        )
    })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "El archivo de API key '{}' está vacío",
            key_path.display()
        ));
    }

    if trimmed.starts_with('{') {
        let parsed: serde_json::Value = serde_json::from_str(trimmed).map_err(|error| {
            format!(
                "No se pudo interpretar '{}' como auth.json de OpenCode: {error}",
                key_path.display()
            )
        })?;
        let provider_ids = match provider {
            ProviderType::Opencode => vec!["opencode-go", "opencode"],
            _ => vec![provider_label(provider)],
        };
        for provider_id in provider_ids {
            if let Some(value) = parsed.get(provider_id) {
                if let Some(key) = extract_key_from_json_value(value) {
                    return Ok(key);
                }
            }
        }
        if let Some(key) = extract_key_from_json_value(&parsed) {
            return Ok(key);
        }
        return Err(format!(
            "No se encontró una key usable para {} en '{}'",
            provider_label(provider),
            key_path.display()
        ));
    }

    Ok(trimmed.to_string())
}

fn extract_key_from_json_value(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.trim().to_string()).filter(|key| !key.is_empty());
    }
    ["key", "apiKey", "api_key"]
        .iter()
        .filter_map(|field| value.get(field))
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .map(str::to_string)
        .find(|key| !key.is_empty())
}

fn is_usage_limit_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("usage limit")
        || normalized.contains("quota")
        || normalized.contains("limit reached")
        || normalized.contains("rate limit")
        || normalized.contains("429")
}

fn is_retryable_provider_shape_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("provider returned")
        || normalized.contains("provider omitted")
        || normalized.contains("unexpected local entry id")
        || normalized.contains("duplicate local entry id")
        || normalized.contains("unexpected entry id")
        || normalized.contains("duplicate entry id")
        || normalized.contains("failed to parse opencode response as translation json")
        || normalized.contains("no se pudo leer la respuesta del proveedor")
}

fn is_provider_content_inspection_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("datainspectionfailed")
        || normalized.contains("inappropriate content")
        || normalized.contains("data may contain inappropriate")
}

fn snapshot_file_state(
    project: &Project,
    file_path: &str,
) -> Result<(Vec<TranslationEntry>, String), String> {
    let file = project
        .files
        .iter()
        .find(|file| file.relative_path == file_path)
        .ok_or_else(|| format!("No se encontró el archivo '{file_path}' en el proyecto"))?;

    Ok((file.entries.clone(), project.updated_at.clone()))
}

fn restore_file_state(
    project: &mut Project,
    file_path: &str,
    entries: Vec<TranslationEntry>,
    updated_at: String,
) -> Result<(), String> {
    let file = project
        .files
        .iter_mut()
        .find(|file| file.relative_path == file_path)
        .ok_or_else(|| format!("No se encontró el archivo '{file_path}' en el proyecto"))?;
    file.entries = entries;
    project.updated_at = updated_at;
    Ok(())
}

fn build_single_entry_context(
    chunk: &translation_orchestrator::PlannedTranslationChunk,
    entry: &TranslationRequest,
) -> app_lib::services::provider::TranslationBatchContext {
    let mut context = chunk.context.clone();
    context.approximate_char_count = entry.text.chars().count();
    context.file_context_hints = vec![entry.file_context.clone()];
    context.previous_chunk_hints.clear();
    context.next_chunk_hints.clear();
    context
}

fn file_has_unresolved_failures(failures: &[TranslationFailureRecord], file_path: &str) -> bool {
    failures
        .iter()
        .any(|failure| failure.file_path == file_path)
}

fn main() -> ExitCode {
    let options = match parse_args(env::args()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    if let Err(error) = std::fs::create_dir_all(&options.workspace) {
        eprintln!(
            "No se pudo crear workspace '{}': {}",
            options.workspace.display(),
            error
        );
        return ExitCode::from(1);
    }
    let progress_log = options.workspace.join("qa_translate.progress.log");
    let _ = std::fs::remove_file(&progress_log);
    let project_path = options.workspace.join("project.fairtl");
    let status_path = options.workspace.join("qa_translate.status.json");
    let planner_cache_path = options.workspace.join("planner_cache.json");
    let failure_manifest_path = options.workspace.join("translation_failures.json");
    let _ = std::fs::remove_file(&failure_manifest_path);

    let mut project = if project_path.exists() {
        match persistence::load_project(&project_path) {
            Ok(mut project) => {
                match parse_game(Path::new(&project.game_dir)) {
                    Ok(parsed) => {
                        let added = merge_missing_entries(&mut project, parsed.files);
                        if added > 0 {
                            eprintln!(
                                "Se añadieron {added} entradas nuevas detectadas al reanudar."
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!("No se pudo refrescar el proyecto existente: {error}");
                        return ExitCode::from(1);
                    }
                }
                project
            }
            Err(error) => {
                eprintln!(
                    "No se pudo reanudar proyecto existente '{}': {}",
                    project_path.display(),
                    error
                );
                return ExitCode::from(1);
            }
        }
    } else {
        match build_project(
            &options.game_dir,
            &options.source_lang,
            &options.target_lang,
            options.style_guide,
        ) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("No se pudo preparar el proyecto: {error}");
                return ExitCode::from(1);
            }
        }
    };
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("No se pudo iniciar el entorno async: {error}");
            return ExitCode::from(1);
        }
    };
    let started_at = Instant::now();
    let resumed = project_path.exists();
    let provider_type = options.provider.clone();
    let primary_config = ProviderRunConfig {
        provider: provider_type.clone(),
        model: options.model.clone(),
        profile: options.profile.clone(),
        reasoning_effort: options.reasoning_effort.clone(),
        api_key_file: options.api_key_file.clone(),
        endpoint: options.endpoint.clone(),
    };
    let fallback_config = options
        .fallback_model
        .clone()
        .map(|model| ProviderRunConfig {
            provider: provider_type.clone(),
            model: Some(model),
            profile: if provider_type == ProviderType::Codex {
                options.fallback_profile.clone()
            } else {
                options.profile.clone()
            },
            reasoning_effort: if provider_type == ProviderType::Codex {
                options.fallback_reasoning_effort.clone()
            } else {
                options.reasoning_effort.clone()
            },
            api_key_file: options.api_key_file.clone(),
            endpoint: options.endpoint.clone(),
        });
    let mut active_config = primary_config.clone();
    let mut fallback_triggered = false;
    let mut models_used = vec![run_config_label(&active_config)];
    let fallback_policy = if fallback_config.is_some() {
        "usage_limit_only".to_string()
    } else {
        "none".to_string()
    };
    let project_file_total = if options.files.is_empty() {
        project.files.len()
    } else {
        options.files.len()
    };
    let coverage_repair_marked = if options.repair_coverage {
        let coverage_before_repair = validate_project_translation_coverage(
            &project,
            CoverageOptions {
                strict_unchanged: true,
                max_issues: usize::MAX,
            },
        );
        let repair_result =
            mark_project_coverage_issues_for_repair(&mut project, &coverage_before_repair);
        if repair_result.marked_entries > 0 {
            let repair_style = "Pase de reparación de cobertura: las entradas solicitadas fallaron antes la QA del idioma destino. Traduce las palabras visibles del idioma fuente al idioma destino; evita devolver texto idéntico salvo que sea un código protegido, marcador, acrónimo de RPG o nombre propio claro.";
            if !project
                .style_guide
                .iter()
                .any(|note| note.eq_ignore_ascii_case(repair_style))
            {
                project.style_guide.push(repair_style.into());
            }
            if let Err(error) = persistence::save_project(&project, &project_path) {
                eprintln!(
                    "No se pudo guardar reparación de cobertura '{}': {}",
                    project_path.display(),
                    error
                );
                return ExitCode::from(1);
            }
        }
        repair_result.marked_entries
    } else {
        0
    };
    let already_translated_entries = project
        .files
        .iter()
        .filter(|file| options.files.is_empty() || options.files.contains(&file.relative_path))
        .flat_map(|file| file.entries.iter())
        .filter(|entry| entry.status != app_lib::models::EntryStatus::Untranslated)
        .count();

    print_progress_line(&progress_log, &format!(
        "[{}] Preparado | juego={} | motor={:?} | archivos objetivo={} | proveedor={} | modelo={} | perfil={} | razonamiento={} | endpoint={} | modelo alternativo={} | reanudar={} | ya_traducidas={}",
        format_elapsed(started_at),
        project.game_dir,
        project.engine,
        project_file_total,
        provider_label(&provider_type),
        options.model.as_deref().unwrap_or("default"),
        options.profile.as_deref().unwrap_or("default"),
        options.reasoning_effort.as_deref().unwrap_or("default"),
        options.endpoint.as_deref().unwrap_or("default"),
        fallback_config
            .as_ref()
            .map(run_config_label)
            .unwrap_or_else(|| "none".to_string()),
        if resumed { "si" } else { "no" },
        already_translated_entries
    ));
    if options.repair_coverage {
        print_progress_line(
            &progress_log,
            &format!(
                "[{}] Reparación de cobertura | entradas marcadas={}",
                format_elapsed(started_at),
                coverage_repair_marked
            ),
        );
    }
    let mut status = QaTranslateStatus {
        phase: "prepared".into(),
        provider: provider_label(&provider_type).into(),
        active_model: run_config_label(&active_config),
        fallback_policy: fallback_policy.clone(),
        files_total: project_file_total,
        files_completed: 0,
        chunks_total: 0,
        chunks_completed: 0,
        entries_total: 0,
        entries_applied: already_translated_entries,
        current_file: None,
        current_chunk_index: None,
        current_file_chunk_index: None,
        current_file_chunk_total: None,
        retry_count: 0,
        unresolved_entries: 0,
        planner_cache_hit: false,
        coverage_repair_marked,
        strict_target_language: options.strict_target_language,
        coverage_complete: false,
        coverage_errors: 0,
        coverage_warnings: 0,
    };
    save_status(&status_path, &status);
    if resumed {
        print_progress_line(
            &progress_log,
            &format!(
                "[{}] Reanudando desde checkpoint '{}'",
                format_elapsed(started_at),
                project_path.display()
            ),
        );
    } else if let Err(error) = persistence::save_project(&project, &project_path) {
        eprintln!(
            "No se pudo crear checkpoint inicial '{}': {}",
            project_path.display(),
            error
        );
        return ExitCode::from(1);
    } else {
        print_progress_line(
            &progress_log,
            &format!(
                "[{}] Checkpoint inicial creado en '{}'",
                format_elapsed(started_at),
                project_path.display()
            ),
        );
    }
    print_progress_line(
        &progress_log,
        &format!(
            "[{}] Validando entorno de {}...",
            format_elapsed(started_at),
            provider_label(&provider_type)
        ),
    );

    let mut provider = match create_provider(&active_config) {
        Ok(provider) => provider,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if let Err(primary_error) = runtime.block_on(provider.validate()) {
        if is_usage_limit_error(&primary_error.to_string()) {
            if let Some(fallback) = fallback_config.as_ref() {
                print_progress_line(&progress_log, &format!(
                    "[{}] Falló la validación del modelo primario por límite de uso ({primary_error}). Intentando modelo alternativo {}",
                    format_elapsed(started_at),
                    run_config_label(fallback)
                ));
                let fallback_provider = match create_provider(fallback) {
                    Ok(provider) => provider,
                    Err(error) => {
                        eprintln!("{error}");
                        return ExitCode::from(1);
                    }
                };
                if let Err(fallback_error) = runtime.block_on(fallback_provider.validate()) {
                    eprintln!(
                        "{} no está listo. Primario: {primary_error}. Modelo alternativo: {fallback_error}",
                        provider_label(&provider_type)
                    );
                    return ExitCode::from(1);
                }
                provider = fallback_provider;
                active_config = fallback.clone();
                fallback_triggered = true;
                models_used.push(run_config_label(&active_config));
                print_progress_line(
                    &progress_log,
                    &format!(
                        "[{}] Modelo alternativo activado | {}",
                        format_elapsed(started_at),
                        run_config_label(&active_config)
                    ),
                );
            } else {
                eprintln!(
                    "{} no está listo: {primary_error}",
                    provider_label(&provider_type)
                );
                return ExitCode::from(1);
            }
        } else {
            eprintln!(
                "{} no está listo: {primary_error}",
                provider_label(&provider_type)
            );
            return ExitCode::from(1);
        }
    }
    print_progress_line(
        &progress_log,
        &format!(
            "[{}] Entorno de {} listo. Construyendo plan de traducción contextual...",
            format_elapsed(started_at),
            provider_label(&provider_type)
        ),
    );
    status.phase = "planning".into();
    status.active_model = run_config_label(&active_config);
    save_status(&status_path, &status);

    let expected_planner_fingerprint = translation_orchestrator::planner_cache_fingerprint(
        &project,
        &options.files,
        &provider_type,
    );
    let planner_cache = load_planner_cache(&planner_cache_path);
    status.planner_cache_hit = planner_cache
        .as_ref()
        .map(|cache| cache.fingerprint == expected_planner_fingerprint)
        .unwrap_or(false);
    save_status(&status_path, &status);

    let plan_result = translation_orchestrator::build_translation_plan_with_cache_and_progress(
        &project,
        &options.files,
        &provider_type,
        planner_cache.as_ref(),
        &mut |snapshot| {
            status.phase = snapshot.phase.clone();
            status.files_total = snapshot.files_total;
            status.files_completed = snapshot.files_done;
            status.entries_total = snapshot.pending_entries;
            status.chunks_total = snapshot.planned_chunks.max(status.chunks_total);
            status.current_file = snapshot.current_file.clone();
            save_status(&status_path, &status);
            if snapshot.phase == "planning" {
                print_progress_line(
                    &progress_log,
                    &format!(
                        "[{}] Planificación | archivos {}/{} | lotes={} | entradas={}{}",
                        format_elapsed(started_at),
                        snapshot.files_done,
                        snapshot.files_total,
                        snapshot.planned_chunks,
                        snapshot.pending_entries,
                        snapshot
                            .current_file
                            .as_ref()
                            .map(|file| format!(" | {}", file))
                            .unwrap_or_default()
                    ),
                );
            }
        },
    );
    let chunks = plan_result.plan;
    let _ = write_json_file(&planner_cache_path, &plan_result.planner_cache);
    if let Err(error) = translation_orchestrator::ensure_translation_plan_within_budget(
        &project,
        &chunks,
        &provider_type,
    ) {
        eprintln!("Plan de traducción inválido: {error}");
        return ExitCode::from(1);
    }
    let mut selected_file_order = Vec::new();
    let mut seen_files = HashSet::new();
    let mut file_chunk_totals = HashMap::new();
    let total_planned_entries = chunks
        .iter()
        .map(|chunk| chunk.entries.len())
        .sum::<usize>();

    for chunk in &chunks {
        *file_chunk_totals
            .entry(chunk.relative_path.clone())
            .or_insert(0usize) += 1;
        if seen_files.insert(chunk.relative_path.clone()) {
            selected_file_order.push(chunk.relative_path.clone());
        }
    }

    let selected_files = selected_file_order.len();
    let file_positions = selected_file_order
        .iter()
        .enumerate()
        .map(|(index, path)| (path.clone(), index + 1))
        .collect::<HashMap<_, _>>();
    let total_chunks = chunks.len();
    let mut translated_entries = 0usize;
    let mut validation_warnings = Vec::new();
    let mut completed_files = HashSet::new();
    let mut failure_records = Vec::<TranslationFailureRecord>::new();
    let mut retry_count_total = 0usize;
    status.phase = "translating".into();
    status.files_total = selected_files;
    status.chunks_total = total_chunks;
    status.entries_total = total_planned_entries;
    status.entries_applied = translated_entries;
    save_status(&status_path, &status);

    print_progress_line(
        &progress_log,
        &format!(
            "[{}] Inicio | {} | archivos={} | lotes={} | entradas={}",
            format_elapsed(started_at),
            run_config_label(&active_config),
            selected_files,
            total_chunks,
            total_planned_entries
        ),
    );

    for (chunk_idx, chunk) in chunks.into_iter().enumerate() {
        let file_position = *file_positions.get(&chunk.relative_path).unwrap_or(&0usize);
        let file_total_chunks = *file_chunk_totals
            .get(&chunk.relative_path)
            .unwrap_or(&chunk.context.total_chunks);
        let completed_chunk_bar = render_progress_bar(chunk_idx, total_chunks, 24);
        let completed_entry_bar =
            render_progress_bar(translated_entries, total_planned_entries, 24);

        print_progress_line(&progress_log, &format!(
            "[{}] {} lotes {}/{} | {} entradas {}/{} | archivo {}/{} | completados {}/{} | {} lote {}/{} | entradas={}",
            format_elapsed(started_at),
            completed_chunk_bar,
            chunk_idx + 1,
            total_chunks,
            completed_entry_bar,
            translated_entries,
            total_planned_entries,
            file_position,
            selected_files,
            completed_files.len(),
            selected_files,
            chunk.relative_path,
            chunk.context.chunk_index,
            file_total_chunks,
            chunk.entries.len()
        ));
        status.current_file = Some(chunk.relative_path.clone());
        status.current_chunk_index = Some(chunk_idx + 1);
        status.current_file_chunk_index = Some(chunk.context.chunk_index);
        status.current_file_chunk_total = Some(file_total_chunks);
        status.chunks_completed = chunk_idx;
        status.files_completed = completed_files.len();
        status.entries_applied = translated_entries;
        save_status(&status_path, &status);

        let mut attempts_with_active_model = 0usize;
        let mut force_entry_isolation = false;
        let mut forced_isolation_error = None::<String>;
        loop {
            attempts_with_active_model += 1;
            if attempts_with_active_model > 1 {
                retry_count_total += 1;
                status.retry_count = retry_count_total;
                save_status(&status_path, &status);
                print_progress_line(
                    &progress_log,
                    &format!(
                        "[{}] Reintentando {} lote {}/{} | intento {}/{} con {}",
                        format_elapsed(started_at),
                        chunk.relative_path,
                        chunk.context.chunk_index,
                        file_total_chunks,
                        attempts_with_active_model,
                        MAX_RETRYABLE_CHUNK_ATTEMPTS,
                        run_config_label(&active_config)
                    ),
                );
            }

            let responses = match translate_with_usage_limit_fallback(
                &runtime,
                &mut provider,
                &mut active_config,
                fallback_config.as_ref(),
                &mut fallback_triggered,
                &mut models_used,
                &progress_log,
                started_at,
                &chunk.relative_path,
                chunk.context.chunk_index,
                chunk.context.total_chunks,
                &chunk.entries,
                &project.source_lang,
                &project.target_lang,
                &project.glossary,
                &chunk.context,
            ) {
                Ok(responses) => responses,
                Err(error) => {
                    if is_provider_content_inspection_error(&error) {
                        force_entry_isolation = true;
                        forced_isolation_error = Some(error.clone());
                        print_progress_line(
                            &progress_log,
                            &format!(
                                "[{}] Inspección de contenido del proveedor en {} lote {}/{} con {} | {}",
                                format_elapsed(started_at),
                                chunk.relative_path,
                                chunk.context.chunk_index,
                                file_total_chunks,
                                run_config_label(&active_config),
                                error
                            ),
                        );
                        if let Some(content_fallback) = fallback_config.as_ref() {
                            print_progress_line(
                                &progress_log,
                                &format!(
                                    "[{}] Probando fallback de lote por inspección | {}",
                                    format_elapsed(started_at),
                                    run_config_label(content_fallback)
                                ),
                            );
                            match create_provider(content_fallback) {
                                Ok(fallback_provider) => {
                                    match runtime.block_on(fallback_provider.translate_batch(
                                        &chunk.entries,
                                        &project.source_lang,
                                        &project.target_lang,
                                        &project.glossary,
                                        &chunk.context,
                                    )) {
                                        Ok(responses) => responses,
                                        Err(fallback_error) => {
                                            forced_isolation_error = Some(format!(
                                                "{error}; fallback de lote falló: {fallback_error}"
                                            ));
                                            print_progress_line(
                                            &progress_log,
                                            &format!(
                                                "[{}] Fallback de lote falló; se aislarán entradas | {}",
                                                format_elapsed(started_at),
                                                fallback_error
                                            ),
                                        );
                                            Vec::new()
                                        }
                                    }
                                }
                                Err(fallback_error) => {
                                    forced_isolation_error = Some(format!(
                                        "{error}; no se pudo crear fallback de lote: {fallback_error}"
                                    ));
                                    print_progress_line(
                                        &progress_log,
                                        &format!(
                                            "[{}] No se pudo crear fallback de lote; se aislarán entradas | {}",
                                            format_elapsed(started_at),
                                            fallback_error
                                        ),
                                    );
                                    Vec::new()
                                }
                            }
                        } else {
                            print_progress_line(
                                &progress_log,
                                &format!(
                                    "[{}] Sin fallback configurado; se aislarán entradas",
                                    format_elapsed(started_at)
                                ),
                            );
                            Vec::new()
                        }
                    } else if is_retryable_provider_shape_error(&error) {
                        print_progress_line(
                            &progress_log,
                            &format!(
                                "[{}] Respuesta de proveedor fuera de esquema en {} lote {}/{} con {} | {}",
                                format_elapsed(started_at),
                                chunk.relative_path,
                                chunk.context.chunk_index,
                                file_total_chunks,
                                run_config_label(&active_config),
                                error
                            ),
                        );

                        if attempts_with_active_model < MAX_RETRYABLE_CHUNK_ATTEMPTS {
                            continue;
                        }

                        print_progress_line(
                            &progress_log,
                            &format!(
                                "[{}] El lote sigue fuera de esquema; se aislarán sus entradas una por una | {} lote {}/{}",
                                format_elapsed(started_at),
                                chunk.relative_path,
                                chunk.context.chunk_index,
                                file_total_chunks
                            ),
                        );
                        Vec::new()
                    } else {
                        eprintln!("{error}");
                        return ExitCode::from(1);
                    }
                }
            };
            status.active_model = run_config_label(&active_config);
            save_status(&status_path, &status);

            let response_ids = responses
                .iter()
                .map(|response| response.entry_id.as_str())
                .collect::<HashSet<_>>();
            let requested_ids = chunk
                .entries
                .iter()
                .map(|request| request.entry_id.as_str())
                .collect::<HashSet<_>>();
            let missing_response_count = chunk
                .entries
                .iter()
                .filter(|request| !response_ids.contains(request.entry_id.as_str()))
                .count();
            let unexpected_response_count = responses
                .iter()
                .filter(|response| !requested_ids.contains(response.entry_id.as_str()))
                .count();
            let (file_snapshot, updated_at_snapshot) =
                match snapshot_file_state(&project, &chunk.relative_path) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        eprintln!("No se pudo preparar rollback de lote: {error}");
                        return ExitCode::from(1);
                    }
                };

            match translation_apply::apply_provider_responses(
                &mut project,
                &chunk.relative_path,
                &responses,
            ) {
                Ok(result) => {
                    let chunk_has_issues = missing_response_count > 0
                        || unexpected_response_count > 0
                        || !result.missing_project_entries.is_empty()
                        || !result.invalid_entries.is_empty();

                    if chunk_has_issues {
                        if let Err(error) = restore_file_state(
                            &mut project,
                            &chunk.relative_path,
                            file_snapshot,
                            updated_at_snapshot,
                        ) {
                            eprintln!(
                                "No se pudo revertir lote parcial '{}': {error}",
                                chunk.relative_path
                            );
                            return ExitCode::from(1);
                        }

                        let mut issue_bits = Vec::new();
                        if missing_response_count > 0 {
                            issue_bits.push(format!("faltan_respuestas={missing_response_count}"));
                        }
                        if unexpected_response_count > 0 {
                            issue_bits.push(format!("ids_inesperados={unexpected_response_count}"));
                        }
                        if !result.missing_project_entries.is_empty() {
                            issue_bits.push(format!(
                                "sin_match_en_proyecto={}",
                                result.missing_project_entries.len()
                            ));
                        }
                        if !result.invalid_entries.is_empty() {
                            issue_bits.push(format!("inválidas={}", result.invalid_entries.len()));
                        }

                        print_progress_line(
                            &progress_log,
                            &format!(
                                "[{}] Respuesta inválida en {} lote {}/{} con {} | {}",
                                format_elapsed(started_at),
                                chunk.relative_path,
                                chunk.context.chunk_index,
                                file_total_chunks,
                                run_config_label(&active_config),
                                issue_bits.join(", ")
                            ),
                        );
                        for warning in &result.warnings {
                            print_progress_line(
                                &progress_log,
                                &format!("[{}]   aviso: {}", format_elapsed(started_at), warning),
                            );
                        }

                        if !force_entry_isolation
                            && attempts_with_active_model < MAX_RETRYABLE_CHUNK_ATTEMPTS
                        {
                            continue;
                        }

                        print_progress_line(
                            &progress_log,
                            &format!(
                                "[{}] Aislando subconjunto valido y reintentando entradas conflictivas en {}",
                                format_elapsed(started_at),
                                chunk.relative_path
                            ),
                        );

                        let mut valid_responses = Vec::<TranslationResponse>::new();
                        let mut invalid_details = HashMap::<String, Vec<String>>::new();
                        let mut missing_ids = HashSet::<String>::new();
                        for request in &chunk.entries {
                            let Some(response) = responses
                                .iter()
                                .find(|response| response.entry_id == request.entry_id)
                                .cloned()
                            else {
                                missing_ids.insert(request.entry_id.clone());
                                continue;
                            };

                            let (trial_entries, trial_updated_at) = match snapshot_file_state(
                                &project,
                                &chunk.relative_path,
                            ) {
                                Ok(snapshot) => snapshot,
                                Err(error) => {
                                    eprintln!(
                                            "No se pudo preparar clasificación individual de lote: {error}"
                                        );
                                    return ExitCode::from(1);
                                }
                            };
                            let trial_result = translation_apply::apply_provider_responses(
                                &mut project,
                                &chunk.relative_path,
                                std::slice::from_ref(&response),
                            );
                            let _ = restore_file_state(
                                &mut project,
                                &chunk.relative_path,
                                trial_entries,
                                trial_updated_at,
                            );
                            match trial_result {
                                Ok(single_result)
                                    if single_result.applied_entries == 1
                                        && single_result.invalid_entries.is_empty()
                                        && single_result.missing_project_entries.is_empty() =>
                                {
                                    valid_responses.push(response);
                                }
                                Ok(single_result) => {
                                    invalid_details.insert(
                                        request.entry_id.clone(),
                                        single_result.warnings.clone(),
                                    );
                                }
                                Err(error) => {
                                    invalid_details
                                        .insert(request.entry_id.clone(), vec![error.to_string()]);
                                }
                            }
                        }

                        if !valid_responses.is_empty() {
                            let clean_result = match translation_apply::apply_provider_responses(
                                &mut project,
                                &chunk.relative_path,
                                &valid_responses,
                            ) {
                                Ok(result) => result,
                                Err(error) => {
                                    eprintln!(
                                        "No se pudo aplicar subconjunto limpio en '{}': {error}",
                                        chunk.relative_path
                                    );
                                    return ExitCode::from(1);
                                }
                            };
                            translated_entries += clean_result.applied_entries;
                            validation_warnings.extend(clean_result.warnings.clone());
                            if let Err(error) = persistence::save_project(&project, &project_path) {
                                eprintln!(
                                    "No se pudo guardar checkpoint parcial tras '{}' lote {}/{}: {}",
                                    chunk.relative_path,
                                    chunk.context.chunk_index,
                                    file_total_chunks,
                                    error
                                );
                                return ExitCode::from(1);
                            }
                            print_progress_line(
                                &progress_log,
                                &format!(
                                    "[{}] Subconjunto limpio aplicado en {} lote {}/{} | aplicadas +{}",
                                    format_elapsed(started_at),
                                    chunk.relative_path,
                                    chunk.context.chunk_index,
                                    file_total_chunks,
                                    clean_result.applied_entries
                                ),
                            );
                        }

                        for request in &chunk.entries {
                            if valid_responses
                                .iter()
                                .any(|response| response.entry_id == request.entry_id)
                            {
                                continue;
                            }

                            let reason_prefix = if forced_isolation_error.is_some() {
                                "inspección de contenido del proveedor".to_string()
                            } else if missing_ids.contains(&request.entry_id) {
                                "respuesta omitida".to_string()
                            } else {
                                "integridad inválida".to_string()
                            };
                            if let Some(error) = forced_isolation_error.as_ref() {
                                invalid_details
                                    .entry(request.entry_id.clone())
                                    .or_default()
                                    .push(error.clone());
                            }
                            let single_context = build_single_entry_context(&chunk, request);
                            let mut single_success = false;

                            for single_attempt in 1..=MAX_RETRYABLE_CHUNK_ATTEMPTS {
                                if single_attempt > 1 {
                                    retry_count_total += 1;
                                    status.retry_count = retry_count_total;
                                    save_status(&status_path, &status);
                                    print_progress_line(
                                        &progress_log,
                                        &format!(
                                            "[{}] Reintentando entry {} | intento {}/{}",
                                            format_elapsed(started_at),
                                            request.entry_id,
                                            single_attempt,
                                            MAX_RETRYABLE_CHUNK_ATTEMPTS
                                        ),
                                    );
                                }

                                let single_responses = match translate_with_usage_limit_fallback(
                                    &runtime,
                                    &mut provider,
                                    &mut active_config,
                                    fallback_config.as_ref(),
                                    &mut fallback_triggered,
                                    &mut models_used,
                                    &progress_log,
                                    started_at,
                                    &chunk.relative_path,
                                    chunk.context.chunk_index,
                                    chunk.context.total_chunks,
                                    std::slice::from_ref(request),
                                    &project.source_lang,
                                    &project.target_lang,
                                    &project.glossary,
                                    &single_context,
                                ) {
                                    Ok(responses) => responses,
                                    Err(error) => {
                                        if is_provider_content_inspection_error(&error) {
                                            if let Some(content_fallback) = fallback_config.as_ref()
                                            {
                                                print_progress_line(
                                                    &progress_log,
                                                    &format!(
                                                        "[{}] Entry {} bloqueada por inspección; probando fallback individual {}",
                                                        format_elapsed(started_at),
                                                        request.entry_id,
                                                        run_config_label(content_fallback)
                                                    ),
                                                );
                                                match create_provider(content_fallback) {
                                                    Ok(fallback_provider) => {
                                                        match runtime.block_on(
                                                            fallback_provider.translate_batch(
                                                                std::slice::from_ref(request),
                                                                &project.source_lang,
                                                                &project.target_lang,
                                                                &project.glossary,
                                                                &single_context,
                                                            ),
                                                        ) {
                                                            Ok(responses) => responses,
                                                            Err(fallback_error) => {
                                                                invalid_details
                                                                    .entry(request.entry_id.clone())
                                                                    .or_default()
                                                                    .push(format!(
                                                                        "{error}; fallback individual falló: {fallback_error}"
                                                                    ));
                                                                continue;
                                                            }
                                                        }
                                                    }
                                                    Err(fallback_error) => {
                                                        invalid_details
                                                            .entry(request.entry_id.clone())
                                                            .or_default()
                                                            .push(format!(
                                                                "{error}; no se pudo crear fallback individual: {fallback_error}"
                                                            ));
                                                        continue;
                                                    }
                                                }
                                            } else {
                                                invalid_details
                                                    .entry(request.entry_id.clone())
                                                    .or_default()
                                                    .push(error);
                                                continue;
                                            }
                                        } else {
                                            invalid_details
                                                .entry(request.entry_id.clone())
                                                .or_default()
                                                .push(error);
                                            continue;
                                        }
                                    }
                                };

                                let (trial_entries, trial_updated_at) = match snapshot_file_state(
                                    &project,
                                    &chunk.relative_path,
                                ) {
                                    Ok(snapshot) => snapshot,
                                    Err(error) => {
                                        eprintln!(
                                                "No se pudo preparar snapshot para retry individual: {error}"
                                            );
                                        return ExitCode::from(1);
                                    }
                                };
                                match translation_apply::apply_provider_responses(
                                    &mut project,
                                    &chunk.relative_path,
                                    &single_responses,
                                ) {
                                    Ok(single_result)
                                        if single_result.applied_entries == 1
                                            && single_result.invalid_entries.is_empty()
                                            && single_result.missing_project_entries.is_empty() =>
                                    {
                                        translated_entries += 1;
                                        validation_warnings.extend(single_result.warnings.clone());
                                        if let Err(error) =
                                            persistence::save_project(&project, &project_path)
                                        {
                                            eprintln!(
                                                "No se pudo guardar checkpoint tras retry individual '{}': {}",
                                                request.entry_id,
                                                error
                                            );
                                            return ExitCode::from(1);
                                        }
                                        print_progress_line(
                                            &progress_log,
                                            &format!(
                                                "[{}] Entrada aislada aplicada | {}",
                                                format_elapsed(started_at),
                                                request.entry_id
                                            ),
                                        );
                                        single_success = true;
                                        break;
                                    }
                                    Ok(single_result) => {
                                        let _ = restore_file_state(
                                            &mut project,
                                            &chunk.relative_path,
                                            trial_entries,
                                            trial_updated_at,
                                        );
                                        invalid_details
                                            .entry(request.entry_id.clone())
                                            .or_default()
                                            .extend(single_result.warnings.clone());
                                    }
                                    Err(error) => {
                                        let _ = restore_file_state(
                                            &mut project,
                                            &chunk.relative_path,
                                            trial_entries,
                                            trial_updated_at,
                                        );
                                        invalid_details
                                            .entry(request.entry_id.clone())
                                            .or_default()
                                            .push(error.to_string());
                                    }
                                }
                            }

                            if !single_success {
                                let warnings = invalid_details
                                    .remove(&request.entry_id)
                                    .unwrap_or_default();
                                failure_records.push(TranslationFailureRecord {
                                    file_path: chunk.relative_path.clone(),
                                    entry_id: request.entry_id.clone(),
                                    stage: "single-entry-retry".into(),
                                    reason: reason_prefix,
                                    warnings,
                                });
                                status.unresolved_entries = failure_records.len();
                                save_status(&status_path, &status);
                                save_failures(&failure_manifest_path, &failure_records);
                                print_progress_line(
                                    &progress_log,
                                    &format!(
                                        "[{}] Entrada pendiente para revisión manual | {}",
                                        format_elapsed(started_at),
                                        request.entry_id
                                    ),
                                );
                            }
                        }

                        if chunk.context.chunk_index >= file_total_chunks
                            && !file_has_unresolved_failures(&failure_records, &chunk.relative_path)
                        {
                            completed_files.insert(chunk.relative_path.clone());
                        }
                        break;
                    }

                    translated_entries += result.applied_entries;
                    validation_warnings.extend(result.warnings);
                    if chunk.context.chunk_index >= file_total_chunks {
                        completed_files.insert(chunk.relative_path.clone());
                    }
                    if let Err(error) = persistence::save_project(&project, &project_path) {
                        eprintln!(
                            "No se pudo guardar checkpoint tras '{}' lote {}/{}: {}",
                            chunk.relative_path,
                            chunk.context.chunk_index,
                            file_total_chunks,
                            error
                        );
                        return ExitCode::from(1);
                    }

                    let finished_chunk_bar = render_progress_bar(chunk_idx + 1, total_chunks, 24);
                    let finished_entry_bar =
                        render_progress_bar(translated_entries, total_planned_entries, 24);
                    print_progress_line(&progress_log, &format!(
                        "[{}] {} lotes {}/{} | {} entradas {}/{} | archivo {}/{} | completados {}/{} | completado {} lote {}/{} | aplicadas +{}",
                        format_elapsed(started_at),
                        finished_chunk_bar,
                        chunk_idx + 1,
                        total_chunks,
                        finished_entry_bar,
                        translated_entries,
                        total_planned_entries,
                        file_position,
                        selected_files,
                        completed_files.len(),
                        selected_files,
                        chunk.relative_path,
                        chunk.context.chunk_index,
                        file_total_chunks,
                        result.applied_entries
                    ));
                    print_progress_line(
                        &progress_log,
                        &format!(
                            "[{}] Checkpoint guardado | {}",
                            format_elapsed(started_at),
                            project_path.display()
                        ),
                    );
                    status.chunks_completed = chunk_idx + 1;
                    status.files_completed = completed_files.len();
                    status.entries_applied = translated_entries;
                    status.unresolved_entries = failure_records.len();
                    save_status(&status_path, &status);
                    break;
                }
                Err(error) => {
                    eprintln!("No se pudieron aplicar traducciones: {error}");
                    return ExitCode::from(1);
                }
            }
        }
    }
    if let Err(error) = persistence::save_project(&project, &project_path) {
        eprintln!(
            "No se pudo guardar proyecto '{}': {}",
            project_path.display(),
            error
        );
        return ExitCode::from(1);
    }

    if !failure_records.is_empty() {
        save_failures(&failure_manifest_path, &failure_records);
        status.phase = "needs-review".into();
        status.entries_applied = translated_entries;
        status.unresolved_entries = failure_records.len();
        status.chunks_completed = total_chunks;
        status.files_completed = completed_files.len();
        save_status(&status_path, &status);
        eprintln!(
            "La corrida terminó con {} entradas pendientes para revisión manual. Revisa '{}'.",
            failure_records.len(),
            failure_manifest_path.display()
        );
        return ExitCode::from(1);
    }

    let export_dir = options.workspace.join("export-es");
    match prepare_workspace_export_dir(&export_dir) {
        Ok(true) => {
            print_progress_line(
                &progress_log,
                &format!(
                    "[{}] Export anterior eliminado | {}",
                    format_elapsed(started_at),
                    export_dir.display()
                ),
            );
        }
        Ok(false) => {}
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    }
    let export_result = match exporter::export_project(&project, &export_dir) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("No se pudo exportar '{}': {}", export_dir.display(), error);
            return ExitCode::from(1);
        }
    };
    validation_warnings.extend(export_result.warnings.clone());
    let playable_dir = match create_playable_copy_if_nested_mv_mz_www(
        Path::new(&project.game_dir),
        &export_dir,
        &options.workspace,
    ) {
        Ok(Some(path)) => {
            print_progress_line(
                &progress_log,
                &format!(
                    "[{}] Copia jugable creada | {}",
                    format_elapsed(started_at),
                    path.display()
                ),
            );
            Some(path)
        }
        Ok(None) => None,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    let coverage = validate_project_translation_coverage(
        &project,
        CoverageOptions {
            strict_unchanged: options.strict_target_language,
            ..CoverageOptions::default()
        },
    );
    status.strict_target_language = options.strict_target_language;
    status.coverage_complete = coverage.complete;
    status.coverage_errors = coverage.error_count;
    status.coverage_warnings = coverage.warning_count;

    let report = RunReport {
        game_dir: project.game_dir.clone(),
        project_path: project_path.display().to_string(),
        export_dir: export_dir.display().to_string(),
        playable_dir: playable_dir.as_ref().map(|path| path.display().to_string()),
        files_selected: selected_files,
        translated_entries,
        unresolved_entries: failure_records.len(),
        validation_warnings,
        provider: provider_label(&provider_type).to_string(),
        model: options.model,
        profile: options.profile,
        reasoning_effort: options.reasoning_effort,
        fallback_model: options.fallback_model,
        fallback_profile: options.fallback_profile,
        fallback_reasoning_effort: options.fallback_reasoning_effort,
        fallback_triggered,
        fallback_policy,
        retry_count: retry_count_total,
        models_used,
        coverage_repair_marked,
        strict_target_language: options.strict_target_language,
        coverage,
    };

    match serde_json::to_string_pretty(&report) {
        Ok(text) => {
            status.phase = if report.coverage.complete {
                "completed".into()
            } else {
                "needs-review".into()
            };
            status.entries_applied = translated_entries;
            status.chunks_completed = total_chunks;
            status.files_completed = completed_files.len();
            save_status(&status_path, &status);
            print_progress_line(
                &progress_log,
                &format!(
                    "[{}] Fin | archivos={}/{} | entradas={}/{} | avisos={} | cobertura={} errores={} avisos_cobertura={}",
                    format_elapsed(started_at),
                    completed_files.len(),
                    selected_files,
                    translated_entries,
                    total_planned_entries,
                    report.validation_warnings.len(),
                    if report.coverage.complete { "PASS" } else { "FAIL" },
                    report.coverage.error_count,
                    report.coverage.warning_count
                ),
            );
            println!("{text}");
            if !report.coverage.complete {
                eprintln!(
                    "La cobertura de traducción no cumple el criterio de idioma destino. Revisa el campo 'coverage' del reporte."
                );
                return ExitCode::from(1);
            }
        }
        Err(error) => {
            eprintln!("No se pudo serializar reporte: {error}");
            return ExitCode::from(1);
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{
        create_playable_copy_if_nested_mv_mz_www, is_provider_content_inspection_error,
        is_retryable_provider_shape_error, merge_missing_entries, parse_args,
        prepare_workspace_export_dir, read_opencode_api_key_file,
    };
    use app_lib::models::{
        Engine, EntryStatus, Project, ProviderType, TranslationEntry, TranslationFile,
    };

    #[test]
    fn provider_shape_errors_are_retryable() {
        assert!(is_retryable_provider_shape_error(
            "API error: Provider returned 23 translations but expected 20"
        ));
        assert!(is_retryable_provider_shape_error(
            "Provider returned an unexpected local entry ID: 25"
        ));
        assert!(is_retryable_provider_shape_error(
            "Provider omitted a translation for entry ID Map001:1:0:text:0"
        ));
        assert!(is_retryable_provider_shape_error(
            "Failed to parse OpenCode response as translation JSON: No se pudo leer la respuesta del proveedor"
        ));
        assert!(!is_retryable_provider_shape_error(
            "Codex CLI is installed but not authenticated"
        ));
    }

    #[test]
    fn provider_content_inspection_errors_trigger_isolation() {
        assert!(is_provider_content_inspection_error(
            "InternalError.Algo.DataInspectionFailed: Input text data may contain inappropriate content."
        ));
        assert!(is_provider_content_inspection_error(
            "data may contain inappropriate content"
        ));
        assert!(!is_provider_content_inspection_error(
            "Provider returned 12 translations but expected 10"
        ));
    }

    #[test]
    fn prepare_workspace_export_dir_removes_existing_tree() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-qa-provider-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let export_dir = temp.join("export-es");
        std::fs::create_dir_all(export_dir.join("nested")).unwrap();
        std::fs::write(export_dir.join("nested").join("file.txt"), "ok").unwrap();

        let removed = prepare_workspace_export_dir(&export_dir).unwrap();

        assert!(removed);
        assert!(!export_dir.exists());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn prepare_workspace_export_dir_is_noop_when_missing() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-qa-provider-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let export_dir = temp.join("export-es");

        let removed = prepare_workspace_export_dir(&export_dir).unwrap();

        assert!(!removed);
        assert!(!export_dir.exists());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn creates_playable_copy_for_nested_mv_www_exports() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-qa-provider-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let game_root = temp.join("SourceGame");
        let game_www = game_root.join("www");
        let export_dir = temp.join("run").join("export-es");
        let workspace = temp.join("run");
        std::fs::create_dir_all(game_www.join("data")).unwrap();
        std::fs::create_dir_all(export_dir.join("data")).unwrap();
        std::fs::write(game_root.join("Game.exe"), "launcher").unwrap();
        std::fs::write(game_root.join("readme.txt"), "keep").unwrap();
        std::fs::write(game_www.join("data").join("System.json"), "original").unwrap();
        std::fs::write(export_dir.join("data").join("System.json"), "translated").unwrap();

        let playable = create_playable_copy_if_nested_mv_mz_www(&game_www, &export_dir, &workspace)
            .unwrap()
            .unwrap();

        assert_eq!(playable, workspace.join("playable-es"));
        assert_eq!(
            std::fs::read_to_string(playable.join("Game.exe")).unwrap(),
            "launcher"
        );
        assert_eq!(
            std::fs::read_to_string(playable.join("readme.txt")).unwrap(),
            "keep"
        );
        assert_eq!(
            std::fs::read_to_string(playable.join("www").join("data").join("System.json")).unwrap(),
            "translated"
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn parse_args_accepts_repair_coverage_flag() {
        let options = parse_args(
            [
                "qa_translate_provider",
                "--game-dir",
                "D:\\Games\\Demo",
                "--repair-coverage",
                "--strict-target-language",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();

        assert!(options.repair_coverage);
        assert!(options.strict_target_language);
    }

    #[test]
    fn reads_opencode_go_key_from_auth_json() {
        let temp = std::env::temp_dir().join(format!(
            "fairtranslator-qa-provider-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let auth_path = temp.join("auth.json");
        std::fs::write(
            &auth_path,
            r#"{"opencode-go":{"type":"api","key":"sk-test-local"}}"#,
        )
        .unwrap();

        let key = read_opencode_api_key_file(&auth_path, &ProviderType::Opencode).unwrap();

        assert_eq!(key, "sk-test-local");
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn resume_adds_only_newly_extracted_entries() {
        let existing = TranslationEntry {
            id: "CommonEvents:1:text:0".into(),
            file_context: "text".into(),
            original: "Hello".into(),
            translation: "Hola".into(),
            status: EntryStatus::Draft,
            notes: String::new(),
        };
        let new_entry = TranslationEntry {
            id: "CommonEvents:1:variableText:1".into(),
            file_context: "variable text".into(),
            original: "Waste Plant".into(),
            translation: String::new(),
            status: EntryStatus::Untranslated,
            notes: String::new(),
        };
        let mut project = Project {
            id: "test".into(),
            name: "test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: String::new(),
            files: vec![TranslationFile {
                relative_path: "data/CommonEvents.json".into(),
                entries: vec![existing.clone()],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let added = merge_missing_entries(
            &mut project,
            vec![TranslationFile {
                relative_path: "data/CommonEvents.json".into(),
                entries: vec![existing, new_entry],
            }],
        );

        assert_eq!(added, 1);
        assert_eq!(project.files[0].entries.len(), 2);
        assert_eq!(project.files[0].entries[0].translation, "Hola");
    }
}

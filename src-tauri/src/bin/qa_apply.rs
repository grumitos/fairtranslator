use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app_lib::models::{AiBatchEntry, EntryStatus, TranslationFile};
use app_lib::parser::parse_game;
use app_lib::services::{ai_batch, exporter, persistence};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
struct Options {
    manifest: PathBuf,
    status: EntryStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArtifactRow {
    index: usize,
    batch_id: String,
    entries_path: String,
    response_template_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SampleManifest {
    sample_name: String,
    project_path: String,
    exports_dir: String,
    batch_dir: String,
    batches: Vec<BatchArtifactRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyReport {
    sample_name: String,
    manifest_path: String,
    applied_entries: usize,
    skipped_entries: usize,
    batches_applied: usize,
    validation_warnings: Vec<String>,
    export_dir: String,
    export_files: usize,
    export_entries_applied: usize,
    reopened_engine: String,
    reopened_files: usize,
    reopened_entries: usize,
}

fn print_usage() {
    eprintln!(
        "Uso: cargo run --bin qa_apply -- [opciones]\n\
         \n\
         Opciones:\n\
           --manifest <ruta>      Manifest generado por qa_prepare\n\
           --sample-dir <ruta>    Directorio de muestra; asume <ruta>\\manifest.json\n\
           --status <estado>      draft | reviewed | final (default: reviewed)\n\
           --help, -h             Muestra esta ayuda"
    );
}

fn parse_status(value: &str) -> Result<EntryStatus, String> {
    match value.to_ascii_lowercase().as_str() {
        "draft" => Ok(EntryStatus::Draft),
        "reviewed" => Ok(EntryStatus::Reviewed),
        "final" => Ok(EntryStatus::Final),
        _ => Err(format!("Estado invalido: {value}")),
    }
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut manifest = None;
    let mut status = EntryStatus::Reviewed;

    let mut args = argv.skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --manifest".to_string())?;
                manifest = Some(PathBuf::from(value));
            }
            "--sample-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --sample-dir".to_string())?;
                manifest = Some(PathBuf::from(value).join("manifest.json"));
            }
            "--status" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --status".to_string())?;
                status = parse_status(&value)?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ if arg.starts_with("--") => return Err(format!("Opcion desconocida: {arg}")),
            _ => return Err(format!("Argumento inesperado: {arg}")),
        }
    }

    let Some(manifest) = manifest else {
        return Err("Debes indicar --manifest o --sample-dir".to_string());
    };

    Ok(Options { manifest, status })
}

fn inventory_signature(files: &[TranslationFile]) -> Vec<(String, usize)> {
    let mut signature = files
        .iter()
        .map(|file| (file.relative_path.clone(), file.entries.len()))
        .collect::<Vec<_>>();
    signature.sort_by(|left, right| left.0.cmp(&right.0));
    signature
}

fn total_entries(files: &[TranslationFile]) -> usize {
    files.iter().map(|file| file.entries.len()).sum()
}

fn load_manifest(path: &Path) -> Result<SampleManifest, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn resolve_response_path(template_path: &Path) -> PathBuf {
    if let Some(template_name) = template_path.file_name().and_then(|value| value.to_str()) {
        let candidate = template_path
            .with_file_name(template_name.replace(".response.template.json", ".response.json"));
        if candidate.exists() {
            return candidate;
        }
    }

    template_path.to_path_buf()
}

fn validate_response_complete(
    expected_entries: &[AiBatchEntry],
    translations: &[app_lib::models::AiBatchTranslation],
) -> Result<(), String> {
    if translations.len() != expected_entries.len() {
        return Err(format!(
            "Respuesta incompleta: esperaba {} entradas y recibi {}",
            expected_entries.len(),
            translations.len()
        ));
    }

    let expected = expected_entries
        .iter()
        .map(|entry| format!("{}::{}", entry.file_path, entry.entry_id))
        .collect::<HashSet<_>>();
    let received = translations
        .iter()
        .map(|entry| format!("{}::{}", entry.file_path, entry.entry_id))
        .collect::<HashSet<_>>();

    if expected != received {
        let missing = expected.difference(&received).cloned().collect::<Vec<_>>();
        let unexpected = received.difference(&expected).cloned().collect::<Vec<_>>();
        return Err(format!(
            "IDs no coinciden; faltantes={}, inesperados={}",
            missing.len(),
            unexpected.len()
        ));
    }

    let unresolved = translations
        .iter()
        .filter(|entry| entry.translation.trim().is_empty() && !entry.skip)
        .count();
    if unresolved > 0 {
        return Err(format!(
            "Respuesta incompleta: {} entradas siguen sin translation ni skip",
            unresolved
        ));
    }

    Ok(())
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

    let manifest = match load_manifest(&options.manifest) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!(
                "No se pudo leer manifest '{}': {}",
                options.manifest.display(),
                error
            );
            return ExitCode::from(1);
        }
    };

    let project_path = PathBuf::from(&manifest.project_path);
    let mut project = match persistence::load_project(&project_path) {
        Ok(project) => project,
        Err(error) => {
            eprintln!(
                "No se pudo abrir proyecto '{}': {}",
                project_path.display(),
                error
            );
            return ExitCode::from(1);
        }
    };

    let expected_signature = inventory_signature(&project.files);
    let expected_entries = total_entries(&project.files);
    let mut applied_entries = 0usize;
    let mut skipped_entries = 0usize;
    let mut validation_warnings = Vec::new();

    for batch in &manifest.batches {
        let entries_path = PathBuf::from(&batch.entries_path);
        let template_path = PathBuf::from(&batch.response_template_path);
        let response_path = resolve_response_path(&template_path);

        let expected_entries_batch: Vec<AiBatchEntry> = match fs::read_to_string(&entries_path)
            .map_err(|error| error.to_string())
            .and_then(|content| serde_json::from_str(&content).map_err(|error| error.to_string()))
        {
            Ok(entries) => entries,
            Err(error) => {
                eprintln!(
                    "No se pudo leer entradas esperadas para batch {}: {}",
                    batch.index, error
                );
                return ExitCode::from(1);
            }
        };

        let raw_response = match fs::read_to_string(&response_path) {
            Ok(content) => content,
            Err(error) => {
                eprintln!(
                    "No se pudo leer respuesta para batch {} en '{}': {}",
                    batch.index,
                    response_path.display(),
                    error
                );
                return ExitCode::from(1);
            }
        };

        let translations = match ai_batch::parse_ai_batch_response(
            &raw_response,
            Some(&project.id),
            Some(&batch.batch_id),
        ) {
            Ok(translations) => translations,
            Err(_error) => {
                let response_map = expected_entries_batch
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| app_lib::models::AiBatchResponseRef {
                        local_id: u16::try_from(index + 1).unwrap_or(u16::MAX),
                        file_path: entry.file_path.clone(),
                        entry_id: entry.entry_id.clone(),
                    })
                    .collect::<Vec<_>>();
                match ai_batch::parse_ai_batch_response_with_refs(
                    &raw_response,
                    Some(&project.id),
                    Some(&batch.batch_id),
                    Some(&response_map),
                ) {
                    Ok(translations) => translations,
                    Err(compact_error) => {
                        eprintln!(
                            "Respuesta invalida para batch {} ({}): {}",
                            batch.index,
                            response_path.display(),
                            compact_error
                        );
                        return ExitCode::from(1);
                    }
                }
            }
        };

        if let Err(error) = validate_response_complete(&expected_entries_batch, &translations) {
            eprintln!(
                "Respuesta incompleta para batch {} ({}): {}",
                batch.index,
                response_path.display(),
                error
            );
            return ExitCode::from(1);
        }

        let result =
            ai_batch::apply_ai_batch_updates(&mut project, &translations, options.status.clone());
        if !result.missing_entries.is_empty()
            || !result.duplicate_entries.is_empty()
            || !result.invalid_entries.is_empty()
        {
            eprintln!(
                "Batch {} no pudo aplicarse limpio: missing={}, duplicates={}, invalid={}",
                batch.index,
                result.missing_entries.len(),
                result.duplicate_entries.len(),
                result.invalid_entries.len()
            );
            return ExitCode::from(1);
        }

        applied_entries += result.applied_entries;
        skipped_entries += result.skipped_entries;
        validation_warnings.extend(result.warnings);
    }

    if let Err(error) = persistence::save_project(&project, &project_path) {
        eprintln!(
            "No se pudo guardar proyecto '{}': {}",
            project_path.display(),
            error
        );
        return ExitCode::from(1);
    }

    let export_dir =
        PathBuf::from(&manifest.exports_dir).join(Local::now().format("%Y%m%d-%H%M%S").to_string());
    let export_result = match exporter::export_project(&project, &export_dir) {
        Ok(result) => result,
        Err(error) => {
            eprintln!(
                "No se pudo exportar a '{}': {}",
                export_dir.display(),
                error
            );
            return ExitCode::from(1);
        }
    };
    validation_warnings.extend(export_result.warnings.clone());

    let reopened = match parse_game(&export_dir) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!(
                "No se pudo reabrir export '{}': {}",
                export_dir.display(),
                error
            );
            return ExitCode::from(1);
        }
    };

    if format!("{:?}", project.engine) != format!("{:?}", reopened.engine) {
        eprintln!(
            "Motor reabierto no coincide: esperado {:?}, recibido {:?}",
            project.engine, reopened.engine
        );
        return ExitCode::from(1);
    }

    if inventory_signature(&reopened.files) != expected_signature
        || total_entries(&reopened.files) != expected_entries
    {
        eprintln!(
            "Firma del export reabierto no coincide con el proyecto preparado '{}'",
            manifest.sample_name
        );
        return ExitCode::from(1);
    }

    let report = ApplyReport {
        sample_name: manifest.sample_name.clone(),
        manifest_path: options.manifest.display().to_string(),
        applied_entries,
        skipped_entries,
        batches_applied: manifest.batches.len(),
        validation_warnings,
        export_dir: export_dir.display().to_string(),
        export_files: export_result.files_exported,
        export_entries_applied: export_result.entries_applied,
        reopened_engine: format!("{:?}", reopened.engine),
        reopened_files: reopened.files.len(),
        reopened_entries: total_entries(&reopened.files),
    };
    let report_path = PathBuf::from(&manifest.batch_dir).join(format!(
        "apply-report-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    ));

    if let Err(error) = fs::write(
        &report_path,
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string()),
    ) {
        eprintln!(
            "No se pudo escribir apply report '{}': {}",
            report_path.display(),
            error
        );
        return ExitCode::from(1);
    }

    println!(
        "[OK] {} => applied={} skipped={} warnings={} exportFiles={} exportEntries={} reopenedEntries={}",
        manifest.sample_name,
        applied_entries,
        skipped_entries,
        report.validation_warnings.len(),
        export_result.files_exported,
        export_result.entries_applied,
        report.reopened_entries
    );
    println!("Report: {}", report_path.display());
    println!("Export: {}", export_dir.display());

    ExitCode::SUCCESS
}

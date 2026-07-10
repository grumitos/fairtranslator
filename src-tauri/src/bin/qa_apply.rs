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

mod qa_inventory_fingerprint;

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
    #[serde(default)]
    inventory_fingerprint: Option<String>,
    #[serde(default)]
    source_lang: Option<String>,
    #[serde(default)]
    target_lang: Option<String>,
    #[serde(default)]
    batch_count: Option<usize>,
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
        _ => Err(format!("Estado inválido: {value}")),
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
            _ if arg.starts_with("--") => return Err(format!("Opción desconocida: {arg}")),
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

fn validate_manifest_contract(
    manifest: &SampleManifest,
    project_source_lang: &str,
    project_target_lang: &str,
) -> Result<(), String> {
    let source_lang = manifest.source_lang.as_deref().ok_or_else(|| {
        "El manifest no incluye sourceLang. Regenera la muestra con qa_prepare.".to_string()
    })?;
    let target_lang = manifest.target_lang.as_deref().ok_or_else(|| {
        "El manifest no incluye targetLang. Regenera la muestra con qa_prepare.".to_string()
    })?;
    if source_lang != project_source_lang || target_lang != project_target_lang {
        return Err(format!(
            "Los idiomas del manifest ({source_lang}->{target_lang}) no coinciden con el proyecto ({project_source_lang}->{project_target_lang}). Regenera la muestra con qa_prepare."
        ));
    }

    let batch_count = manifest.batch_count.ok_or_else(|| {
        "El manifest no incluye batchCount. Regenera la muestra con qa_prepare.".to_string()
    })?;
    if batch_count != manifest.batches.len() {
        return Err(format!(
            "batchCount declara {batch_count} lotes, pero el manifest contiene {}. Regenera la muestra con qa_prepare.",
            manifest.batches.len()
        ));
    }

    let mut indices = HashSet::new();
    let mut batch_ids = HashSet::new();
    let mut entries_paths = HashSet::new();
    let mut response_paths = HashSet::new();
    for batch in &manifest.batches {
        if !indices.insert(batch.index)
            || !batch_ids.insert(batch.batch_id.as_str())
            || !entries_paths.insert(batch.entries_path.as_str())
            || !response_paths.insert(batch.response_template_path.as_str())
        {
            return Err(format!(
                "El manifest contiene metadata duplicada en el lote {}. Regenera la muestra con qa_prepare.",
                batch.index
            ));
        }
    }
    if !(1..=batch_count).all(|index| indices.contains(&index)) {
        return Err(
            "Los índices de los lotes no son consecutivos desde 1. Regenera la muestra con qa_prepare."
                .to_string(),
        );
    }

    Ok(())
}

fn load_all_batch_entries(manifest: &SampleManifest) -> Result<Vec<Vec<AiBatchEntry>>, String> {
    manifest
        .batches
        .iter()
        .map(|batch| {
            let entries_path = PathBuf::from(&batch.entries_path);
            fs::read_to_string(&entries_path)
                .map_err(|error| {
                    format!(
                        "No se pudieron leer entradas esperadas para el lote {} en '{}': {error}",
                        batch.index,
                        entries_path.display()
                    )
                })
                .and_then(|content| {
                    serde_json::from_str(&content).map_err(|error| {
                        format!(
                            "Entradas inválidas para el lote {} en '{}': {error}",
                            batch.index,
                            entries_path.display()
                        )
                    })
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_manifest() -> SampleManifest {
        SampleManifest {
            sample_name: "sample".into(),
            inventory_fingerprint: Some("fairtl-inventory-v1-fnv1a64:0000000000000000".into()),
            source_lang: Some("en".into()),
            target_lang: Some("es".into()),
            batch_count: Some(1),
            project_path: "project.fairtl".into(),
            exports_dir: "exports".into(),
            batch_dir: "ai-batches".into(),
            batches: vec![BatchArtifactRow {
                index: 1,
                batch_id: "batch-1".into(),
                entries_path: "batch-1.entries.json".into(),
                response_template_path: "batch-1.response.template.json".into(),
            }],
        }
    }

    #[test]
    fn legacy_manifest_loads_for_an_actionable_fingerprint_rejection() {
        let manifest: SampleManifest = serde_json::from_value(serde_json::json!({
            "sampleName": "sample",
            "projectPath": "project.fairtl",
            "exportsDir": "exports",
            "batchDir": "ai-batches",
            "batches": []
        }))
        .unwrap();

        let error = qa_inventory_fingerprint::verify_project_inventory_fingerprint(
            manifest.inventory_fingerprint.as_deref(),
            &[],
        )
        .unwrap_err();
        assert!(error.contains("qa_prepare"));
    }

    #[test]
    fn manifest_contract_rejects_language_and_batch_drift() {
        let manifest = current_manifest();
        assert!(validate_manifest_contract(&manifest, "en", "es").is_ok());

        let language_error = validate_manifest_contract(&manifest, "en", "fr").unwrap_err();
        assert!(language_error.contains("idiomas"));

        let mut missing_batch = current_manifest();
        missing_batch.batch_count = Some(2);
        assert!(validate_manifest_contract(&missing_batch, "en", "es")
            .unwrap_err()
            .contains("batchCount"));

        let mut duplicate_path = current_manifest();
        duplicate_path.batch_count = Some(2);
        duplicate_path.batches.push(BatchArtifactRow {
            index: 2,
            batch_id: "batch-2".into(),
            entries_path: "batch-1.entries.json".into(),
            response_template_path: "batch-2.response.template.json".into(),
        });
        assert!(validate_manifest_contract(&duplicate_path, "en", "es")
            .unwrap_err()
            .contains("duplicada"));
    }
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

    let expected_fingerprint = match qa_inventory_fingerprint::verify_project_inventory_fingerprint(
        manifest.inventory_fingerprint.as_deref(),
        &project.files,
    ) {
        Ok(fingerprint) => fingerprint,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) =
        validate_manifest_contract(&manifest, &project.source_lang, &project.target_lang)
    {
        eprintln!("{error} No se aplicó ninguna respuesta.");
        return ExitCode::from(1);
    }
    let source = match parse_game(Path::new(&project.game_dir)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!(
                "No se pudo verificar la fuente actual '{}': {}. No se aplicó ninguna respuesta.",
                project.game_dir, error
            );
            return ExitCode::from(1);
        }
    };
    if source.engine != project.engine {
        eprintln!(
            "El motor detectado en la fuente actual ({:?}) no coincide con el proyecto ({:?}). No se aplicó ninguna respuesta.",
            source.engine, project.engine
        );
        return ExitCode::from(1);
    }
    if let Err(error) = qa_inventory_fingerprint::verify_source_inventory_fingerprint(
        expected_fingerprint,
        &source.files,
    ) {
        eprintln!("{error}");
        return ExitCode::from(1);
    }

    let batch_entries = match load_all_batch_entries(&manifest) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("{error}. No se aplicó ninguna respuesta.");
            return ExitCode::from(1);
        }
    };
    if let Err(error) =
        qa_inventory_fingerprint::verify_batch_inventory_coverage(&project.files, &batch_entries)
    {
        eprintln!("{error} No se aplicó ninguna respuesta.");
        return ExitCode::from(1);
    }

    let expected_signature = inventory_signature(&project.files);
    let expected_entries = total_entries(&project.files);
    let mut applied_entries = 0usize;
    let mut skipped_entries = 0usize;
    let mut validation_warnings = Vec::new();

    for (batch, expected_entries_batch) in manifest.batches.iter().zip(batch_entries.iter()) {
        let template_path = PathBuf::from(&batch.response_template_path);
        let response_path = resolve_response_path(&template_path);
        let response_map = expected_entries_batch
            .iter()
            .enumerate()
            .map(|(index, entry)| app_lib::models::AiBatchResponseRef {
                local_id: u16::try_from(index + 1).unwrap_or(u16::MAX),
                file_path: entry.file_path.clone(),
                entry_id: entry.entry_id.clone(),
            })
            .collect::<Vec<_>>();

        let raw_response = match fs::read_to_string(&response_path) {
            Ok(content) => content,
            Err(error) => {
                eprintln!(
                    "No se pudo leer respuesta para el lote {} en '{}': {}",
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
                match ai_batch::parse_ai_batch_response_with_refs(
                    &raw_response,
                    Some(&project.id),
                    Some(&batch.batch_id),
                    Some(&response_map),
                ) {
                    Ok(translations) => translations,
                    Err(compact_error) => {
                        eprintln!(
                            "Respuesta inválida para lote {} ({}): {}",
                            batch.index,
                            response_path.display(),
                            compact_error
                        );
                        return ExitCode::from(1);
                    }
                }
            }
        };

        if let Err(error) =
            ai_batch::validate_ai_batch_response_complete(&response_map, &translations)
        {
            eprintln!(
                "Respuesta incompleta para el lote {} ({}): {}",
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

    if let Err(error) = persistence::save_project(&project, &project_path) {
        eprintln!(
            "No se pudo guardar proyecto '{}': {}",
            project_path.display(),
            error
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
        "[OK] {} => aplicadas={} omitidas={} avisos={} archivosExportados={} entradasExportadas={} entradasReabiertas={}",
        manifest.sample_name,
        applied_entries,
        skipped_entries,
        report.validation_warnings.len(),
        export_result.files_exported,
        export_result.entries_applied,
        report.reopened_entries
    );
    println!("Reporte: {}", report_path.display());
    println!("Exportación: {}", export_dir.display());

    ExitCode::SUCCESS
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use app_lib::models::{AiBatchEntry, AiBatchExport, AiBatchResponseRef, Project, TranslationFile};
use app_lib::parser::{parse_game, resolve_game_dir};
use app_lib::services::workspace_lock::WorkspaceRunLock;
use app_lib::services::{ai_batch, persistence};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::json;

mod qa_inventory_fingerprint;

#[derive(Debug)]
struct Options {
    workspace: PathBuf,
    report: Option<PathBuf>,
    source_lang: String,
    target_lang: String,
    chunk_size: usize,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArtifactRow {
    index: usize,
    batch_id: String,
    entries_fingerprint: String,
    exported_entries: usize,
    instruction_candidates: usize,
    prompt_path: String,
    entries_path: String,
    response_template_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SampleManifest {
    sample_name: String,
    selected_path: String,
    resolved_path: String,
    engine: String,
    files: usize,
    entries: usize,
    inventory_fingerprint: String,
    source_lang: String,
    target_lang: String,
    #[serde(default)]
    chunk_size: Option<usize>,
    stable_import: bool,
    project_path: String,
    exports_dir: String,
    batch_dir: String,
    manifest_path: String,
    instruction_candidates: usize,
    batch_count: usize,
    batches: Vec<BatchArtifactRow>,
}

#[derive(Debug)]
struct ReusableSample {
    manifest: SampleManifest,
    completed_responses: usize,
}

fn print_usage() {
    eprintln!(
        "Uso: cargo run --release --bin qa_prepare -- [opciones] <ruta> [ruta...]\n\
         \n\
         Opciones:\n\
           --workspace <ruta>    Directorio base para artefactos generados\n\
           --report <ruta>       Markdown global a escribir (default: <workspace>\\campaign-report.md)\n\
           --source-lang <code>  Idioma fuente del proyecto (default: en)\n\
           --target-lang <code>  Idioma destino del proyecto (default: es)\n\
           --chunk-size <n>      Tamaño de lote para prompts IA (default: 200)\n\
           --help, -h            Muestra esta ayuda"
    );
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut workspace = PathBuf::from(format!(
        "D:\\Game\\FairTranslatorQA\\campaign\\{}",
        Local::now().format("%Y%m%d-%H%M%S")
    ));
    let mut report = None;
    let mut source_lang = "en".to_string();
    let mut target_lang = "es".to_string();
    let mut chunk_size = ai_batch::DEFAULT_AI_BATCH_MAX_ENTRIES;
    let mut paths = Vec::new();

    let mut args = argv.skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workspace" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --workspace".to_string())?;
                workspace = PathBuf::from(value);
            }
            "--report" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --report".to_string())?;
                report = Some(PathBuf::from(value));
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
            "--chunk-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --chunk-size".to_string())?;
                chunk_size = value
                    .parse::<usize>()
                    .map_err(|_| format!("Valor inválido para --chunk-size: {value}"))?;
                if chunk_size == 0 {
                    return Err("--chunk-size debe ser >= 1".to_string());
                }
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ if arg.starts_with("--") => {
                return Err(format!("Opción desconocida: {arg}"));
            }
            _ => paths.push(arg),
        }
    }

    if paths.is_empty() {
        return Err("Debes indicar al menos una ruta".to_string());
    }

    Ok(Options {
        workspace,
        report,
        source_lang,
        target_lang,
        chunk_size,
        paths,
    })
}

fn is_web_root(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("www"))
        .unwrap_or(false)
}

fn sample_name(selected: &Path, resolved: &Path) -> String {
    let is_bad_name = |name: &str| {
        name.eq_ignore_ascii_case("www")
            || name.eq_ignore_ascii_case("test")
            || name.eq_ignore_ascii_case("game")
            || name.len() > 48
    };

    let mut candidates = Vec::new();
    for path in [selected, resolved] {
        let mut cursor = Some(path);
        for _ in 0..3 {
            let Some(current) = cursor else {
                break;
            };
            if let Some(name) = current.file_name().and_then(|value| value.to_str()) {
                if !name.trim().is_empty() {
                    candidates.push(name.to_string());
                }
            }
            cursor = current.parent();
        }
    }

    if is_web_root(resolved) {
        for candidate in &candidates {
            if !is_bad_name(candidate) {
                return candidate.clone();
            }
        }
    }

    candidates
        .into_iter()
        .find(|candidate| !is_bad_name(candidate))
        .unwrap_or_else(|| "sample".to_string())
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            previous_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if previous_dash {
            None
        } else {
            previous_dash = true;
            Some('-')
        };

        if let Some(ch) = normalized {
            slug.push(ch);
        }
    }

    slug.trim_matches('-').to_string()
}

fn total_entries(files: &[TranslationFile]) -> usize {
    files.iter().map(|file| file.entries.len()).sum()
}

fn build_project(
    name: String,
    source_lang: &str,
    target_lang: &str,
    resolved_path: &Path,
    parsed: app_lib::parser::ParsedGame,
) -> Project {
    let now = persistence::now_iso8601();
    Project {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        source_lang: source_lang.to_string(),
        target_lang: target_lang.to_string(),
        engine: parsed.engine,
        game_dir: resolved_path.display().to_string(),
        files: parsed.files,
        glossary: Vec::new(),
        style_guide: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn response_template_for(batch: &AiBatchExport) -> serde_json::Value {
    serde_json::from_str(&batch.translation_sheet).unwrap_or_else(|_| {
        json!({
            "p": batch.project_id,
            "b": batch.batch_id,
            "translations": batch.response_map.iter().map(|entry| {
                json!({
                    "k": entry.local_id,
                    "t": "",
                    "n": "",
                    "s": false
                })
            }).collect::<Vec<_>>()
        })
    })
}

fn response_path_for(template_path: &Path) -> PathBuf {
    let Some(template_name) = template_path.file_name().and_then(|value| value.to_str()) else {
        return template_path.to_path_buf();
    };
    template_path.with_file_name(template_name.replace(".response.template.json", ".response.json"))
}

fn batch_dir_has_responses(batch_dir: &Path) -> bool {
    fs::read_dir(batch_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.ends_with(".response.json"))
                .unwrap_or(false)
        })
}

fn refuse_rebuild_with_responses(
    batch_dir: &Path,
    reason: impl AsRef<str>,
) -> Result<Option<ReusableSample>, String> {
    if batch_dir_has_responses(batch_dir) {
        Err(format!(
            "La preparación existente no se puede reutilizar ({}) y ya contiene respuestas. No se reconstruirá para no perder chunks completos; usa un workspace nuevo o conserva/corrige el manifest actual.",
            reason.as_ref()
        ))
    } else {
        Ok(None)
    }
}

fn completed_response_count(manifest: &SampleManifest, project: &Project) -> usize {
    manifest
        .batches
        .iter()
        .filter(|batch| {
            let entries_path = PathBuf::from(&batch.entries_path);
            let template_path = PathBuf::from(&batch.response_template_path);
            let response_path = response_path_for(&template_path);
            let Ok(entries_text) = fs::read_to_string(&entries_path) else {
                return false;
            };
            let Ok(entries) = serde_json::from_str::<Vec<AiBatchEntry>>(&entries_text) else {
                return false;
            };
            let response_map = entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    u16::try_from(index + 1)
                        .ok()
                        .map(|local_id| AiBatchResponseRef {
                            local_id,
                            file_path: entry.file_path.clone(),
                            entry_id: entry.entry_id.clone(),
                        })
                })
                .collect::<Vec<_>>();
            if response_map.len() != entries.len() {
                return false;
            }
            let Ok(raw_response) = fs::read_to_string(response_path) else {
                return false;
            };
            let Ok(translations) = ai_batch::parse_ai_batch_response_with_refs(
                &raw_response,
                Some(&project.id),
                Some(&batch.batch_id),
                Some(&response_map),
            ) else {
                return false;
            };
            ai_batch::validate_ai_batch_response_complete(&response_map, &translations).is_ok()
        })
        .count()
}

#[allow(clippy::too_many_arguments)]
fn try_load_reusable_sample(
    manifest_path: &Path,
    batch_dir: &Path,
    resolved: &Path,
    engine: &str,
    files: usize,
    entries: usize,
    inventory_fingerprint: &str,
    options: &Options,
) -> Result<Option<ReusableSample>, String> {
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let manifest_text = match fs::read_to_string(manifest_path) {
        Ok(text) => text,
        Err(error) => {
            return refuse_rebuild_with_responses(
                batch_dir,
                format!("no se pudo leer el manifest: {error}"),
            )
        }
    };
    let manifest = match serde_json::from_str::<SampleManifest>(&manifest_text) {
        Ok(manifest) => manifest,
        Err(error) => {
            return refuse_rebuild_with_responses(
                batch_dir,
                format!("el manifest no es válido: {error}"),
            )
        }
    };

    let metadata_matches = Path::new(&manifest.resolved_path) == resolved
        && manifest.engine == engine
        && manifest.files == files
        && manifest.entries == entries
        && manifest.inventory_fingerprint == inventory_fingerprint
        && manifest.source_lang == options.source_lang
        && manifest.target_lang == options.target_lang
        && manifest
            .chunk_size
            .map(|chunk_size| chunk_size == options.chunk_size)
            .unwrap_or(true)
        && Path::new(&manifest.batch_dir) == batch_dir
        && manifest.batch_count == manifest.batches.len();
    if !metadata_matches {
        return refuse_rebuild_with_responses(batch_dir, "cambió la fuente o la configuración");
    }

    let project = match persistence::load_project(Path::new(&manifest.project_path)) {
        Ok(project) => project,
        Err(error) => {
            return refuse_rebuild_with_responses(
                batch_dir,
                format!("no se pudo abrir el proyecto: {error}"),
            )
        }
    };
    if project.source_lang != options.source_lang
        || project.target_lang != options.target_lang
        || qa_inventory_fingerprint::inventory_fingerprint(&project.files) != inventory_fingerprint
    {
        return refuse_rebuild_with_responses(batch_dir, "el proyecto no coincide con la fuente");
    }

    for (offset, batch) in manifest.batches.iter().enumerate() {
        let prompt_path = PathBuf::from(&batch.prompt_path);
        let entries_path = PathBuf::from(&batch.entries_path);
        let template_path = PathBuf::from(&batch.response_template_path);
        if batch.index != offset + 1
            || !prompt_path.is_file()
            || !entries_path.is_file()
            || !template_path.is_file()
        {
            return refuse_rebuild_with_responses(batch_dir, "faltan artefactos de un chunk");
        }

        let entries_text = match fs::read_to_string(&entries_path) {
            Ok(text) => text,
            Err(error) => {
                return refuse_rebuild_with_responses(
                    batch_dir,
                    format!("no se pudo leer un chunk: {error}"),
                )
            }
        };
        let parsed_entries = match serde_json::from_str::<Vec<AiBatchEntry>>(&entries_text) {
            Ok(entries) => entries,
            Err(error) => {
                return refuse_rebuild_with_responses(
                    batch_dir,
                    format!("un chunk no es válido: {error}"),
                )
            }
        };
        let fingerprint =
            qa_inventory_fingerprint::batch_entries_fingerprint(&batch.batch_id, &parsed_entries);
        if fingerprint != batch.entries_fingerprint
            || parsed_entries.len() != batch.exported_entries
        {
            return refuse_rebuild_with_responses(batch_dir, "cambió el contenido de un chunk");
        }

        let template_text = match fs::read_to_string(&template_path) {
            Ok(text) => text,
            Err(error) => {
                return refuse_rebuild_with_responses(
                    batch_dir,
                    format!("no se pudo leer un template: {error}"),
                )
            }
        };
        let template = match serde_json::from_str::<serde_json::Value>(&template_text) {
            Ok(template) => template,
            Err(error) => {
                return refuse_rebuild_with_responses(
                    batch_dir,
                    format!("un template no es válido: {error}"),
                )
            }
        };
        if template.get("p").and_then(serde_json::Value::as_str) != Some(project.id.as_str())
            || template.get("b").and_then(serde_json::Value::as_str)
                != Some(batch.batch_id.as_str())
        {
            return refuse_rebuild_with_responses(
                batch_dir,
                "el template no coincide con el chunk",
            );
        }
    }

    let completed_responses = completed_response_count(&manifest, &project);
    Ok(Some(ReusableSample {
        manifest,
        completed_responses,
    }))
}

fn prepare_sample(
    selected: &str,
    options: &Options,
    sample_index: usize,
) -> Result<SampleManifest, String> {
    let selected_path = PathBuf::from(selected);
    let resolved = resolve_game_dir(&selected_path).map_err(|error| error.to_string())?;
    let parsed_a = parse_game(&resolved).map_err(|error| error.to_string())?;
    let files = parsed_a.files.len();
    let entries = total_entries(&parsed_a.files);
    let inventory_fingerprint = qa_inventory_fingerprint::inventory_fingerprint(&parsed_a.files);
    let sample_name = sample_name(&selected_path, &resolved);
    let slug_base = slugify(&format!("{:?}-{}", parsed_a.engine, sample_name));
    let slug = if slug_base.is_empty() {
        format!("sample-{:02}", sample_index + 1)
    } else {
        format!("{:02}-{}", sample_index + 1, slug_base)
    };
    let sample_dir = options.workspace.join(&slug);
    let project_dir = sample_dir.join("project.fairtl");
    let batch_dir = sample_dir.join("ai-batches");
    let exports_dir = sample_dir.join("exports");
    let manifest_path = sample_dir.join("manifest.json");

    fs::create_dir_all(&batch_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&exports_dir).map_err(|error| error.to_string())?;

    if let Some(reusable) = try_load_reusable_sample(
        &manifest_path,
        &batch_dir,
        &resolved,
        &format!("{:?}", parsed_a.engine),
        files,
        entries,
        &inventory_fingerprint,
        options,
    )? {
        eprintln!(
            "[RESUME] {} => preparación reutilizada; respuestas completas={}/{}",
            reusable.manifest.sample_name,
            reusable.completed_responses,
            reusable.manifest.batch_count
        );
        return Ok(reusable.manifest);
    }

    let parsed_b = parse_game(&resolved).map_err(|error| error.to_string())?;
    if parsed_a.engine != parsed_b.engine {
        return Err(format!(
            "Import inestable: engine distinto entre corridas para '{}'",
            selected
        ));
    }
    if inventory_fingerprint != qa_inventory_fingerprint::inventory_fingerprint(&parsed_b.files) {
        return Err(format!(
            "Import inestable: firma de archivos distinta entre corridas para '{}'",
            selected
        ));
    }

    let project = build_project(
        sample_name.clone(),
        &options.source_lang,
        &options.target_lang,
        &resolved,
        parsed_a,
    );
    persistence::save_project(&project, &project_dir).map_err(|error| error.to_string())?;

    let batches = ai_batch::export_ai_batches(&project, &[], Some(options.chunk_size))
        .map_err(|error| error.to_string())?;
    let mut batch_rows = Vec::new();
    let mut instruction_candidates = 0usize;

    for (batch_index, batch) in batches.iter().enumerate() {
        let file_stub = format!("batch-{:04}", batch_index + 1);
        let prompt_path = batch_dir.join(format!("{file_stub}.prompt.txt"));
        let entries_path = batch_dir.join(format!("{file_stub}.entries.json"));
        let response_template_path = batch_dir.join(format!("{file_stub}.response.template.json"));
        let candidate_count = batch
            .entries
            .iter()
            .filter(|entry| entry.instruction_candidate)
            .count();

        instruction_candidates += candidate_count;

        fs::write(&prompt_path, &batch.prompt).map_err(|error| error.to_string())?;
        fs::write(
            &entries_path,
            serde_json::to_string_pretty(&batch.entries).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            &response_template_path,
            serde_json::to_string_pretty(&response_template_for(batch))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        batch_rows.push(BatchArtifactRow {
            index: batch_index + 1,
            batch_id: batch.batch_id.clone(),
            entries_fingerprint: qa_inventory_fingerprint::batch_entries_fingerprint(
                &batch.batch_id,
                &batch.entries,
            ),
            exported_entries: batch.exported_entries,
            instruction_candidates: candidate_count,
            prompt_path: prompt_path.display().to_string(),
            entries_path: entries_path.display().to_string(),
            response_template_path: response_template_path.display().to_string(),
        });
    }

    let manifest = SampleManifest {
        sample_name,
        selected_path: selected.to_string(),
        resolved_path: resolved.display().to_string(),
        engine: format!("{:?}", project.engine),
        files,
        entries,
        inventory_fingerprint,
        source_lang: options.source_lang.clone(),
        target_lang: options.target_lang.clone(),
        chunk_size: Some(options.chunk_size),
        stable_import: true,
        project_path: project_dir.display().to_string(),
        exports_dir: exports_dir.display().to_string(),
        batch_dir: batch_dir.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        instruction_candidates,
        batch_count: batch_rows.len(),
        batches: batch_rows,
    };

    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    Ok(manifest)
}

fn build_report(workspace: &Path, manifests: &[SampleManifest], options: &Options) -> String {
    let mut markdown = String::new();
    markdown.push_str("# QA Campaign Preparation\n\n");
    markdown.push_str(&format!(
        "- Generated: `{}`\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    markdown.push_str(&format!("- Workspace: `{}`\n", workspace.display()));
    markdown.push_str(&format!("- Source language: `{}`\n", options.source_lang));
    markdown.push_str(&format!("- Target language: `{}`\n", options.target_lang));
    markdown.push_str(&format!("- Chunk size: `{}`\n", options.chunk_size));
    markdown.push_str(&format!("- Samples prepared: `{}`\n\n", manifests.len()));

    for manifest in manifests {
        markdown.push_str(&format!("## {}\n\n", manifest.sample_name));
        markdown.push_str(&format!("- Selected path: `{}`\n", manifest.selected_path));
        markdown.push_str(&format!("- Resolved path: `{}`\n", manifest.resolved_path));
        markdown.push_str(&format!("- Engine: `{}`\n", manifest.engine));
        markdown.push_str(&format!(
            "- Stable import: `PASS` (`files={}`, `entries={}`)\n",
            manifest.files, manifest.entries
        ));
        markdown.push_str(&format!("- Project: `{}`\n", manifest.project_path));
        markdown.push_str(&format!("- Exports dir: `{}`\n", manifest.exports_dir));
        markdown.push_str(&format!(
            "- Directorio de lotes IA: `{}`\n",
            manifest.batch_dir
        ));
        markdown.push_str(&format!("- Manifest: `{}`\n", manifest.manifest_path));
        markdown.push_str(&format!(
            "- Lotes IA: `{}` (`instructionCandidates={}`)\n\n",
            manifest.batch_count, manifest.instruction_candidates
        ));
    }

    markdown
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

    if let Err(error) = fs::create_dir_all(&options.workspace) {
        eprintln!(
            "No se pudo crear workspace '{}': {}",
            options.workspace.display(),
            error
        );
        return ExitCode::from(1);
    }
    let _workspace_lock = match WorkspaceRunLock::acquire(&options.workspace) {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    let mut manifests = Vec::new();

    for (index, selected) in options.paths.iter().enumerate() {
        match prepare_sample(selected, &options, index) {
            Ok(manifest) => {
                println!(
                    "[OK] {} => motor={} archivos={} entradas={} lotes={}",
                    manifest.sample_name,
                    manifest.engine,
                    manifest.files,
                    manifest.entries,
                    manifest.batch_count
                );
                manifests.push(manifest);
            }
            Err(error) => {
                eprintln!("[ERROR] {} => {}", selected, error);
                return ExitCode::from(1);
            }
        }
    }

    let report_path = options
        .report
        .clone()
        .unwrap_or_else(|| options.workspace.join("campaign-report.md"));
    let report_content = build_report(&options.workspace, &manifests, &options);

    if let Err(error) = fs::write(&report_path, report_content) {
        eprintln!(
            "No se pudo escribir reporte '{}': {}",
            report_path.display(),
            error
        );
        return ExitCode::from(1);
    }

    println!("Reporte: {}", report_path.display());
    println!("Espacio de trabajo: {}", options.workspace.display());

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_lib::models::{Engine, EntryStatus, TranslationEntry};

    fn reusable_fixture() -> (PathBuf, PathBuf, PathBuf, Options, String) {
        let root = std::env::temp_dir().join(format!(
            "fairtranslator-qa-prepare-resume-test-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let batch_dir = root.join("ai-batches");
        let project_path = root.join("project.fairtl");
        let manifest_path = root.join("manifest.json");
        fs::create_dir_all(&batch_dir).unwrap();

        let project = Project {
            id: "stable-project".into(),
            name: "sample".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: root.join("game").display().to_string(),
            files: vec![TranslationFile {
                relative_path: "data/CommonEvents.json".into(),
                entries: vec![TranslationEntry {
                    id: "CommonEvents:1:text:0".into(),
                    file_context: "text".into(),
                    original: "Hello".into(),
                    translation: String::new(),
                    status: EntryStatus::Untranslated,
                    notes: String::new(),
                }],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        persistence::save_project(&project, &project_path).unwrap();
        let inventory_fingerprint = qa_inventory_fingerprint::inventory_fingerprint(&project.files);

        let entries = vec![AiBatchEntry {
            file_path: "data/CommonEvents.json".into(),
            entry_id: "CommonEvents:1:text:0".into(),
            file_context: "text".into(),
            original: "Hello".into(),
            current_translation: String::new(),
            instruction_candidate: false,
            instruction_reason: None,
        }];
        let prompt_path = batch_dir.join("batch-0001.prompt.txt");
        let entries_path = batch_dir.join("batch-0001.entries.json");
        let template_path = batch_dir.join("batch-0001.response.template.json");
        let response_path = batch_dir.join("batch-0001.response.json");
        fs::write(&prompt_path, "prompt").unwrap();
        fs::write(&entries_path, serde_json::to_string(&entries).unwrap()).unwrap();
        fs::write(
            &template_path,
            serde_json::json!({"p": project.id, "b": "stable-batch", "translations": []})
                .to_string(),
        )
        .unwrap();
        fs::write(
            &response_path,
            serde_json::json!({
                "p": "stable-project",
                "b": "stable-batch",
                "translations": [{"k": 1, "t": "Hola", "n": "", "s": false}]
            })
            .to_string(),
        )
        .unwrap();

        let manifest = SampleManifest {
            sample_name: "sample".into(),
            selected_path: root.join("game").display().to_string(),
            resolved_path: root.join("game").display().to_string(),
            engine: "Mv".into(),
            files: 1,
            entries: 1,
            inventory_fingerprint: inventory_fingerprint.clone(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            chunk_size: Some(200),
            stable_import: true,
            project_path: project_path.display().to_string(),
            exports_dir: root.join("exports").display().to_string(),
            batch_dir: batch_dir.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            instruction_candidates: 0,
            batch_count: 1,
            batches: vec![BatchArtifactRow {
                index: 1,
                batch_id: "stable-batch".into(),
                entries_fingerprint: qa_inventory_fingerprint::batch_entries_fingerprint(
                    "stable-batch",
                    &entries,
                ),
                exported_entries: 1,
                instruction_candidates: 0,
                prompt_path: prompt_path.display().to_string(),
                entries_path: entries_path.display().to_string(),
                response_template_path: template_path.display().to_string(),
            }],
        };
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let options = Options {
            workspace: root.clone(),
            report: None,
            source_lang: "ja".into(),
            target_lang: "es".into(),
            chunk_size: 200,
            paths: vec![root.join("game").display().to_string()],
        };
        (
            root,
            batch_dir,
            manifest_path,
            options,
            inventory_fingerprint,
        )
    }

    #[test]
    fn valid_preparation_reuses_completed_responses() {
        let (root, batch_dir, manifest_path, options, fingerprint) = reusable_fixture();
        let resolved = root.join("game");

        let reusable = try_load_reusable_sample(
            &manifest_path,
            &batch_dir,
            &resolved,
            "Mv",
            1,
            1,
            &fingerprint,
            &options,
        )
        .unwrap()
        .unwrap();

        assert_eq!(reusable.completed_responses, 1);
        assert_eq!(reusable.manifest.batches[0].batch_id, "stable-batch");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preparation_refuses_to_invalidate_existing_responses() {
        let (root, batch_dir, manifest_path, options, _fingerprint) = reusable_fixture();
        let resolved = root.join("game");

        let error = try_load_reusable_sample(
            &manifest_path,
            &batch_dir,
            &resolved,
            "Mv",
            1,
            1,
            "different-inventory",
            &options,
        )
        .unwrap_err();

        assert!(error.contains("no perder chunks completos"));
        let _ = fs::remove_dir_all(root);
    }
}

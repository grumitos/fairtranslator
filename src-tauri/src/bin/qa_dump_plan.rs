use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use app_lib::models::ProviderType;
use app_lib::services::persistence;
use app_lib::services::translation_orchestrator;
use chrono::Utc;
use serde::Serialize;

#[derive(Debug)]
struct Options {
    project_path: PathBuf,
    output_path: PathBuf,
    provider: ProviderType,
    full: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanReport {
    project_path: String,
    provider: String,
    generated_at: String,
    files_total: usize,
    chunks_total: usize,
    entries_total: usize,
    files: Vec<FilePlan>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FilePlan {
    relative_path: String,
    chunks_total: usize,
    entries_total: usize,
    chunks_shown: usize,
    chunks_omitted: usize,
    chunks: Vec<ChunkPlan>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChunkPlan {
    chunk_index: usize,
    total_chunks: usize,
    entry_count: usize,
    approximate_char_count: usize,
    style_guide_rules: usize,
    term_hint_count: usize,
    translation_example_count: usize,
    file_context_hints: Vec<String>,
    term_hint_preview: Vec<String>,
    example_preview: Vec<String>,
    entry_ids: Vec<String>,
    entry_preview: Vec<String>,
}

fn print_usage() {
    eprintln!(
        "Uso: cargo run --bin qa_dump_plan -- --project <ruta .fairtl> [opciones]\n\
         \n\
         Opciones:\n\
           --output <ruta>          Archivo JSON de salida (default: <project>/qa_translate.plan.json)\n\
           --provider <nombre>      codex|openai|opencode|deepl|google|external-ai (default: codex)\n\
           --full                   Incluye todos los chunks y entryIds completos\n\
           --help, -h               Muestra esta ayuda"
    );
}

fn parse_provider(value: &str) -> Result<ProviderType, String> {
    match value.to_ascii_lowercase().as_str() {
        "openai" => Ok(ProviderType::Openai),
        "opencode" | "open-code" => Ok(ProviderType::Opencode),
        "codex" => Ok(ProviderType::Codex),
        "deepl" => Ok(ProviderType::Deepl),
        "google" => Ok(ProviderType::Google),
        "external-ai" | "external_ai" | "external" => Ok(ProviderType::Codex),
        _ => Err(format!("Proveedor desconocido: {value}")),
    }
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut project_path = None;
    let mut output_path = None;
    let mut provider = ProviderType::Codex;
    let mut full = false;

    let mut args = argv.skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --project".to_string())?;
                project_path = Some(PathBuf::from(value));
            }
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --output".to_string())?;
                output_path = Some(PathBuf::from(value));
            }
            "--provider" => {
                let value = args
                    .next()
                    .ok_or_else(|| "Falta valor para --provider".to_string())?;
                provider = parse_provider(&value)?;
            }
            "--full" => {
                full = true;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("Argumento desconocido: {arg}")),
        }
    }

    let project_path = project_path.ok_or_else(|| "Debes indicar --project".to_string())?;
    let output_path = output_path.unwrap_or_else(|| project_path.join("qa_translate.plan.json"));

    Ok(Options {
        project_path,
        output_path,
        provider,
        full,
    })
}

fn provider_label(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::Openai => "openai",
        ProviderType::Opencode => "opencode",
        ProviderType::Codex => "codex",
        ProviderType::Deepl => "deepl",
        ProviderType::Google => "google",
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

    let project = match persistence::load_project(&options.project_path) {
        Ok(project) => project,
        Err(error) => {
            eprintln!(
                "No se pudo cargar proyecto '{}': {}",
                options.project_path.display(),
                error
            );
            return ExitCode::from(1);
        }
    };

    let plan = translation_orchestrator::build_translation_plan(&project, &[], &options.provider);
    let mut file_map = std::collections::BTreeMap::<String, Vec<ChunkPlan>>::new();

    for chunk in &plan {
        file_map
            .entry(chunk.relative_path.clone())
            .or_default()
            .push(ChunkPlan {
                chunk_index: chunk.context.chunk_index,
                total_chunks: chunk.context.total_chunks,
                entry_count: chunk.entries.len(),
                approximate_char_count: chunk.context.approximate_char_count,
                style_guide_rules: chunk.context.style_guide.len(),
                term_hint_count: chunk.context.term_hints.len(),
                translation_example_count: chunk.context.translation_examples.len(),
                file_context_hints: chunk.context.file_context_hints.clone(),
                term_hint_preview: chunk
                    .context
                    .term_hints
                    .iter()
                    .take(6)
                    .map(|hint| hint.source.clone())
                    .collect(),
                example_preview: chunk
                    .context
                    .translation_examples
                    .iter()
                    .take(4)
                    .map(|example| format!("{} => {}", example.original, example.translation))
                    .collect(),
                entry_ids: chunk
                    .entries
                    .iter()
                    .map(|entry| entry.entry_id.clone())
                    .collect(),
                entry_preview: chunk
                    .entries
                    .iter()
                    .take(6)
                    .map(|entry| entry.text.chars().take(120).collect())
                    .collect(),
            });
    }

    let files = file_map
        .into_iter()
        .map(|(relative_path, mut chunks)| {
            let chunks_total = chunks.len();
            let entries_total = chunks.iter().map(|chunk| chunk.entry_count).sum();

            if !options.full {
                for chunk in &mut chunks {
                    chunk.entry_ids.truncate(4);
                    chunk.entry_preview.truncate(2);
                    chunk.term_hint_preview.truncate(4);
                    chunk.example_preview.truncate(2);
                    chunk.file_context_hints.truncate(3);
                }
            }

            let displayed_chunks = if options.full {
                chunks
            } else {
                chunks.into_iter().take(3).collect()
            };
            let chunks_shown = displayed_chunks.len();
            let chunks_omitted = chunks_total.saturating_sub(chunks_shown);

            FilePlan {
                entries_total,
                chunks_total,
                chunks_shown,
                chunks_omitted,
                relative_path,
                chunks: displayed_chunks,
            }
        })
        .collect::<Vec<_>>();

    let report = PlanReport {
        project_path: options.project_path.display().to_string(),
        provider: provider_label(&options.provider).to_string(),
        generated_at: Utc::now().to_rfc3339(),
        files_total: files.len(),
        chunks_total: plan.len(),
        entries_total: plan.iter().map(|chunk| chunk.entries.len()).sum(),
        files,
    };

    let json = match serde_json::to_string_pretty(&report) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("No se pudo serializar plan: {error}");
            return ExitCode::from(1);
        }
    };

    if let Err(error) = std::fs::write(&options.output_path, json) {
        eprintln!(
            "No se pudo escribir plan '{}': {}",
            options.output_path.display(),
            error
        );
        return ExitCode::from(1);
    }

    println!("{}", options.output_path.display());
    ExitCode::SUCCESS
}

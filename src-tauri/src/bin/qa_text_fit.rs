//! Text-fit QA harness: pseudo-localizes every entry of a game (keeping
//! control codes and line counts), exports it, and measures the layout of the
//! source and of the export with the independent `text_fit` audit. It also
//! checks that every pseudo translation is present in the re-parsed export.
//!
//! Reads the game folder and writes only inside `--workspace`. With
//! `--inspect-only` it prints the read-only `game_inspect` report instead.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use app_lib::models::{EntryStatus, Project};
use app_lib::parser::{inspect_game, parse_game, resolve_game_dir};
use app_lib::services::translation_integrity::collect_special_token_spans;
use app_lib::services::{exporter, text_fit};
use serde::Serialize;

const PSEUDO_WORDS: &[&str] = &[
    "mañana",
    "también",
    "corazón",
    "después",
    "aquí",
    "está",
    "camino",
    "nosotros",
    "llegar",
    "pequeño",
    "acción",
    "búsqueda",
    "dragón",
    "espíritu",
    "último",
    "jardín",
    "montaña",
    "río",
    "canción",
    "héroe",
    "océano",
    "música",
    "señor",
    "niña",
    "árbol",
    "ciudad",
    "fuerza",
    "luz",
    "sombra",
    "tiempo",
    "viaje",
    "día",
    "él",
    "de",
    "la",
    "que",
    "por",
    "una",
];

struct Options {
    game_dir: PathBuf,
    workspace: PathBuf,
    expand: f32,
    /// Relative path whose expected/exported entries are dumped for diagnosis.
    dump_file: Option<String>,
    inspect_only: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    game_dir: String,
    engine: String,
    files: usize,
    entries: usize,
    pseudo_translated: usize,
    expand: f32,
    calibration: Vec<String>,
    wrap_plugin: Option<String>,
    source_layout: text_fit::TextLayoutAudit,
    export_layout: Option<text_fit::TextLayoutAudit>,
    export_result: Option<serde_json::Value>,
    export_error: Option<String>,
    roundtrip_checked: usize,
    roundtrip_missing: usize,
    roundtrip_missing_examples: Vec<String>,
    elapsed_ms: u128,
}

fn parse_args() -> Result<Options, String> {
    let mut game_dir = None;
    let mut workspace = None;
    let mut expand = 1.3f32;
    let mut dump_file = None;
    let mut inspect_only = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => game_dir = args.next().map(PathBuf::from),
            "--workspace" => workspace = args.next().map(PathBuf::from),
            "--expand" => {
                expand = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("--expand needs a number")?;
            }
            "--dump-file" => dump_file = args.next(),
            "--inspect-only" => inspect_only = true,
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    let workspace = match workspace {
        Some(workspace) => workspace,
        None if inspect_only => PathBuf::new(),
        None => return Err("--workspace is required".into()),
    };
    Ok(Options {
        game_dir: game_dir.ok_or("--game-dir is required")?,
        workspace,
        expand,
        dump_file,
        inspect_only,
    })
}

/// Deterministic pseudo text of about `target` visible characters.
fn pseudo_words(target: usize, seed: &mut u64) -> String {
    let mut text = String::new();
    while text.chars().count() < target {
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let word = PSEUDO_WORDS[(*seed >> 33) as usize % PSEUDO_WORDS.len()];
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word);
    }
    let mut truncated = text.chars().take(target.max(1)).collect::<String>();
    while truncated.ends_with(' ') {
        truncated.pop();
    }
    truncated
}

fn pseudo_segment(segment: &str, expand: f32, seed: &mut u64) -> String {
    if !segment.chars().any(char::is_alphabetic) {
        return segment.to_string();
    }
    // Keep every non-letter prefix/suffix (spaces, punctuation, digits) so the
    // text next to a control code keeps its original boundary.
    let leading = segment
        .char_indices()
        .find(|(_, ch)| ch.is_alphabetic())
        .map_or(segment.len(), |(index, _)| index);
    let trailing_start = segment
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_alphabetic())
        .map_or(leading, |(index, ch)| index + ch.len_utf8());
    let core = &segment[leading..trailing_start];
    let units: usize = core
        .chars()
        .map(|ch| if ch.len_utf8() >= 3 { 2 } else { 1 })
        .sum();
    let target = ((units as f32) * expand).round().max(1.0) as usize;
    format!(
        "{}{}{}",
        &segment[..leading],
        pseudo_words(target, seed),
        &segment[trailing_start..]
    )
}

fn pseudo_translate(original: &str, expand: f32, seed: &mut u64) -> String {
    original
        .split('\n')
        .map(|line| {
            let mut output = String::new();
            let mut cursor = 0usize;
            for span in collect_special_token_spans(line) {
                output.push_str(&pseudo_segment(&line[cursor..span.start], expand, seed));
                output.push_str(&line[span.start..span.end]);
                cursor = span.end;
            }
            output.push_str(&pseudo_segment(&line[cursor..], expand, seed));
            output
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Words of a text with control codes removed: fitting may move line breaks,
/// repeat a speaker label or restore a color, but it must never lose a word.
fn add_words(text: &str, counts: &mut HashMap<String, i64>, delta: i64) {
    let mut plain = String::new();
    let mut cursor = 0usize;
    for span in collect_special_token_spans(text) {
        plain.push_str(&text[cursor..span.start]);
        plain.push(' ');
        cursor = span.end;
    }
    plain.push_str(&text[cursor..]);
    for word in plain.split_whitespace() {
        *counts.entry(word.to_string()).or_insert(0) += delta;
    }
}

fn build_project(game_dir: &Path) -> Result<Project, String> {
    let resolved = resolve_game_dir(game_dir).map_err(|error| error.to_string())?;
    let parsed = parse_game(&resolved).map_err(|error| error.to_string())?;
    Ok(Project {
        id: "qa-text-fit".into(),
        name: "qa-text-fit".into(),
        source_lang: "en".into(),
        target_lang: "es".into(),
        engine: parsed.engine,
        game_dir: resolved.to_string_lossy().into_owned(),
        files: parsed.files,
        glossary: Vec::new(),
        style_guide: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
    })
}

fn run(options: &Options) -> Result<Report, String> {
    let started = Instant::now();
    let mut project = build_project(&options.game_dir)?;
    let game_root = PathBuf::from(&project.game_dir);
    // Judge source and export by the same windows: defaults, game settings,
    // then widened to what the original text already uses.
    let mut context = text_fit::TextFitContext::detect(&project.engine, &game_root);
    let calibration = context
        .calibrate_from_game(&game_root)
        .map_err(|error| error.to_string())?;
    let source_layout =
        text_fit::audit_game_layout(&context, &game_root).map_err(|error| error.to_string())?;

    let mut seed = 0x5eed_u64;
    let mut pseudo_translated = 0usize;
    let mut report_entries = 0usize;
    let mut expected: HashMap<String, HashMap<String, i64>> = HashMap::new();
    for file in &mut project.files {
        for entry in &mut file.entries {
            if entry.original.trim().is_empty() {
                continue;
            }
            entry.translation = pseudo_translate(&entry.original, options.expand, &mut seed);
            entry.status = EntryStatus::Draft;
            pseudo_translated += 1;
            add_words(
                &entry.translation,
                expected.entry(file.relative_path.clone()).or_default(),
                1,
            );
            report_entries += 1;
        }
    }

    std::fs::create_dir_all(&options.workspace).map_err(|error| error.to_string())?;
    let export_dir = options.workspace.join("export");
    if export_dir.exists() {
        std::fs::remove_dir_all(&export_dir).map_err(|error| error.to_string())?;
    }

    let mut report = Report {
        game_dir: project.game_dir.clone(),
        engine: format!("{:?}", project.engine),
        files: project.files.len(),
        entries: project.files.iter().map(|file| file.entries.len()).sum(),
        pseudo_translated,
        expand: options.expand,
        calibration,
        wrap_plugin: context.wrap_plugin.clone(),
        source_layout,
        export_layout: None,
        export_result: None,
        export_error: None,
        roundtrip_checked: 0,
        roundtrip_missing: 0,
        roundtrip_missing_examples: Vec::new(),
        elapsed_ms: 0,
    };

    match exporter::export_project(&project, &export_dir) {
        Ok(result) => {
            report.export_result = serde_json::to_value(&result).ok();
            report.export_layout = Some(
                text_fit::audit_game_layout(&context, &export_dir)
                    .map_err(|error| error.to_string())?,
            );
            let reparsed = parse_game(&export_dir).map_err(|error| error.to_string())?;
            report.roundtrip_checked = report_entries;
            if let Some(dump_file) = &options.dump_file {
                let entries_of = |files: &[app_lib::models::TranslationFile], translated: bool| {
                    files
                        .iter()
                        .filter(|file| &file.relative_path == dump_file)
                        .flat_map(|file| file.entries.iter())
                        .map(|entry| {
                            let text = if translated {
                                &entry.translation
                            } else {
                                &entry.original
                            };
                            (entry.id.clone(), text.clone())
                        })
                        .collect::<std::collections::BTreeMap<_, _>>()
                };
                let dump = serde_json::json!({
                    "expected": entries_of(&project.files, true),
                    "exported": entries_of(&reparsed.files, false),
                });
                std::fs::write(
                    options.workspace.join("dump.json"),
                    serde_json::to_string_pretty(&dump).unwrap_or_default(),
                )
                .map_err(|error| error.to_string())?;
            }
            let mut exported: HashMap<String, HashMap<String, i64>> = HashMap::new();
            for file in &reparsed.files {
                let counts = exported.entry(file.relative_path.clone()).or_default();
                for entry in &file.entries {
                    add_words(&entry.original, counts, 1);
                }
            }
            let empty = HashMap::new();
            let mut paths = expected.keys().cloned().collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                let found = exported.get(&path).unwrap_or(&empty);
                let mut missing = expected[&path]
                    .iter()
                    .filter_map(|(word, count)| {
                        let lost = count - found.get(word).copied().unwrap_or(0);
                        (lost > 0).then(|| (word.clone(), lost))
                    })
                    .collect::<Vec<_>>();
                if missing.is_empty() {
                    continue;
                }
                missing.sort();
                let lost = missing
                    .iter()
                    .map(|(_, count)| *count as usize)
                    .sum::<usize>();
                report.roundtrip_missing += lost;
                if report.roundtrip_missing_examples.len() < 8 {
                    report.roundtrip_missing_examples.push(format!(
                        "{path}: {lost} words missing, e.g. {}",
                        missing
                            .iter()
                            .take(6)
                            .map(|(word, _)| word.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }
            }
        }
        Err(error) => report.export_error = Some(error.to_string()),
    }

    report.elapsed_ms = started.elapsed().as_millis();
    Ok(report)
}

fn main() -> ExitCode {
    let options = match parse_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\nUsage: qa_text_fit --game-dir <dir> (--workspace <dir> [--expand 1.3] | --inspect-only)");
            return ExitCode::from(2);
        }
    };
    if options.inspect_only {
        let started = Instant::now();
        return match inspect_game(&options.game_dir) {
            Ok(inspection) => {
                let report = serde_json::json!({
                    "inspection": inspection,
                    "elapsedMs": started.elapsed().as_millis(),
                });
                println!("{report:#}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        };
    }
    match run(&options) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into())
            );
            if report.export_error.is_some() || report.roundtrip_missing > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

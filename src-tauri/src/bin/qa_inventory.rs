use std::env;
use std::process::ExitCode;

use app_lib::parser::{detect_engine, parse_game, resolve_game_dir};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryRow {
    selected_path: String,
    resolved_path: String,
    engine: String,
    files: usize,
    entries: usize,
}

fn print_usage() {
    eprintln!("Uso: cargo run --bin qa_inventory -- [--json] <ruta> [ruta...]");
}

fn main() -> ExitCode {
    let mut json = false;
    let mut paths: Vec<String> = Vec::new();

    for arg in env::args().skip(1) {
        if arg == "--json" {
            json = true;
        } else if arg == "--help" || arg == "-h" {
            print_usage();
            return ExitCode::SUCCESS;
        } else {
            paths.push(arg);
        }
    }

    if paths.is_empty() {
        print_usage();
        return ExitCode::from(2);
    }

    let mut rows: Vec<InventoryRow> = Vec::new();

    for selected in paths {
        let selected_path = std::path::PathBuf::from(&selected);
        let resolved = match resolve_game_dir(&selected_path) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("{} => ERROR: {}", selected, error);
                return ExitCode::from(1);
            }
        };

        let engine = match detect_engine(&resolved) {
            Ok(engine) => engine,
            Err(error) => {
                eprintln!("{} => ERROR: {}", selected, error);
                return ExitCode::from(1);
            }
        };

        let parsed = match parse_game(&resolved) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("{} => ERROR: {}", selected, error);
                return ExitCode::from(1);
            }
        };

        let entries = parsed
            .files
            .iter()
            .map(|file| file.entries.len())
            .sum::<usize>();

        rows.push(InventoryRow {
            selected_path: selected,
            resolved_path: resolved.display().to_string(),
            engine: format!("{engine:?}"),
            files: parsed.files.len(),
            entries,
        });
    }

    if json {
        match serde_json::to_string_pretty(&rows) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("No se pudo serializar inventario: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        for row in rows {
            println!("selected: {}", row.selected_path);
            println!("resolved: {}", row.resolved_path);
            println!("engine: {}", row.engine);
            println!("files: {}", row.files);
            println!("entries: {}", row.entries);
            println!();
        }
    }

    ExitCode::SUCCESS
}

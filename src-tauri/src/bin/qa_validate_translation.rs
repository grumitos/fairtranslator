use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use app_lib::services::persistence;
use app_lib::services::translation_coverage::{
    validate_project_translation_coverage, CoverageOptions, CoverageSeverity,
};

#[derive(Debug)]
struct Options {
    project_path: PathBuf,
    strict_unchanged: bool,
    json: bool,
}

fn print_usage() {
    eprintln!(
        "Uso: cargo run --bin qa_validate_translation -- --project <ruta.fairtl> [--strict-unchanged] [--json]\n\
         \n\
         Opciones:\n\
           --project <ruta>       Proyecto .fairtl a validar\n\
           --strict-unchanged     Trata texto idéntico al original como error\n\
           --json                 Imprime reporte JSON\n\
           --help, -h             Muestra esta ayuda"
    );
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut project_path = None;
    let mut strict_unchanged = false;
    let mut json = false;

    let mut args = argv.skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => {
                project_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "Falta valor para --project".to_string())?,
                ));
            }
            "--strict-unchanged" => strict_unchanged = true,
            "--json" => json = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("Argumento desconocido: {arg}")),
        }
    }

    Ok(Options {
        project_path: project_path.ok_or_else(|| "Debes indicar --project".to_string())?,
        strict_unchanged,
        json,
    })
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
                "No se pudo abrir proyecto '{}': {}",
                options.project_path.display(),
                error
            );
            return ExitCode::from(1);
        }
    };

    let report = validate_project_translation_coverage(
        &project,
        CoverageOptions {
            strict_unchanged: options.strict_unchanged,
            ..CoverageOptions::default()
        },
    );

    if options.json {
        match serde_json::to_string_pretty(&report) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("No se pudo serializar reporte: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        println!("proyecto: {}", report.project_name);
        println!("origen: {}", report.source_lang);
        println!("destino: {}", report.target_lang);
        println!(
            "entradas: {}/{}",
            report.translated_entries, report.total_entries
        );
        println!("omitidas vacías: {}", report.skipped_blank_entries);
        println!("faltantes: {}", report.missing_translations);
        println!(
            "residuos del idioma fuente: {}",
            report.source_script_residue
        );
        println!("sin cambios: {}", report.unchanged_translations);
        println!("errores: {}", report.error_count);
        println!("avisos: {}", report.warning_count);
        println!("estado: {}", if report.complete { "OK" } else { "FALLO" });

        for issue in &report.issues {
            let severity = match issue.severity {
                CoverageSeverity::Error => "ERROR",
                CoverageSeverity::Warning => "AVISO",
            };
            println!(
                "[{}] {:?} {} -> {} | {}",
                severity, issue.kind, issue.file_path, issue.entry_id, issue.message
            );
        }
    }

    if report.complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

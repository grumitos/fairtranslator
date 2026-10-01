use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use app_lib::services::exporter;
use app_lib::services::persistence;
use app_lib::services::text_fit::TextFitReport;
use app_lib::services::translation_coverage::{
    validate_project_translation_coverage, CoverageOptions, CoverageSeverity,
    TranslationCoverageReport,
};
use serde::Serialize;

/// Coverage report plus the text-fit dry run (absent when the source game is
/// not available).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationOutput<'a> {
    #[serde(flatten)]
    coverage: &'a TranslationCoverageReport,
    text_fit: Option<&'a TextFitReport>,
}

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

    // Read-only: measures what an export would wrap, paginate or report.
    let text_fit = match exporter::analyze_text_fit(&project) {
        Ok(text_fit) => Some(text_fit),
        Err(error) => {
            eprintln!("No se pudo analizar el encaje de texto: {error}");
            None
        }
    };

    if options.json {
        let output = ValidationOutput {
            coverage: &report,
            text_fit: text_fit.as_ref(),
        };
        match serde_json::to_string_pretty(&output) {
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
        if let Some(text_fit) = &text_fit {
            println!(
                "encaje: {} entradas reajustadas, {} mensajes con páginas nuevas (+{}), {} sin espacio",
                text_fit.wrapped_entries,
                text_fit.paginated_entries,
                text_fit.pages_added,
                text_fit.overflow_count
            );
            for note in &text_fit.calibration {
                println!("  calibración: {note}");
            }
            for overflow in text_fit.overflow.iter().take(20) {
                println!(
                    "  [ENCAJE] {:?} {} -> {} | {}px de {}px, {} líneas",
                    overflow.kind,
                    overflow.file_path,
                    overflow.entry_id,
                    overflow.width,
                    overflow.max_width,
                    overflow.lines
                );
            }
        }

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

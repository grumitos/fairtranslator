use std::path::PathBuf;

use crate::commands::project::AppState;
use crate::error::AppError;
use crate::models::Project;
use crate::services::exporter::{self, ExportResult};
use crate::services::text_fit::TextFitReport;

fn open_project(state: &AppState) -> Result<Project, AppError> {
    let project = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    project
        .clone()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))
}

/// Runs file-heavy work on the blocking pool so it never stalls other commands.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::Project(format!("Background task failed: {error}")))?
}

/// Export all translated entries back into the original game JSON files.
///
/// Reads original JSON files from the game directory, injects translations,
/// and writes modified copies to `output_dir` preserving relative paths.
/// Original files are never modified.
#[tauri::command]
pub async fn export_project(
    state: tauri::State<'_, AppState>,
    output_dir: String,
) -> Result<ExportResult, AppError> {
    let project = open_project(&state)?;
    let output_path = PathBuf::from(&output_dir);
    run_blocking(move || exporter::export_project(&project, &output_path)).await
}

/// Dry run of the export's text fitting for the open project: which entries
/// would be wrapped, paginated or still overflow. Reads the source game and
/// never writes.
#[tauri::command]
pub async fn text_fit_analyze(
    state: tauri::State<'_, AppState>,
) -> Result<TextFitReport, AppError> {
    let project = open_project(&state)?;
    run_blocking(move || exporter::analyze_text_fit(&project)).await
}

use std::path::PathBuf;

use crate::commands::project::AppState;
use crate::error::AppError;
use crate::services::exporter::{self, ExportResult};

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
    let project = {
        let proj = state
            .project
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        proj.clone()
            .ok_or_else(|| AppError::Project("No project is currently open".into()))?
    };

    let output_path = PathBuf::from(&output_dir);
    exporter::export_project(&project, &output_path)
}

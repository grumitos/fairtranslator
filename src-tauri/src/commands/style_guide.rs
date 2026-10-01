/// Tauri commands for style guide CRUD operations.
///
/// Style guide notes live in the Project's `style_guide` Vec and are persisted
/// with the project file. The guide is intentionally lightweight: short rules
/// that help AI translation stay consistent across the whole game.
use crate::commands::project::AppState;
use crate::error::AppError;
use crate::services::persistence;

fn normalize_items(items: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if normalized
            .iter()
            .any(|existing: &String| existing == trimmed)
        {
            continue;
        }
        normalized.push(trimmed.to_string());
    }

    normalized
}

/// Replaces the guide in memory and, when the project exists on disk, in its
/// `project.json` only. Pending entry edits stay unsaved: the next project
/// save writes them (and the guide) as usual.
fn replace_style_guide(state: &AppState, items: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    let save_path = state
        .project_path
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?
        .clone();
    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    let style_guide = normalize_items(items);
    let updated_at = persistence::now_iso8601();
    if let Some(save_path) = save_path {
        persistence::save_style_guide(&save_path, &style_guide, &updated_at)?;
    }
    project.style_guide = style_guide.clone();
    project.updated_at = updated_at;

    Ok(style_guide)
}

/// Replace the current project style guide with the provided note list.
#[tauri::command]
pub async fn style_guide_replace(
    state: tauri::State<'_, AppState>,
    items: Vec<String>,
) -> Result<Vec<String>, AppError> {
    replace_style_guide(state.inner(), items)
}

/// List all style guide notes for the current project.
#[tauri::command]
pub async fn style_guide_list(state: tauri::State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_ref()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    Ok(project.style_guide.clone())
}

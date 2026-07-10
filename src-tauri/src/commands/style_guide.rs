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

fn replace_style_guide_and_save(
    state: &AppState,
    items: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    let save_path = {
        let path_guard = state
            .project_path
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        path_guard.clone().ok_or_else(|| {
            AppError::Project("No hay ruta de guardado. Guarda el proyecto primero.".into())
        })?
    };
    let mut updated_project = proj_guard
        .clone()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    updated_project.style_guide = normalize_items(items);
    updated_project.updated_at = persistence::now_iso8601();

    // ponytail: keep the project lock through I/O; use revisioned snapshots if save latency matters.
    persistence::save_project(&updated_project, &save_path)?;

    *proj_guard = Some(updated_project.clone());

    Ok(updated_project.style_guide)
}

/// Replace the current project style guide with the provided note list.
#[tauri::command]
pub async fn style_guide_replace(
    state: tauri::State<'_, AppState>,
    items: Vec<String>,
) -> Result<Vec<String>, AppError> {
    replace_style_guide_and_save(state.inner(), items)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, Project, TranslationFile};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_normalize_items_drops_blanks_and_duplicates() {
        let items = normalize_items(vec![
            "  Keep menu labels short  ".into(),
            "".into(),
            "Keep menu labels short".into(),
            "Use informal Spanish".into(),
        ]);

        assert_eq!(
            items,
            vec![
                "Keep menu labels short".to_string(),
                "Use informal Spanish".to_string()
            ]
        );
    }

    fn make_project() -> Project {
        Project {
            id: "style-guide-test".into(),
            name: "Style Guide Test".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\StyleGuideTest".into(),
            files: vec![TranslationFile {
                relative_path: "data/System.json".into(),
                entries: Vec::new(),
            }],
            glossary: Vec::new(),
            style_guide: vec!["Old rule".into()],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn test_replace_style_guide_persists_project_to_disk() {
        let state = AppState::new();
        let temp_path = std::env::temp_dir().join(format!(
            "fairtl_style_guide_save_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp_path.exists() {
            fs::remove_dir_all(&temp_path).unwrap();
        }

        {
            let mut project = state.project.lock().unwrap();
            *project = Some(make_project());
        }
        {
            let mut project_path = state.project_path.lock().unwrap();
            *project_path = Some(PathBuf::from(&temp_path));
        }

        let updated = replace_style_guide_and_save(
            &state,
            vec![
                "Keep menu labels short".into(),
                "Keep menu labels short".into(),
                "Use informal Spanish".into(),
            ],
        )
        .unwrap();

        assert_eq!(
            updated,
            vec![
                "Keep menu labels short".to_string(),
                "Use informal Spanish".to_string()
            ]
        );

        let loaded = persistence::load_project(&temp_path).unwrap();
        assert_eq!(loaded.style_guide, updated);

        let _ = fs::remove_dir_all(&temp_path);
    }
}

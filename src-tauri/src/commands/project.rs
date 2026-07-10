use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::error::AppError;
use crate::models::{EntryStatus, Project};
use crate::parser;
use crate::services::persistence;

/// Shared application state managed by Tauri.
pub struct AppState {
    /// The currently loaded project (None if no project is open).
    pub project: Arc<Mutex<Option<Project>>>,
    /// The path to the `.fairtl` directory for the current project.
    pub project_path: Arc<Mutex<Option<PathBuf>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: Arc::new(Mutex::new(None)),
            project_path: Arc::new(Mutex::new(None)),
        }
    }
}

/// Create a new translation project from an RPG Maker game directory.
///
/// Parses all JSON files in the game's /data/ directory, extracts translatable
/// entries, and returns the new Project. The project is NOT saved to disk yet —
/// call `project_save` to persist it.
#[tauri::command]
pub async fn project_create(
    state: tauri::State<'_, AppState>,
    game_dir: String,
    name: String,
    source_lang: String,
    target_lang: String,
) -> Result<Project, AppError> {
    if source_lang == target_lang {
        return Err(AppError::Project(
            "El idioma de origen y destino deben ser distintos".into(),
        ));
    }

    let selected_path = PathBuf::from(&game_dir);
    if !selected_path.is_dir() {
        return Err(AppError::NotFound(format!(
            "No se encontró la carpeta del juego: {game_dir}"
        )));
    }

    let game_path = parser::resolve_game_dir(&selected_path)?;
    let parser::ParsedGame { engine, files } = parser::parse_game(&game_path)?;

    let now = persistence::now_iso8601();

    let project = Project {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        source_lang,
        target_lang,
        engine,
        game_dir: game_path.to_string_lossy().to_string(),
        files,
        glossary: Vec::new(),
        style_guide: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };

    {
        let default_path = persistence::default_project_path(&game_path);
        let mut proj = state
            .project
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        let mut path = state
            .project_path
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        *proj = Some(project.clone());
        *path = Some(default_path);
    }

    Ok(project)
}

/// Open an existing `.fairtl` project from disk.
#[tauri::command]
pub async fn project_open(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<Project, AppError> {
    let project_path = PathBuf::from(&path);

    let project = persistence::load_project(&project_path)?;

    {
        let mut proj = state
            .project
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        let mut p = state
            .project_path
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        *proj = Some(project.clone());
        *p = Some(project_path);
    }

    Ok(project)
}

/// Save the current project to its `.fairtl` directory.
///
/// If `path` is provided, saves to that location (Save As).
/// Otherwise saves to the existing project path.
#[tauri::command]
pub async fn project_save(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), AppError> {
    let mut proj = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    let mut project = proj
        .clone()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    let save_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        let stored = state
            .project_path
            .lock()
            .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
        stored
            .clone()
            .ok_or_else(|| AppError::Project("No hay ruta de guardado. Usa Guardar como.".into()))?
    };

    // Update timestamp
    project.updated_at = persistence::now_iso8601();

    // ponytail: keep the project lock through I/O; use revisioned snapshots if save latency matters.
    persistence::save_project(&project, &save_path)?;

    // Save succeeded — update in-memory state with new timestamp
    *proj = Some(project);
    let mut stored = state
        .project_path
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    *stored = Some(save_path);

    Ok(())
}

/// Get the currently loaded project, if any.
#[tauri::command]
pub async fn project_get(state: tauri::State<'_, AppState>) -> Result<Option<Project>, AppError> {
    let proj = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;
    Ok(proj.clone())
}

/// Update a single translation entry in the current project.
#[tauri::command]
pub async fn project_update_entry(
    state: tauri::State<'_, AppState>,
    file_path: String,
    entry_id: String,
    translation: String,
    status: EntryStatus,
    notes: Option<String>,
) -> Result<(), AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    apply_entry_update(
        project,
        &ProjectEntryUpdate {
            file_path,
            entry_id,
            translation,
            status,
            notes,
        },
    )?;

    project.updated_at = persistence::now_iso8601();

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntryUpdate {
    pub file_path: String,
    pub entry_id: String,
    pub translation: String,
    pub status: EntryStatus,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn project_update_entries(
    state: tauri::State<'_, AppState>,
    updates: Vec<ProjectEntryUpdate>,
) -> Result<(), AppError> {
    if updates.is_empty() {
        return Ok(());
    }

    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    let mut updated_project = project.clone();

    for update in &updates {
        apply_entry_update(&mut updated_project, update)?;
    }

    updated_project.updated_at = persistence::now_iso8601();
    *project = updated_project;

    Ok(())
}

fn apply_entry_update(project: &mut Project, update: &ProjectEntryUpdate) -> Result<(), AppError> {
    let file = project
        .files
        .iter_mut()
        .find(|f| f.relative_path == update.file_path)
        .ok_or_else(|| {
            AppError::NotFound(format!("Archivo no encontrado: {}", update.file_path))
        })?;

    let entry = file
        .entries
        .iter_mut()
        .find(|e| e.id == update.entry_id)
        .ok_or_else(|| AppError::NotFound(format!("Entrada no encontrada: {}", update.entry_id)))?;

    entry.translation = update.translation.clone();
    entry.status = if update.translation.trim().is_empty() {
        EntryStatus::Untranslated
    } else {
        update.status.clone()
    };
    if let Some(notes) = &update.notes {
        entry.notes = notes.clone();
    }

    Ok(())
}

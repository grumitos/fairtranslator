/// Tauri commands for glossary CRUD operations.
///
/// Glossary terms live in the Project's `glossary` Vec and are persisted
/// with the project file. These commands modify the in-memory project state.
use crate::commands::project::AppState;
use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::persistence;

/// Add a new glossary term to the current project.
/// Returns an error if a term with the same source text already exists.
#[tauri::command]
pub async fn glossary_add(
    state: tauri::State<'_, AppState>,
    source: String,
    target: String,
    case_sensitive: bool,
) -> Result<Vec<GlossaryTerm>, AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    if project.glossary.iter().any(|g| g.source == source) {
        return Err(AppError::Project(format!(
            "El término ya existe en el glosario: \"{}\"",
            source
        )));
    }

    project.glossary.push(GlossaryTerm {
        source,
        target,
        case_sensitive,
    });

    project.updated_at = persistence::now_iso8601();

    Ok(project.glossary.clone())
}

/// Update an existing glossary term (identified by its source text).
/// Allows changing the target translation and case sensitivity.
#[tauri::command]
pub async fn glossary_update(
    state: tauri::State<'_, AppState>,
    source: String,
    target: String,
    case_sensitive: bool,
) -> Result<Vec<GlossaryTerm>, AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    let term = project
        .glossary
        .iter_mut()
        .find(|g| g.source == source)
        .ok_or_else(|| AppError::NotFound(format!("Término no encontrado: \"{}\"", source)))?;

    term.target = target;
    term.case_sensitive = case_sensitive;

    project.updated_at = persistence::now_iso8601();

    Ok(project.glossary.clone())
}

/// Remove a glossary term by its source text.
#[tauri::command]
pub async fn glossary_remove(
    state: tauri::State<'_, AppState>,
    source: String,
) -> Result<Vec<GlossaryTerm>, AppError> {
    let mut proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    let before = project.glossary.len();
    project.glossary.retain(|g| g.source != source);

    if project.glossary.len() == before {
        return Err(AppError::NotFound(format!(
            "Término no encontrado: \"{}\"",
            source
        )));
    }

    project.updated_at = persistence::now_iso8601();

    Ok(project.glossary.clone())
}

/// List all glossary terms in the current project.
#[tauri::command]
pub async fn glossary_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<GlossaryTerm>, AppError> {
    let proj_guard = state
        .project
        .lock()
        .map_err(|e| AppError::Project(format!("Lock error: {e}")))?;

    let project = proj_guard
        .as_ref()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))?;

    Ok(project.glossary.clone())
}

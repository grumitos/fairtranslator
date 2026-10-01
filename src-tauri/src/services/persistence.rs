use std::collections::HashSet;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::{Engine, GlossaryTerm, Project, TranslationEntry, TranslationFile};

// ---------------------------------------------------------------------------
// Schema version — bump when the .fairtl format changes
// ---------------------------------------------------------------------------

const SCHEMA_VERSION: u32 = 2;

fn project_backup_path(project_path: &Path) -> Result<PathBuf, AppError> {
    let parent = project_path
        .parent()
        .ok_or_else(|| AppError::Io("Cannot determine parent directory".into()))?;
    let file_name = project_path
        .file_name()
        .ok_or_else(|| AppError::Io("Cannot determine project directory name".into()))?;
    Ok(parent.join(format!(".{}.backup", file_name.to_string_lossy())))
}

/// Restores the last complete project directory when a process stopped in the
/// narrow window between moving the previous checkpoint aside and publishing
/// the replacement.
pub fn recover_interrupted_save(project_path: &Path) -> Result<bool, AppError> {
    if project_path.exists() {
        return Ok(false);
    }

    let deterministic_backup = project_backup_path(project_path)?;
    let backup = if deterministic_backup.exists() {
        deterministic_backup
    } else {
        let parent = project_path
            .parent()
            .ok_or_else(|| AppError::Io("Cannot determine parent directory".into()))?;
        let is_legacy_qa_workspace = project_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("project.fairtl"))
            .unwrap_or(false)
            && parent.join("qa_translate.status.json").is_file();
        if !is_legacy_qa_workspace {
            return Ok(false);
        }
        let mut legacy_backups = std::fs::read_dir(parent)
            .map_err(|error| {
                AppError::Io(format!(
                    "Failed to inspect project recovery directory '{}': {error}",
                    parent.display()
                ))
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.starts_with(".fairtl_backup_"))
                        .unwrap_or(false)
                    && path.join("project.json").is_file()
            })
            .collect::<Vec<_>>();
        legacy_backups.sort();
        match legacy_backups.len() {
            0 => return Ok(false),
            1 => legacy_backups.remove(0),
            count => {
                return Err(AppError::Io(format!(
                    "Found {count} legacy project checkpoints in '{}'; recovery is ambiguous",
                    parent.display()
                )))
            }
        }
    };
    if !backup.is_dir() {
        return Err(AppError::Io(format!(
            "Project recovery path is not a directory: {}",
            backup.display()
        )));
    }

    std::fs::rename(&backup, project_path).map_err(|error| {
        AppError::Io(format!(
            "Failed to restore interrupted project checkpoint '{}' from '{}': {error}",
            project_path.display(),
            backup.display()
        ))
    })?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// On-disk structures (project.json inside .fairtl directory)
// ---------------------------------------------------------------------------

/// Metadata written to `project.json` inside the `.fairtl` directory.
/// Does NOT include entries — those are stored in `entries/<filename>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMeta {
    schema_version: u32,
    id: String,
    name: String,
    source_lang: String,
    target_lang: String,
    engine: Engine,
    game_dir: String,
    /// Relative paths of files that have entries (matches entries/<name>.json)
    file_list: Vec<String>,
    glossary: Vec<GlossaryTerm>,
    #[serde(default)]
    style_guide: Vec<String>,
    created_at: String,
    updated_at: String,
}

// ---------------------------------------------------------------------------
// Save project
// ---------------------------------------------------------------------------

/// Save a project to a `.fairtl` directory.
///
/// Uses atomic write strategy: writes to a temp directory first, then renames.
/// If `project_path` already exists, the old directory is replaced atomically.
pub fn save_project(project: &Project, project_path: &Path) -> Result<(), AppError> {
    validate_project(project)?;

    // If the previous process stopped during the directory swap, restore the
    // last complete checkpoint before attempting another save.
    recover_interrupted_save(project_path)?;

    let parent = project_path
        .parent()
        .ok_or_else(|| AppError::Io("Cannot determine parent directory".into()))?;
    let temp_dir = parent.join(format!(
        ".fairtl_tmp_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().as_simple()
    ));
    let backup = project_backup_path(project_path)?;

    let result = (|| {
        std::fs::create_dir_all(temp_dir.join("entries"))?;

        let file_list: Vec<String> = project
            .files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();

        let meta = ProjectMeta {
            schema_version: SCHEMA_VERSION,
            id: project.id.clone(),
            name: project.name.clone(),
            source_lang: project.source_lang.clone(),
            target_lang: project.target_lang.clone(),
            engine: project.engine.clone(),
            game_dir: project.game_dir.clone(),
            file_list,
            glossary: project.glossary.clone(),
            style_guide: project.style_guide.clone(),
            created_at: project.created_at.clone(),
            updated_at: project.updated_at.clone(),
        };

        let meta_json = serde_json::to_string_pretty(&meta)?;
        write_synced(&temp_dir.join("project.json"), meta_json.as_bytes())?;

        // Write entries/<stem>.json for each file
        for file in &project.files {
            let entry_filename = entries_filename(&file.relative_path);
            let entries_json = serde_json::to_string_pretty(&file.entries)?;
            write_synced(
                &temp_dir.join("entries").join(&entry_filename),
                entries_json.as_bytes(),
            )?;
        }
        sync_dir(&temp_dir.join("entries"))?;
        sync_dir(&temp_dir)?;

        // Atomic swap: remove old, rename temp to final
        if project_path.exists() {
            // Rename old to a backup first, so if rename fails we don't lose data
            if backup.exists() {
                std::fs::remove_dir_all(&backup)?;
            }
            std::fs::rename(project_path, &backup)?;
            match std::fs::rename(&temp_dir, project_path) {
                Ok(()) => {
                    // Success — remove backup
                    let _ = std::fs::remove_dir_all(&backup);
                }
                Err(e) => {
                    // Restore backup
                    let _ = std::fs::rename(&backup, project_path);
                    return Err(AppError::Io(format!("Failed to finalize save: {e}")));
                }
            }
        } else {
            std::fs::rename(&temp_dir, project_path)?;
        }
        sync_dir(parent)?;

        Ok(())
    })();

    // The temp name is unique per save, so a failed attempt must remove its own
    // snapshot or it is left orphaned next to the project forever. Keep it only
    // when both the swap and the backup restore failed: then it is the sole
    // on-disk copy of the newest data.
    if result.is_err() && (project_path.exists() || !backup.exists()) {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    result
}

/// Writes a file and flushes it to the disk (`FlushFileBuffers` on Windows),
/// so a save that reached its final rename survives a power cut.
pub(crate) fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Flushes a directory's entries (the renames into it). Windows offers no
/// supported way to flush a directory, so there this is a no-op: NTFS
/// journals the rename itself, and the files were flushed before it.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(dir)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Saves only the style guide of an already saved project: `project.json` is
/// replaced atomically and the entry files on disk are left alone, so pending
/// translation edits are not written with it. Returns `false` when the project
/// has not been saved yet (the guide is then saved with the project).
pub fn save_style_guide(
    project_path: &Path,
    style_guide: &[String],
    updated_at: &str,
) -> Result<bool, AppError> {
    recover_interrupted_save(project_path)?;
    let meta_path = project_path.join("project.json");
    if !meta_path.is_file() {
        return Ok(false);
    }
    let mut meta =
        migrate_project_meta(serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?)?;
    meta.style_guide = style_guide.to_vec();
    meta.updated_at = updated_at.to_string();

    let temp_path = project_path.join(format!(
        ".project.json.tmp_{}",
        uuid::Uuid::new_v4().as_simple()
    ));
    let result = write_synced(&temp_path, serde_json::to_string_pretty(&meta)?.as_bytes())
        .and_then(|()| std::fs::rename(&temp_path, &meta_path))
        .and_then(|()| sync_dir(project_path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Load project
// ---------------------------------------------------------------------------

/// Load a project from a `.fairtl` directory.
pub fn load_project(project_path: &Path) -> Result<Project, AppError> {
    recover_interrupted_save(project_path)?;

    if !project_path.is_dir() {
        return Err(AppError::NotFound(format!(
            "Project directory not found: {}",
            project_path.display()
        )));
    }

    // Read project.json
    let meta_path = project_path.join("project.json");
    if !meta_path.exists() {
        return Err(AppError::NotFound(format!(
            "project.json not found in {}",
            project_path.display()
        )));
    }

    let meta_content = std::fs::read_to_string(&meta_path)?;
    let meta: ProjectMeta = migrate_project_meta(serde_json::from_str(&meta_content)?)?;
    validate_project_meta(&meta)?;

    // Read entry files
    let entries_dir = project_path.join("entries");
    let mut files: Vec<TranslationFile> = Vec::new();

    let mut missing_entry_files: Vec<String> = Vec::new();

    for rel_path in &meta.file_list {
        let entry_filename = entries_filename(rel_path);
        let entry_path = entries_dir.join(&entry_filename);

        if entry_path.exists() {
            let content = std::fs::read_to_string(&entry_path)?;
            let entries: Vec<TranslationEntry> = serde_json::from_str(&content)?;
            files.push(TranslationFile {
                relative_path: rel_path.clone(),
                entries,
            });
        } else {
            missing_entry_files.push(entry_filename);
        }
    }

    if !missing_entry_files.is_empty() {
        missing_entry_files.sort();
        return Err(AppError::Project(format!(
            "Project data is incomplete. Missing entry files in '{}': {}",
            entries_dir.display(),
            missing_entry_files.join(", ")
        )));
    }

    let project = Project {
        id: meta.id,
        name: meta.name,
        source_lang: meta.source_lang,
        target_lang: meta.target_lang,
        engine: meta.engine,
        game_dir: meta.game_dir,
        files,
        glossary: meta.glossary,
        style_guide: meta.style_guide,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
    };
    validate_project(&project)?;

    Ok(project)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn migrate_project_meta(mut meta: ProjectMeta) -> Result<ProjectMeta, AppError> {
    match meta.schema_version {
        1 => {
            meta.schema_version = SCHEMA_VERSION;
            Ok(meta)
        }
        SCHEMA_VERSION => Ok(meta),
        version if version > SCHEMA_VERSION => Err(AppError::Parse(format!(
            "Project was saved with schema version {} but this app only supports up to version {}. Please update FairTranslator.",
            version, SCHEMA_VERSION
        ))),
        version => Err(AppError::Parse(format!(
            "Unsupported project schema version {version}."
        ))),
    }
}

fn validate_project_meta(meta: &ProjectMeta) -> Result<(), AppError> {
    validate_relative_paths(&meta.file_list)?;
    validate_entry_filename_collisions(&meta.file_list)
}

fn validate_project(project: &Project) -> Result<(), AppError> {
    let file_list = project
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<Vec<_>>();
    validate_relative_paths(&file_list)?;
    validate_entry_filename_collisions(&file_list)?;
    validate_translation_files(&project.files)
}

fn validate_relative_paths(paths: &[String]) -> Result<(), AppError> {
    let mut seen = HashSet::new();

    for path in paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(AppError::Project(
                "Project file list contains an empty relative path.".to_string(),
            ));
        }
        if !seen.insert(trimmed.to_string()) {
            return Err(AppError::Project(format!(
                "Project file list contains duplicate path: {trimmed}"
            )));
        }

        let path = Path::new(trimmed);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(AppError::Project(format!(
                "Project file list contains unsafe relative path: {trimmed}"
            )));
        }
    }

    Ok(())
}

fn validate_entry_filename_collisions(paths: &[String]) -> Result<(), AppError> {
    let mut entry_filenames = HashSet::new();

    for relative_path in paths {
        let entry_filename = entries_filename(relative_path);
        let collision_key = entry_filename.to_lowercase();
        if !entry_filenames.insert(collision_key) {
            return Err(AppError::Project(format!(
                "Project file list maps multiple files to entries/{entry_filename}"
            )));
        }
    }

    Ok(())
}

fn validate_translation_files(files: &[TranslationFile]) -> Result<(), AppError> {
    for file in files {
        let mut entry_ids = HashSet::new();
        for entry in &file.entries {
            if entry.id.trim().is_empty() {
                return Err(AppError::Project(format!(
                    "Project file '{}' contains an entry with an empty ID.",
                    file.relative_path
                )));
            }
            if !entry_ids.insert(entry.id.as_str()) {
                return Err(AppError::Project(format!(
                    "Project file '{}' contains duplicate entry ID: {}",
                    file.relative_path, entry.id
                )));
            }
        }
    }

    Ok(())
}

/// Convert a relative path like "data/Map001.json" into an entries filename "Map001.json".
/// If it already has no directory component, returns as-is.
fn entries_filename(relative_path: &str) -> String {
    let p = Path::new(relative_path);
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(relative_path)
        .to_string()
}

/// Default location of a new project: `<projects_dir>\<name>.fairtl` (the
/// app passes `Documents\FairTranslator`), never inside the game folder. The
/// name is made safe for file names, and " (2)", " (3)"... avoid clobbering
/// an existing project.
pub fn default_project_path(projects_dir: &Path, project_name: &str) -> PathBuf {
    let stem = file_stem_for(project_name);
    let mut candidate = projects_dir.join(format!("{stem}.fairtl"));
    let mut counter = 2;
    while candidate.exists() || project_backup_path(&candidate).is_ok_and(|backup| backup.exists())
    {
        candidate = projects_dir.join(format!("{stem} ({counter}).fairtl"));
        counter += 1;
    }
    candidate
}

fn file_stem_for(name: &str) -> String {
    let cleaned = name
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .take(80)
        .collect::<String>();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']);
    if cleaned.is_empty() {
        "project".to_string()
    } else {
        cleaned.to_string()
    }
}

/// The folder the user sees as "the game": the resolved root, or its parent
/// for MV's deployed `www` layout.
fn game_folder(game_dir: &Path) -> PathBuf {
    let is_www = game_dir
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("www"));
    match game_dir.parent() {
        Some(parent) if is_www => parent.to_path_buf(),
        _ => game_dir.to_path_buf(),
    }
}

fn comparable_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return canonical;
    }
    // A target that does not exist yet: resolve its parent.
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => std::fs::canonicalize(parent)
            .map(|parent| parent.join(name))
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// Validates a "Save as" target and returns the path to save to: absolute,
/// named `*.fairtl` (added when missing), outside the game folder, and either
/// new or an existing FairTranslator project. Any other existing folder or
/// file is refused, because the save swap replaces whatever is at the path.
pub fn save_as_target(requested: &Path, game_dir: &Path) -> Result<PathBuf, AppError> {
    if !requested.is_absolute() {
        return Err(AppError::Project(format!(
            "La ruta de «Guardar como» debe ser absoluta: {}",
            requested.display()
        )));
    }
    let has_extension = requested
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("fairtl"));
    let target = if has_extension {
        requested.to_path_buf()
    } else {
        let mut name = requested
            .file_name()
            .ok_or_else(|| {
                AppError::Project(format!(
                    "La ruta de «Guardar como» no tiene nombre: {}",
                    requested.display()
                ))
            })?
            .to_os_string();
        name.push(".fairtl");
        requested.with_file_name(name)
    };

    let game_folder = game_folder(game_dir);
    if !game_dir.as_os_str().is_empty()
        && comparable_path(&target).starts_with(comparable_path(&game_folder))
    {
        return Err(AppError::Project(format!(
            "Guarda el proyecto fuera de la carpeta del juego ({}).",
            game_folder.display()
        )));
    }
    if target.exists() && !target.join("project.json").is_file() {
        return Err(AppError::Project(format!(
            "«{}» ya existe y no es un proyecto de FairTranslator; elige otro nombre.",
            target.display()
        )));
    }
    Ok(target)
}

/// Get the current timestamp as an ISO 8601 string.
pub fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EntryStatus;
    use std::fs;

    fn make_test_project() -> Project {
        Project {
            id: "test-id-123".to_string(),
            name: "Test RPG".to_string(),
            source_lang: "ja".to_string(),
            target_lang: "es".to_string(),
            engine: Engine::Mv,
            game_dir: "C:\\Games\\TestRPG".to_string(),
            files: vec![
                TranslationFile {
                    relative_path: "data/Actors.json".to_string(),
                    entries: vec![
                        TranslationEntry {
                            id: "Actors:name:1".to_string(),
                            file_context: "Actors > #1 (Harold) > name".to_string(),
                            original: "ハロルド".to_string(),
                            translation: "Harold".to_string(),
                            status: EntryStatus::Draft,
                            notes: "Main character".to_string(),
                        },
                        TranslationEntry {
                            id: "Actors:name:2".to_string(),
                            file_context: "Actors > #2 (Therese) > name".to_string(),
                            original: "テレーゼ".to_string(),
                            translation: String::new(),
                            status: EntryStatus::Untranslated,
                            notes: String::new(),
                        },
                    ],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".to_string(),
                    entries: vec![TranslationEntry {
                        id: "Map001:displayName".to_string(),
                        file_context: "Map001 > displayName".to_string(),
                        original: "はじまりの村".to_string(),
                        translation: "Pueblo Inicial".to_string(),
                        status: EntryStatus::Reviewed,
                        notes: String::new(),
                    }],
                },
            ],
            glossary: vec![GlossaryTerm {
                source: "勇者".to_string(),
                target: "Héroe".to_string(),
                case_sensitive: false,
            }],
            style_guide: vec!["Keep menu text concise.".to_string()],
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let project = make_test_project();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_roundtrip_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));

        // Clean up from any previous run
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }

        // Save
        save_project(&project, &temp).unwrap();

        // Verify directory structure
        assert!(temp.join("project.json").exists());
        assert!(temp.join("entries").join("Actors.json").exists());
        assert!(temp.join("entries").join("Map001.json").exists());

        // Load
        let loaded = load_project(&temp).unwrap();

        // Compare
        assert_eq!(loaded.id, project.id);
        assert_eq!(loaded.name, project.name);
        assert_eq!(loaded.source_lang, project.source_lang);
        assert_eq!(loaded.target_lang, project.target_lang);
        assert_eq!(loaded.game_dir, project.game_dir);
        assert_eq!(loaded.files.len(), 2);
        assert_eq!(loaded.files[0].entries.len(), 2);
        assert_eq!(loaded.files[0].entries[0].original, "ハロルド");
        assert_eq!(loaded.files[0].entries[0].translation, "Harold");
        assert_eq!(loaded.files[1].entries[0].original, "はじまりの村");
        assert_eq!(loaded.glossary.len(), 1);
        assert_eq!(loaded.glossary[0].source, "勇者");
        assert_eq!(loaded.glossary[0].target, "Héroe");
        assert_eq!(loaded.style_guide, project.style_guide);

        // Clean up
        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn test_save_overwrites_existing() {
        let mut project = make_test_project();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_overwrite_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));

        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }

        // Save v1
        save_project(&project, &temp).unwrap();

        // Modify and save v2
        project.name = "Updated RPG".to_string();
        project.files[0].entries[1].translation = "Teresa".to_string();
        save_project(&project, &temp).unwrap();

        // Load and verify v2
        let loaded = load_project(&temp).unwrap();
        assert_eq!(loaded.name, "Updated RPG");
        assert_eq!(loaded.files[0].entries[1].translation, "Teresa");

        // Clean up
        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn test_load_recovers_checkpoint_interrupted_during_atomic_swap() {
        let project = make_test_project();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_recover_swap_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        let backup = temp.with_file_name(format!(
            ".{}.backup",
            temp.file_name().unwrap().to_string_lossy()
        ));

        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(&backup);
        save_project(&project, &temp).unwrap();

        // Simulate an interruption after the current checkpoint was moved out
        // of the canonical path but before the replacement became visible.
        fs::rename(&temp, &backup).unwrap();

        let loaded = load_project(&temp).unwrap();

        assert_eq!(loaded.id, project.id);
        assert!(temp.exists());
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(&backup);
    }

    #[test]
    fn test_load_recovers_unambiguous_legacy_checkpoint_backup() {
        let project = make_test_project();
        let root = std::env::temp_dir().join(format!(
            "fairtl_test_recover_legacy_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let temp = root.join("project.fairtl");
        let legacy_backup = root.join(format!(
            ".fairtl_backup_999_{}",
            uuid::Uuid::new_v4().as_simple()
        ));

        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("qa_translate.status.json"), "{}").unwrap();
        save_project(&project, &temp).unwrap();
        fs::rename(&temp, &legacy_backup).unwrap();

        let loaded = load_project(&temp).unwrap();

        assert_eq!(loaded.id, project.id);
        assert!(temp.exists());
        assert!(!legacy_backup.exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_entries_filename() {
        assert_eq!(entries_filename("data/Map001.json"), "Map001.json");
        assert_eq!(entries_filename("Map001.json"), "Map001.json");
        assert_eq!(entries_filename("data/sub/Actors.json"), "Actors.json");
    }

    /// New projects default to a projects folder (never the game folder),
    /// with a file-safe, non-clobbering name; "Save as" only accepts absolute
    /// `.fairtl` targets outside the game folder (MV's `www` parent included)
    /// that are new or already a project, because the save swap replaces
    /// whatever is at the target.
    #[test]
    fn default_and_save_as_paths_stay_out_of_game_folders() {
        let root =
            std::env::temp_dir().join(format!("fairtl_paths_{}", uuid::Uuid::new_v4().as_simple()));
        let projects = root.join("Documents").join("FairTranslator");
        let game = root.join("Games").join("MyRPG");
        let www = game.join("www");
        fs::create_dir_all(www.join("data")).unwrap();
        fs::create_dir_all(&projects).unwrap();

        let first = default_project_path(&projects, "  My: RPG?  ");
        assert_eq!(first, projects.join("My_ RPG_.fairtl"));
        fs::create_dir_all(&first).unwrap();
        assert_eq!(
            default_project_path(&projects, "My: RPG?"),
            projects.join("My_ RPG_ (2).fairtl")
        );
        assert_eq!(
            default_project_path(&projects, " .. "),
            projects.join("project.fairtl")
        );

        let error = |target: &Path| save_as_target(target, &www).unwrap_err().to_string();
        assert!(error(Path::new("relative.fairtl")).contains("absoluta"));
        assert!(error(&game.join("Traduccion.fairtl")).contains("fuera de la carpeta del juego"));
        assert!(error(&www.join("data").join("x")).contains("fuera de la carpeta del juego"));
        // An existing folder is never the target itself: `.fairtl` is added.
        assert_eq!(
            save_as_target(&root.join("Documents"), &www).unwrap(),
            root.join("Documents.fairtl")
        );
        let stray = root.join("Stray.fairtl");
        fs::create_dir_all(stray.join("keep")).unwrap();
        assert!(error(&stray).contains("no es un proyecto"));
        let file = root.join("notes.fairtl");
        fs::write(&file, "keep").unwrap();
        assert!(error(&file).contains("no es un proyecto"));

        assert_eq!(
            save_as_target(&root.join("Games").join("MyRPG ES"), &www).unwrap(),
            root.join("Games").join("MyRPG ES.fairtl")
        );
        let project = make_test_project();
        save_project(&project, &first).unwrap();
        assert_eq!(save_as_target(&first, &www).unwrap(), first);

        let _ = fs::remove_dir_all(&root);
    }

    /// Changing the style guide of a saved project rewrites only its
    /// `project.json`; pending entry edits reach the disk with the next save.
    /// A project that was never saved is left for that first save.
    #[test]
    fn style_guide_save_leaves_pending_entry_edits_unsaved() {
        let path = std::env::temp_dir().join(format!(
            "fairtl_style_guide_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        let mut project = make_test_project();
        save_project(&project, &path).unwrap();

        project.files[0].entries[1].translation = "Teresa".into();
        let guide = vec!["Usa tú".to_string()];
        assert!(save_style_guide(&path, &guide, "2026-09-30T00:00:00Z").unwrap());

        let on_disk = load_project(&path).unwrap();
        assert_eq!(on_disk.style_guide, guide);
        assert_eq!(on_disk.updated_at, "2026-09-30T00:00:00Z");
        assert_eq!(on_disk.files[0].entries[1].translation, "");
        assert_eq!(
            serde_json::to_value(&on_disk.files).unwrap(),
            serde_json::to_value(make_test_project().files).unwrap()
        );
        assert!(fs::read_dir(&path)
            .unwrap()
            .flatten()
            .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp")));

        let unsaved = path.with_file_name(format!(
            "fairtl_style_guide_unsaved_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        assert!(!save_style_guide(&unsaved, &guide, "2026-09-30T00:00:00Z").unwrap());
        assert!(!unsaved.exists());

        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_load_schema_version_mismatch() {
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_schema_mismatch_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(temp.join("entries")).unwrap();

        // Write project.json with a future schema version
        let meta_json = serde_json::json!({
            "schemaVersion": 999,
            "id": "test-id",
            "name": "Test",
            "sourceLang": "ja",
            "targetLang": "es",
            "engine": "mv",
            "gameDir": "C:\\Games\\Test",
            "fileList": [],
            "glossary": [],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        });
        fs::write(temp.join("project.json"), meta_json.to_string()).unwrap();

        let result = load_project(&temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("schema version"),
            "Expected schema version error, got: {err_msg}"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_load_fails_when_entries_file_is_missing() {
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_missing_entries_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(temp.join("entries")).unwrap();

        let meta_json = serde_json::json!({
            "schemaVersion": 1,
            "id": "test-id",
            "name": "Test",
            "sourceLang": "ja",
            "targetLang": "es",
            "engine": "mv",
            "gameDir": "C:\\Games\\Test",
            "fileList": ["data/Actors.json"],
            "glossary": [],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        });
        fs::write(temp.join("project.json"), meta_json.to_string()).unwrap();

        let result = load_project(&temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Missing entry files"),
            "Expected missing entry file error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_save_writes_current_schema_version() {
        let project = make_test_project();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_schema_write_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }

        save_project(&project, &temp).unwrap();

        let project_json = fs::read_to_string(temp.join("project.json")).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&project_json).unwrap();
        assert_eq!(
            meta.get("schemaVersion").and_then(|value| value.as_u64()),
            Some(SCHEMA_VERSION as u64)
        );
        assert_eq!(
            meta.get("styleGuide")
                .and_then(|value| value.as_array())
                .map(|values| values.len()),
            Some(project.style_guide.len())
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_loads_schema_v1_project_without_style_guide() {
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_schema_v1_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(temp.join("entries")).unwrap();

        let meta_json = serde_json::json!({
            "schemaVersion": 1,
            "id": "legacy-project",
            "name": "Legacy",
            "sourceLang": "ja",
            "targetLang": "es",
            "engine": "mv",
            "gameDir": "C:\\Games\\Legacy",
            "fileList": ["data/System.json"],
            "glossary": [],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        });
        fs::write(temp.join("project.json"), meta_json.to_string()).unwrap();
        fs::write(temp.join("entries").join("System.json"), "[]").unwrap();

        let loaded = load_project(&temp).unwrap();
        assert!(loaded.style_guide.is_empty());
        assert_eq!(loaded.name, "Legacy");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_load_rejects_unsafe_file_list_path() {
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_unsafe_path_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(temp.join("entries")).unwrap();

        let meta_json = serde_json::json!({
            "schemaVersion": 2,
            "id": "test-id",
            "name": "Test",
            "sourceLang": "ja",
            "targetLang": "es",
            "engine": "mv",
            "gameDir": "C:\\Games\\Test",
            "fileList": ["../Actors.json"],
            "glossary": [],
            "styleGuide": [],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        });
        fs::write(temp.join("project.json"), meta_json.to_string()).unwrap();

        let result = load_project(&temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("unsafe relative path"),
            "Expected unsafe path error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_load_rejects_duplicate_file_list_paths() {
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_duplicate_paths_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(temp.join("entries")).unwrap();

        let meta_json = serde_json::json!({
            "schemaVersion": 2,
            "id": "test-id",
            "name": "Test",
            "sourceLang": "ja",
            "targetLang": "es",
            "engine": "mv",
            "gameDir": "C:\\Games\\Test",
            "fileList": ["data/Actors.json", "data/Actors.json"],
            "glossary": [],
            "styleGuide": [],
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z"
        });
        fs::write(temp.join("project.json"), meta_json.to_string()).unwrap();
        fs::write(temp.join("entries").join("Actors.json"), "[]").unwrap();

        let result = load_project(&temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("duplicate path"),
            "Expected duplicate path error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_save_rejects_duplicate_entry_ids() {
        let mut project = make_test_project();
        project.files[0].entries[1].id = project.files[0].entries[0].id.clone();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_duplicate_entries_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));

        let result = save_project(&project, &temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("duplicate entry ID"),
            "Expected duplicate entry ID error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_save_rejects_entry_filename_collisions() {
        let mut project = make_test_project();
        project.files[1].relative_path = "other/Actors.json".to_string();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_entry_filename_collision_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));

        let result = save_project(&project, &temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("maps multiple files"),
            "Expected entry filename collision error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_save_rejects_case_insensitive_entry_filename_collisions() {
        let mut project = make_test_project();
        project.files[1].relative_path = "other/actors.json".to_string();
        let temp = std::env::temp_dir().join(format!(
            "fairtl_test_entry_filename_case_collision_{}.fairtl",
            uuid::Uuid::new_v4().as_simple()
        ));

        let result = save_project(&project, &temp);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("maps multiple files"),
            "Expected case-insensitive entry filename collision error, got: {err_msg}"
        );

        let _ = fs::remove_dir_all(&temp);
    }
}

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::{Engine, GlossaryTerm, Project, TranslationEntry, TranslationFile};

// ---------------------------------------------------------------------------
// Schema version — bump when the .fairtl format changes
// ---------------------------------------------------------------------------

const SCHEMA_VERSION: u32 = 1;

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
    // Build a temp path next to the final destination
    let parent = project_path
        .parent()
        .ok_or_else(|| AppError::Io("Cannot determine parent directory".into()))?;
    let temp_dir = parent.join(format!(
        ".fairtl_tmp_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().as_simple()
    ));

    // Clean up any leftover temp dir from a previous crashed save
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }

    // Create temp directory structure
    std::fs::create_dir_all(temp_dir.join("entries"))?;

    // Build metadata
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
        created_at: project.created_at.clone(),
        updated_at: project.updated_at.clone(),
    };

    // Write project.json
    let meta_json = serde_json::to_string_pretty(&meta)?;
    std::fs::write(temp_dir.join("project.json"), meta_json)?;

    // Write entries/<stem>.json for each file
    for file in &project.files {
        let entry_filename = entries_filename(&file.relative_path);
        let entries_json = serde_json::to_string_pretty(&file.entries)?;
        std::fs::write(temp_dir.join("entries").join(&entry_filename), entries_json)?;
    }

    // Atomic swap: remove old, rename temp to final
    if project_path.exists() {
        // Rename old to a backup first, so if rename fails we don't lose data
        let backup = parent.join(format!(
            ".fairtl_backup_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().as_simple()
        ));
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

    Ok(())
}

// ---------------------------------------------------------------------------
// Load project
// ---------------------------------------------------------------------------

/// Load a project from a `.fairtl` directory.
pub fn load_project(project_path: &Path) -> Result<Project, AppError> {
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
    let meta: ProjectMeta = serde_json::from_str(&meta_content)?;

    // Version check
    if meta.schema_version > SCHEMA_VERSION {
        return Err(AppError::Parse(format!(
            "Project was saved with schema version {} but this app only supports up to version {}. Please update FairTranslator.",
            meta.schema_version, SCHEMA_VERSION
        )));
    }

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

    Ok(Project {
        id: meta.id,
        name: meta.name,
        source_lang: meta.source_lang,
        target_lang: meta.target_lang,
        engine: meta.engine,
        game_dir: meta.game_dir,
        files,
        glossary: meta.glossary,
        created_at: meta.created_at,
        updated_at: meta.updated_at,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a relative path like "data/Map001.json" into an entries filename "Map001.json".
/// If it already has no directory component, returns as-is.
fn entries_filename(relative_path: &str) -> String {
    let p = Path::new(relative_path);
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(relative_path)
        .to_string()
}

/// Generate a default `.fairtl` path from a game directory path.
/// E.g. `C:\Games\MyRPG` -> `C:\Games\MyRPG.fairtl`
pub fn default_project_path(game_dir: &Path) -> PathBuf {
    let mut path = game_dir.to_path_buf();
    let name = game_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    path.pop();
    path.push(format!("{name}.fairtl"));
    path
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
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let project = make_test_project();
        let temp = std::env::temp_dir().join("fairtl_test_roundtrip.fairtl");

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

        // Clean up
        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn test_save_overwrites_existing() {
        let mut project = make_test_project();
        let temp = std::env::temp_dir().join("fairtl_test_overwrite.fairtl");

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
    fn test_entries_filename() {
        assert_eq!(entries_filename("data/Map001.json"), "Map001.json");
        assert_eq!(entries_filename("Map001.json"), "Map001.json");
        assert_eq!(entries_filename("data/sub/Actors.json"), "Actors.json");
    }

    #[test]
    fn test_default_project_path() {
        let game_dir = Path::new("C:\\Games\\MyRPG");
        let result = default_project_path(game_dir);
        assert_eq!(result, PathBuf::from("C:\\Games\\MyRPG.fairtl"));
    }

    #[test]
    fn test_load_nonexistent() {
        let result = load_project(Path::new("C:\\nonexistent\\path.fairtl"));
        assert!(result.is_err());
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
}

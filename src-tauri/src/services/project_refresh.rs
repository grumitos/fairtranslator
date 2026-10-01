//! Keeps saved projects in step with the parser. Entries the current parser
//! extracts but an older project lacks (new parser coverage, such as XP's
//! first message lines) are added in place; existing entries are never
//! renumbered or changed.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::AppError;
use crate::models::{Project, TranslationEntry, TranslationFile};
use crate::parser;
use crate::services::persistence;

/// A project opened from disk plus what the source refresh did to it.
pub struct OpenedProject {
    pub project: Project,
    /// Entries added because the parser now extracts them.
    pub added_entries: usize,
    /// Why the refresh was skipped (source missing or unreadable), if it was.
    pub refresh_skipped: Option<String>,
}

/// Loads a `.fairtl` project and adds the entries its source game now yields.
/// A missing or unreadable source never blocks opening the saved work.
pub fn open_project(path: &Path) -> Result<OpenedProject, AppError> {
    let mut project = persistence::load_project(path)?;
    let game_dir = Path::new(&project.game_dir);
    if !game_dir.is_dir() {
        return Ok(OpenedProject {
            project,
            added_entries: 0,
            refresh_skipped: Some("source game folder not found".into()),
        });
    }

    match parser::parse_game(game_dir) {
        Ok(parsed) if parsed.engine == project.engine => {
            let added_entries = merge_new_entries(&mut project, parsed.files);
            Ok(OpenedProject {
                project,
                added_entries,
                refresh_skipped: None,
            })
        }
        Ok(parsed) => Ok(OpenedProject {
            refresh_skipped: Some(format!(
                "source engine is now {:?}, project uses {:?}",
                parsed.engine, project.engine
            )),
            project,
            added_entries: 0,
        }),
        Err(error) => Ok(OpenedProject {
            project,
            added_entries: 0,
            refresh_skipped: Some(error.to_string()),
        }),
    }
}

/// Adds entries found by the parser but missing from the project, at their
/// parse position. Existing entries keep their ID, translation, status, notes
/// and even a drifted original; entries the source no longer has stay at the
/// end of their file, where the export's drift check reports them.
pub fn merge_new_entries(project: &mut Project, parsed_files: Vec<TranslationFile>) -> usize {
    let original_order = project
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<Vec<_>>();
    let mut existing = std::mem::take(&mut project.files)
        .into_iter()
        .map(|file| (file.relative_path.clone(), file))
        .collect::<HashMap<_, _>>();

    let mut added = 0usize;
    let mut files = Vec::with_capacity(existing.len() + parsed_files.len());
    for parsed in parsed_files {
        match existing.remove(&parsed.relative_path) {
            Some(file) => {
                let (merged, count) = merge_file(file, parsed.entries);
                added += count;
                files.push(merged);
            }
            None => {
                added += parsed.entries.len();
                files.push(parsed);
            }
        }
    }
    for path in original_order {
        if let Some(file) = existing.remove(&path) {
            files.push(file);
        }
    }

    project.files = files;
    added
}

fn merge_file(
    mut file: TranslationFile,
    parsed: Vec<TranslationEntry>,
) -> (TranslationFile, usize) {
    let known = file
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    if parsed.iter().all(|entry| known.contains(&entry.id)) {
        // Nothing new: leave the saved order untouched.
        return (file, 0);
    }

    let saved_order = file
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let mut saved = std::mem::take(&mut file.entries)
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<HashMap<_, _>>();

    let mut added = 0usize;
    let mut entries = Vec::with_capacity(saved.len() + parsed.len());
    for entry in parsed {
        match saved.remove(&entry.id) {
            Some(existing) => entries.push(existing),
            None => {
                added += 1;
                entries.push(entry);
            }
        }
    }
    for id in saved_order {
        if let Some(entry) = saved.remove(&id) {
            entries.push(entry);
        }
    }

    file.entries = entries;
    (file, added)
}

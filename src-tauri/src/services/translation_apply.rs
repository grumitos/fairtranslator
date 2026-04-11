use crate::error::AppError;
use crate::models::{EntryStatus, Project};
use crate::services::persistence;
use crate::services::provider::TranslationResponse;
use crate::services::translation_integrity::{self, IntegrityMode};

#[derive(Debug, Default)]
pub struct ProviderApplyResult {
    pub applied_entries: usize,
    pub missing_project_entries: Vec<String>,
    pub invalid_entries: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn apply_provider_responses(
    project: &mut Project,
    file_path: &str,
    responses: &[TranslationResponse],
) -> Result<ProviderApplyResult, AppError> {
    let file = project
        .files
        .iter_mut()
        .find(|f| f.relative_path == file_path)
        .ok_or_else(|| AppError::NotFound(format!("File not found: {file_path}")))?;

    let mut result = ProviderApplyResult::default();

    for response in responses {
        let Some(entry) = file
            .entries
            .iter_mut()
            .find(|entry| entry.id == response.entry_id)
        else {
            result
                .missing_project_entries
                .push(response.entry_id.clone());
            continue;
        };

        let normalized_translation = translation_integrity::normalize_translation_for_apply(
            entry,
            &response.translated_text,
        );
        let validation = translation_integrity::validate_translation(
            file_path,
            entry,
            &normalized_translation,
            IntegrityMode::AutomatedApply,
        );
        if validation.has_errors() {
            result.invalid_entries.push(response.entry_id.clone());
            append_limited(&mut result.warnings, &validation.errors);
            append_limited(&mut result.warnings, &validation.warnings);
            continue;
        }

        append_limited(&mut result.warnings, &validation.warnings);
        entry.translation = normalized_translation;
        entry.status = EntryStatus::Draft;
        result.applied_entries += 1;
    }

    if result.applied_entries > 0 {
        project.updated_at = persistence::now_iso8601();
    }

    Ok(result)
}

fn append_limited(target: &mut Vec<String>, messages: &[String]) {
    const MAX_WARNINGS: usize = 8;
    for message in messages {
        if target.len() >= MAX_WARNINGS {
            break;
        }
        if !target.contains(message) {
            target.push(message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, TranslationEntry, TranslationFile};

    fn make_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "Apply Test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\Test".into(),
            files: vec![TranslationFile {
                relative_path: "data/Map001.json".into(),
                entries: vec![TranslationEntry {
                    id: "Map001:1:0:text:0".into(),
                    file_context: "ctx".into(),
                    original: "\\C[4]Line 1\nLine 2".into(),
                    translation: String::new(),
                    status: EntryStatus::Untranslated,
                    notes: String::new(),
                }],
            }],
            glossary: Vec::new(),
            style_guide: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn test_apply_provider_responses_skips_invalid_integrity_breaks() {
        let mut project = make_project();
        let result = apply_provider_responses(
            &mut project,
            "data/Map001.json",
            &[TranslationResponse {
                entry_id: "Map001:1:0:text:0".into(),
                translated_text: "Linea 1 Linea 2".into(),
            }],
        )
        .unwrap();

        assert_eq!(result.applied_entries, 0);
        assert_eq!(result.invalid_entries, vec!["Map001:1:0:text:0"]);
        assert!(!result.warnings.is_empty());
        assert!(project.files[0].entries[0].translation.is_empty());
    }

    #[test]
    fn test_apply_provider_responses_applies_valid_translations() {
        let mut project = make_project();
        let result = apply_provider_responses(
            &mut project,
            "data/Map001.json",
            &[TranslationResponse {
                entry_id: "Map001:1:0:text:0".into(),
                translated_text: "\\C[4]Linea 1\nLinea 2".into(),
            }],
        )
        .unwrap();

        assert_eq!(result.applied_entries, 1);
        assert!(result.invalid_entries.is_empty());
        assert_eq!(
            project.files[0].entries[0].translation,
            "\\C[4]Linea 1\nLinea 2"
        );
        assert_eq!(project.files[0].entries[0].status, EntryStatus::Draft);
    }

    #[test]
    fn test_apply_provider_responses_normalizes_escaped_line_breaks() {
        let mut project = make_project();
        let result = apply_provider_responses(
            &mut project,
            "data/Map001.json",
            &[TranslationResponse {
                entry_id: "Map001:1:0:text:0".into(),
                translated_text: "\\C[4]Linea 1\\nLinea 2".into(),
            }],
        )
        .unwrap();

        assert_eq!(result.applied_entries, 1);
        assert!(result.invalid_entries.is_empty());
        assert_eq!(
            project.files[0].entries[0].translation,
            "\\C[4]Linea 1\nLinea 2"
        );
    }
}

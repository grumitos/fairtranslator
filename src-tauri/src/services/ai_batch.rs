use serde_json::Value;
use std::collections::HashSet;

use crate::error::AppError;
use crate::models::{
    AiBatchApplyResult, AiBatchEntry, AiBatchExport, AiBatchTranslation, EntryStatus, Project,
    TranslationEntry,
};
use crate::services::persistence;

/// Default batch size when exporting entries for manual or AI-assisted translation.
pub const DEFAULT_AI_BATCH_MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiBatchResponseEnvelope {
    project_id: Option<String>,
    batch_id: Option<String>,
    translations: Vec<AiBatchTranslation>,
}

const INSTRUCTION_PATTERNS: &[(&str, &str)] = &[
    ("press ", "mentions an input action"),
    ("click ", "mentions an input action"),
    ("tap ", "mentions an input action"),
    ("hold ", "mentions an input action"),
    ("use the arrow", "describes control instructions"),
    ("arrow keys", "describes control instructions"),
    ("keyboard", "describes control instructions"),
    ("mouse", "describes control instructions"),
    ("gamepad", "describes control instructions"),
    ("touchscreen", "describes control instructions"),
    ("controls", "looks like controls/help text"),
    ("how to play", "looks like controls/help text"),
    ("tutorial", "looks like tutorial/help text"),
    ("instruction", "looks like instructions/help text"),
    ("save your game", "looks like operational guidance"),
    ("open the menu", "looks like operational guidance"),
    ("press z", "mentions a concrete keybinding"),
    ("press x", "mentions a concrete keybinding"),
    ("press enter", "mentions a concrete keybinding"),
    ("press esc", "mentions a concrete keybinding"),
];

pub fn instruction_reason_for(entry: &TranslationEntry) -> Option<String> {
    let normalized = entry.original.to_ascii_lowercase();
    for (needle, reason) in INSTRUCTION_PATTERNS {
        if normalized.contains(needle) {
            return Some((*reason).to_string());
        }
    }

    let context = entry.file_context.to_ascii_lowercase();
    if context.contains("tutorial") || context.contains("how to") {
        return Some("context suggests tutorial or help text".to_string());
    }

    None
}

fn selected_ai_batch_entries(
    project: &Project,
    file_paths: &[String],
    offset: usize,
    max_entries: Option<usize>,
) -> Vec<AiBatchEntry> {
    let limit = max_entries.unwrap_or(DEFAULT_AI_BATCH_MAX_ENTRIES).max(1);
    let mut entries: Vec<AiBatchEntry> = Vec::new();
    let mut skipped = 0usize;

    for file in &project.files {
        if !file_paths.is_empty() && !file_paths.contains(&file.relative_path) {
            continue;
        }

        for entry in &file.entries {
            let is_untranslated = entry.status == EntryStatus::Untranslated
                || (entry.translation.trim().is_empty() && entry.status != EntryStatus::Final);
            if !is_untranslated {
                continue;
            }

            if skipped < offset {
                skipped += 1;
                continue;
            }

            let instruction_reason = instruction_reason_for(entry);
            entries.push(AiBatchEntry {
                file_path: file.relative_path.clone(),
                entry_id: entry.id.clone(),
                file_context: entry.file_context.clone(),
                original: entry.original.clone(),
                current_translation: entry.translation.clone(),
                instruction_candidate: instruction_reason.is_some(),
                instruction_reason,
            });

            if entries.len() >= limit {
                return entries;
            }
        }
    }

    entries
}

fn total_selected_entries(project: &Project, file_paths: &[String]) -> usize {
    project
        .files
        .iter()
        .filter(|file| file_paths.is_empty() || file_paths.contains(&file.relative_path))
        .flat_map(|file| file.entries.iter())
        .filter(|entry| {
            entry.status == EntryStatus::Untranslated
                || (entry.translation.trim().is_empty() && entry.status != EntryStatus::Final)
        })
        .count()
}

fn build_ai_batch_prompt(
    project: &Project,
    batch_id: &str,
    entries: &[AiBatchEntry],
) -> Result<String, AppError> {
    let entries_json = serde_json::to_string_pretty(entries)?;
    Ok(format!(
        concat!(
            "Translate the following RPG Maker entries from {source_lang} to {target_lang}.\n",
            "Return ONLY valid JSON.\n",
            "Required output shape:\n",
            "{{\"projectId\": \"{project_id}\", \"batchId\": \"{batch_id}\", \"translations\": [{{\"filePath\": \"...\", \"entryId\": \"...\", \"translation\": \"...\", \"notes\": \"...\", \"skip\": false}}]}}\n",
            "Rules:\n",
            "- Preserve entry order.\n",
            "- Preserve control codes, placeholders, escape sequences and line breaks.\n",
            "- Do not invent IDs or file paths.\n",
            "- Echo the exact projectId and batchId shown above.\n",
            "- If an entry looks like instructions/help text and should stay excluded, set \"skip\": true and leave \"translation\" empty.\n",
            "- Keep \"notes\" short and optional.\n",
            "- Project: {project_name}\n",
            "- Engine: {engine:?}\n",
            "- Entries:\n",
            "{entries_json}\n"
        ),
        project_id = project.id,
        batch_id = batch_id,
        source_lang = project.source_lang,
        target_lang = project.target_lang,
        project_name = project.name,
        engine = project.engine,
        entries_json = entries_json
    ))
}

fn make_ai_batch_export(
    project: &Project,
    total_selected_entries: usize,
    entries: Vec<AiBatchEntry>,
) -> Result<AiBatchExport, AppError> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let prompt = build_ai_batch_prompt(project, &batch_id, &entries)?;

    Ok(AiBatchExport {
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        batch_id,
        engine: project.engine.clone(),
        source_lang: project.source_lang.clone(),
        target_lang: project.target_lang.clone(),
        total_selected_entries,
        exported_entries: entries.len(),
        entries,
        prompt,
    })
}

pub fn export_ai_batch(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
) -> Result<AiBatchExport, AppError> {
    let total = total_selected_entries(project, file_paths);
    let entries = selected_ai_batch_entries(project, file_paths, 0, max_entries);
    make_ai_batch_export(project, total, entries)
}

pub fn export_ai_batches(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
) -> Result<Vec<AiBatchExport>, AppError> {
    let total = total_selected_entries(project, file_paths);
    if total == 0 {
        return Ok(vec![make_ai_batch_export(project, 0, Vec::new())?]);
    }

    let chunk_size = max_entries.unwrap_or(DEFAULT_AI_BATCH_MAX_ENTRIES).max(1);
    let mut offset = 0usize;
    let mut batches = Vec::new();

    while offset < total {
        let entries = selected_ai_batch_entries(project, file_paths, offset, Some(chunk_size));
        if entries.is_empty() {
            break;
        }
        offset += entries.len();
        batches.push(make_ai_batch_export(project, total, entries)?);
    }

    Ok(batches)
}

fn strip_markdown_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }

    let without_open = trimmed
        .split_once('\n')
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);

    without_open
        .rsplit_once("```")
        .map(|(body, _)| body.trim())
        .unwrap_or(without_open.trim())
}

pub fn parse_ai_batch_response(
    raw_response: &str,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    let normalized = strip_markdown_fences(raw_response);
    let value: Value = serde_json::from_str(normalized)?;

    let translations = match value {
        Value::Array(items) => {
            if expected_project_id.is_some() || expected_batch_id.is_some() {
                return Err(AppError::Parse(
                    "AI batch response must be an object with matching 'projectId' and 'batchId'"
                        .to_string(),
                ));
            }
            serde_json::from_value(Value::Array(items))
                .map_err(|e| AppError::Parse(format!("Invalid AI batch response schema: {e}")))?
        }
        Value::Object(map) => {
            let envelope: AiBatchResponseEnvelope = serde_json::from_value(Value::Object(map))
                .map_err(|e| AppError::Parse(format!("Invalid AI batch response schema: {e}")))?;

            if let Some(expected) = expected_project_id {
                if envelope.project_id.as_deref() != Some(expected) {
                    return Err(AppError::Parse(
                        "AI batch response projectId does not match the current export batch"
                            .to_string(),
                    ));
                }
            }
            if let Some(expected) = expected_batch_id {
                if envelope.batch_id.as_deref() != Some(expected) {
                    return Err(AppError::Parse(
                        "AI batch response batchId does not match the current export batch"
                            .to_string(),
                    ));
                }
            }

            envelope.translations
        }
        _ => {
            return Err(AppError::Parse(
                "AI batch response must be a JSON array or an object with 'translations'".into(),
            ));
        }
    };

    Ok(translations)
}

fn merge_notes(existing: &str, incoming: Option<&str>) -> String {
    let Some(incoming) = incoming.map(str::trim).filter(|text| !text.is_empty()) else {
        return existing.to_string();
    };
    if existing.trim().is_empty() {
        return incoming.to_string();
    }
    if existing.contains(incoming) {
        return existing.to_string();
    }
    format!("{}\n{}", existing.trim_end(), incoming)
}

pub fn apply_ai_batch_updates(
    project: &mut Project,
    translations: &[AiBatchTranslation],
    mark_status: EntryStatus,
) -> AiBatchApplyResult {
    let mut result = AiBatchApplyResult {
        applied_entries: 0,
        skipped_entries: 0,
        missing_entries: Vec::new(),
        duplicate_entries: Vec::new(),
    };
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for update in translations {
        let dedupe_key = (update.file_path.clone(), update.entry_id.clone());
        if !seen.insert(dedupe_key.clone()) {
            result
                .duplicate_entries
                .push(format!("{}::{}", dedupe_key.0, dedupe_key.1));
            continue;
        }

        let Some(file) = project
            .files
            .iter_mut()
            .find(|file| file.relative_path == update.file_path)
        else {
            result
                .missing_entries
                .push(format!("{}::{}", update.file_path, update.entry_id));
            continue;
        };

        let Some(entry) = file
            .entries
            .iter_mut()
            .find(|entry| entry.id == update.entry_id)
        else {
            result
                .missing_entries
                .push(format!("{}::{}", update.file_path, update.entry_id));
            continue;
        };

        entry.notes = merge_notes(&entry.notes, update.notes.as_deref());

        if update.skip || update.translation.trim().is_empty() {
            result.skipped_entries += 1;
            continue;
        }

        entry.translation = update.translation.clone();
        entry.status = mark_status.clone();
        result.applied_entries += 1;
    }

    if result.applied_entries > 0 || result.skipped_entries > 0 {
        project.updated_at = persistence::now_iso8601();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, TranslationFile};

    fn make_test_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "AI Batch Test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\Test".into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![
                        TranslationEntry {
                            id: "Map001:event:1:page:1:list:0".into(),
                            file_context: "Map001 > Event #1 > page 1 > text".into(),
                            original: "Welcome to town.".into(),
                            translation: String::new(),
                            status: EntryStatus::Untranslated,
                            notes: String::new(),
                        },
                        TranslationEntry {
                            id: "Map001:event:1:page:1:list:1".into(),
                            file_context: "Map001 > Tutorial".into(),
                            original: "Press Z to confirm your choice.".into(),
                            translation: String::new(),
                            status: EntryStatus::Untranslated,
                            notes: String::new(),
                        },
                    ],
                },
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![TranslationEntry {
                        id: "System:gameTitle".into(),
                        file_context: "System > gameTitle".into(),
                        original: "Test Game".into(),
                        translation: "Juego de Prueba".into(),
                        status: EntryStatus::Final,
                        notes: String::new(),
                    }],
                },
            ],
            glossary: Vec::new(),
            created_at: "2026-03-29T00:00:00Z".into(),
            updated_at: "2026-03-29T00:00:00Z".into(),
        }
    }

    #[test]
    fn test_export_ai_batch_flags_instruction_candidates_and_builds_prompt() {
        let project = make_test_project();
        let batch = export_ai_batch(&project, &[], Some(10)).unwrap();

        assert_eq!(batch.total_selected_entries, 2);
        assert_eq!(batch.exported_entries, 2);
        assert_eq!(batch.entries.len(), 2);
        assert!(!batch.batch_id.is_empty());
        assert!(batch.prompt.contains("\"projectId\": \"project-1\""));
        assert!(batch
            .prompt
            .contains(&format!("\"batchId\": \"{}\"", batch.batch_id)));
        assert!(batch
            .prompt
            .contains("\"entryId\": \"Map001:event:1:page:1:list:0\""));

        let tutorial = batch
            .entries
            .iter()
            .find(|entry| entry.entry_id == "Map001:event:1:page:1:list:1")
            .unwrap();
        assert!(tutorial.instruction_candidate);
        assert!(!tutorial
            .instruction_reason
            .as_deref()
            .unwrap_or_default()
            .is_empty());
    }

    #[test]
    fn test_export_ai_batches_splits_large_projects() {
        let project = make_test_project();
        let batches = export_ai_batches(&project, &[], Some(1)).unwrap();

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].exported_entries, 1);
        assert_eq!(batches[1].exported_entries, 1);
        assert_eq!(batches[0].total_selected_entries, 2);
        assert_eq!(batches[1].total_selected_entries, 2);
        assert_ne!(batches[0].batch_id, batches[1].batch_id);
    }

    #[test]
    fn test_parse_ai_batch_response_accepts_wrapped_object() {
        let parsed = parse_ai_batch_response(
            r#"{
              "projectId": "project-1",
              "batchId": "batch-1",
              "translations": [
                {
                  "filePath": "data/Map001.json",
                  "entryId": "Map001:event:1:page:1:list:0",
                  "translation": "Bienvenido al pueblo.",
                  "notes": "npc intro",
                  "skip": false
                }
              ]
            }"#,
            Some("project-1"),
            Some("batch-1"),
        )
        .unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].file_path, "data/Map001.json");
        assert_eq!(parsed[0].entry_id, "Map001:event:1:page:1:list:0");
    }

    #[test]
    fn test_parse_ai_batch_response_rejects_mismatched_batch_id() {
        let error = parse_ai_batch_response(
            r#"{
              "projectId": "project-1",
              "batchId": "wrong-batch",
              "translations": []
            }"#,
            Some("project-1"),
            Some("batch-1"),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("batchId does not match the current export batch"));
    }

    #[test]
    fn test_apply_ai_batch_updates_translations_and_skips_marked_entries() {
        let mut project = make_test_project();
        let result = apply_ai_batch_updates(
            &mut project,
            &[
                AiBatchTranslation {
                    file_path: "data/Map001.json".into(),
                    entry_id: "Map001:event:1:page:1:list:0".into(),
                    translation: "Bienvenido al pueblo.".into(),
                    notes: Some("done".into()),
                    skip: false,
                },
                AiBatchTranslation {
                    file_path: "data/Map001.json".into(),
                    entry_id: "Map001:event:1:page:1:list:1".into(),
                    translation: String::new(),
                    notes: Some("instruction text".into()),
                    skip: true,
                },
            ],
            EntryStatus::Draft,
        );

        assert_eq!(result.applied_entries, 1);
        assert_eq!(result.skipped_entries, 1);
        assert!(result.missing_entries.is_empty());

        let file = project
            .files
            .iter()
            .find(|file| file.relative_path == "data/Map001.json")
            .unwrap();
        let translated = file
            .entries
            .iter()
            .find(|entry| entry.id == "Map001:event:1:page:1:list:0")
            .unwrap();
        assert_eq!(translated.translation, "Bienvenido al pueblo.");
        assert_eq!(translated.status, EntryStatus::Draft);
        assert!(translated.notes.contains("done"));

        let skipped = file
            .entries
            .iter()
            .find(|entry| entry.id == "Map001:event:1:page:1:list:1")
            .unwrap();
        assert!(skipped.translation.is_empty());
        assert!(skipped.notes.contains("instruction text"));
    }
}

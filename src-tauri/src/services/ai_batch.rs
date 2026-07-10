use std::collections::HashSet;

use crate::error::AppError;
use crate::models::{
    AiBatchApplyResult, AiBatchEntry, AiBatchExport, AiBatchResponseRef, AiBatchTranslation,
    EntryStatus, Project, TranslationEntry,
};
use crate::services::compact_prompt;
use crate::services::persistence;
use crate::services::provider::{
    approximate_token_count, TranslationBatchContext, TranslationRequest,
};
use crate::services::translation_integrity::{self, IntegrityMode};
use crate::services::translation_orchestrator;
use serde_json::json;

/// Default batch size when exporting entries for manual or AI-assisted translation.
pub const DEFAULT_AI_BATCH_MAX_ENTRIES: usize = 200;
const EXTERNAL_AI_PROMPT_WRAPPER_TOKEN_OVERHEAD: usize = 96;
const EXTERNAL_AI_RESPONSE_HEADROOM_TOKENS: usize = 2048;

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

fn total_selected_entries(project: &Project, file_paths: &[String]) -> usize {
    project
        .files
        .iter()
        .filter(|file| file_paths.is_empty() || file_paths.contains(&file.relative_path))
        .flat_map(|file| file.entries.iter())
        .filter(|entry| {
            !entry.original.trim().is_empty()
                && (entry.status == EntryStatus::Untranslated
                    || (entry.translation.trim().is_empty() && entry.status != EntryStatus::Final))
        })
        .count()
}

pub(crate) fn estimate_ai_batch_prompt_tokens(
    project: &Project,
    requests: &[TranslationRequest],
    context: &TranslationBatchContext,
) -> usize {
    let entries = requests
        .iter()
        .map(|request| AiBatchEntry {
            file_path: context.file_path.clone(),
            entry_id: request.entry_id.clone(),
            file_context: request.file_context.clone(),
            original: request.text.clone(),
            current_translation: String::new(),
            instruction_candidate: false,
            instruction_reason: None,
        })
        .collect::<Vec<_>>();
    let bundle =
        compact_prompt::build_external_ai_bundle(project, "budget-estimate", &entries, context)
            .expect("serializing compact external AI prompt should not fail");
    let response_map = bundle.response_map();
    let translation_sheet =
        translation_sheet_for(project, "budget-estimate", &entries, context, &response_map)
            .expect("serializing external AI translation sheet should not fail");
    let prompt = render_external_ai_prompt(&bundle.system_prompt, &translation_sheet);

    approximate_token_count(&prompt)
        + EXTERNAL_AI_PROMPT_WRAPPER_TOKEN_OVERHEAD
        + EXTERNAL_AI_RESPONSE_HEADROOM_TOKENS
}

fn entries_for_chunk(
    project: &Project,
    chunk: &translation_orchestrator::PlannedTranslationChunk,
) -> Vec<AiBatchEntry> {
    let Some(file) = project
        .files
        .iter()
        .find(|file| file.relative_path == chunk.relative_path)
    else {
        return Vec::new();
    };

    chunk
        .entries
        .iter()
        .map(|request| {
            let project_entry = file
                .entries
                .iter()
                .find(|entry| entry.id == request.entry_id);
            let instruction_reason = project_entry.and_then(instruction_reason_for);

            AiBatchEntry {
                file_path: chunk.relative_path.clone(),
                entry_id: request.entry_id.clone(),
                file_context: request.file_context.clone(),
                original: request.text.clone(),
                current_translation: project_entry
                    .map(|entry| entry.translation.clone())
                    .unwrap_or_default(),
                instruction_candidate: instruction_reason.is_some(),
                instruction_reason,
            }
        })
        .collect()
}

fn make_ai_batch_export(
    project: &Project,
    total_selected_entries: usize,
    chunk: Option<translation_orchestrator::PlannedTranslationChunk>,
) -> Result<AiBatchExport, AppError> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let (entries, context) = if let Some(chunk) = chunk {
        let entries = entries_for_chunk(project, &chunk);
        (entries, chunk.context)
    } else {
        (
            Vec::new(),
            TranslationBatchContext {
                project_name: project.name.clone(),
                ..TranslationBatchContext::default()
            },
        )
    };
    let bundle = compact_prompt::build_external_ai_bundle(project, &batch_id, &entries, &context)?;
    let response_map = bundle.response_map();
    let translation_sheet =
        translation_sheet_for(project, &batch_id, &entries, &context, &response_map)?;
    let prompt = render_external_ai_prompt(&bundle.system_prompt, &translation_sheet);

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
        response_map,
        prompt,
        translation_sheet,
    })
}

fn render_external_ai_prompt(system_prompt: &str, translation_sheet: &str) -> String {
    format!(
        "{system_prompt}\n\nEditable translation sheet JSON:\n{translation_sheet}\n\nReturn ONLY the same JSON object with each translations[].t filled. Keep p, b and k unchanged. Do not rewrite source text in translations[].o; it is reference-only."
    )
}

fn translation_sheet_for(
    project: &Project,
    batch_id: &str,
    entries: &[AiBatchEntry],
    context: &TranslationBatchContext,
    response_map: &[AiBatchResponseRef],
) -> Result<String, AppError> {
    let rows = entries
        .iter()
        .zip(response_map.iter())
        .map(|(entry, response_ref)| {
            json!({
                "k": response_ref.local_id,
                "c": &entry.file_context,
                "o": &entry.original,
                "t": &entry.current_translation,
                "n": "",
                "s": false
            })
        })
        .collect::<Vec<_>>();

    let sheet = json!({
        "p": &project.id,
        "b": batch_id,
        "sourceLanguage": &project.source_lang,
        "targetLanguage": &project.target_lang,
        "context": {
            "project": &project.name,
            "file": &context.file_path,
            "kind": &context.file_kind,
            "chunk": context.chunk_index,
            "totalChunks": context.total_chunks,
            "styleGuide": &context.style_guide,
            "glossary": project.glossary.iter().map(|term| {
                json!({
                    "source": &term.source,
                    "target": &term.target
                })
            }).collect::<Vec<_>>()
        },
        "instructions": [
            "Translate only translations[].o into translations[].t.",
            "Leave p, b, k, c and o unchanged if you include them in the response.",
            "Use s=true only for entries that should stay skipped; keep n short."
        ],
        "translations": rows
    });

    serde_json::to_string_pretty(&sheet).map_err(AppError::from)
}

pub fn export_ai_batch(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
) -> Result<AiBatchExport, AppError> {
    let total = total_selected_entries(project, file_paths);
    let chunks = translation_orchestrator::build_external_ai_plan(project, file_paths, max_entries);
    translation_orchestrator::ensure_external_ai_plan_within_budget(project, &chunks)?;
    let first_chunk = chunks.into_iter().next();
    make_ai_batch_export(project, total, first_chunk)
}

pub fn export_ai_batches(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
) -> Result<Vec<AiBatchExport>, AppError> {
    let total = total_selected_entries(project, file_paths);
    let chunks = translation_orchestrator::build_external_ai_plan(project, file_paths, max_entries);
    translation_orchestrator::ensure_external_ai_plan_within_budget(project, &chunks)?;
    if chunks.is_empty() {
        return Ok(vec![make_ai_batch_export(project, 0, None)?]);
    }

    chunks
        .into_iter()
        .map(|chunk| make_ai_batch_export(project, total, Some(chunk)))
        .collect()
}

pub fn parse_ai_batch_response(
    raw_response: &str,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    parse_ai_batch_response_with_refs(raw_response, expected_project_id, expected_batch_id, None)
}

pub fn parse_ai_batch_response_with_refs(
    raw_response: &str,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
    response_map: Option<&[AiBatchResponseRef]>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    compact_prompt::parse_external_ai_response(
        raw_response,
        expected_project_id,
        expected_batch_id,
        response_map,
    )
}

pub fn validate_ai_batch_response_complete(
    response_map: &[AiBatchResponseRef],
    translations: &[AiBatchTranslation],
) -> Result<(), AppError> {
    if translations.len() != response_map.len() {
        return Err(AppError::Parse(format!(
            "Respuesta incompleta: se esperaban {} entradas y se recibieron {}",
            response_map.len(),
            translations.len()
        )));
    }

    let mut expected_entries = HashSet::with_capacity(response_map.len());
    for (index, entry) in response_map.iter().enumerate() {
        let expected_local_id = u16::try_from(index + 1).map_err(|_| {
            AppError::Parse("El mapa de respuesta excede el rango de IDs locales".into())
        })?;
        if entry.local_id != expected_local_id {
            return Err(AppError::Parse(
                "El mapa de respuesta contiene IDs locales inválidos o duplicados".into(),
            ));
        }
        if entry.file_path.trim().is_empty() || entry.entry_id.trim().is_empty() {
            return Err(AppError::Parse(
                "El mapa de respuesta contiene una ruta o ID de entrada vacío".into(),
            ));
        }
        if !expected_entries.insert((entry.file_path.as_str(), entry.entry_id.as_str())) {
            return Err(AppError::Parse(
                "El mapa de respuesta contiene una entrada duplicada".into(),
            ));
        }
    }

    let mut received_entries = HashSet::with_capacity(translations.len());
    for translation in translations {
        if !received_entries.insert((
            translation.file_path.as_str(),
            translation.entry_id.as_str(),
        )) {
            return Err(AppError::Parse(
                "La respuesta contiene una entrada duplicada".into(),
            ));
        }
        if !translation.skip && translation.translation.trim().is_empty() {
            return Err(AppError::Parse(
                "Respuesta incompleta: una entrada no tiene translation ni skip=true".into(),
            ));
        }
    }

    if expected_entries != received_entries {
        let missing = expected_entries.difference(&received_entries).count();
        let unexpected = received_entries.difference(&expected_entries).count();
        return Err(AppError::Parse(format!(
            "Los IDs de la respuesta no coinciden con el mapa: faltantes={missing}, inesperados={unexpected}"
        )));
    }

    Ok(())
}

pub(crate) fn ensure_ai_batch_project_matches(
    current_project_id: &str,
    expected_project_id: &str,
) -> Result<(), AppError> {
    if current_project_id == expected_project_id {
        Ok(())
    } else {
        Err(AppError::Project(
            "El proyecto abierto no coincide con el lote de IA exportado".into(),
        ))
    }
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

fn push_warning(warnings: &mut Vec<String>, message: &str) {
    const MAX_WARNINGS: usize = 8;
    if warnings.len() < MAX_WARNINGS && !warnings.iter().any(|existing| existing == message) {
        warnings.push(message.to_string());
    }
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
        invalid_entries: Vec::new(),
        warnings: Vec::new(),
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

        if !translation_orchestrator::entry_is_pending_translation(entry) {
            result.skipped_entries += 1;
            push_warning(
                &mut result.warnings,
                &format!(
                    "{}::{} se omitió porque dejó de estar pendiente después de exportar el lote.",
                    update.file_path, update.entry_id
                ),
            );
            continue;
        }

        let merged_notes = merge_notes(&entry.notes, update.notes.as_deref());
        let normalized_translation =
            translation_integrity::normalize_translation_for_apply(entry, &update.translation);

        if update.skip || normalized_translation.trim().is_empty() {
            entry.notes = merged_notes;
            result.skipped_entries += 1;
            continue;
        }

        let validation = translation_integrity::validate_translation(
            &update.file_path,
            entry,
            &normalized_translation,
            IntegrityMode::AutomatedApply,
        );
        if validation.has_errors() {
            result
                .invalid_entries
                .push(format!("{}::{}", update.file_path, update.entry_id));
            for message in validation.errors.iter().chain(validation.warnings.iter()) {
                push_warning(&mut result.warnings, message);
            }
            continue;
        }

        for message in &validation.warnings {
            push_warning(&mut result.warnings, message);
        }
        entry.notes = merged_notes;
        entry.translation = normalized_translation;
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
            style_guide: Vec::new(),
            created_at: "2026-03-29T00:00:00Z".into(),
            updated_at: "2026-03-29T00:00:00Z".into(),
        }
    }

    fn make_oversized_export_project() -> Project {
        let mut project = make_test_project();
        project.files[0].entries[0].original = "A".repeat(18_000);
        project.glossary = (0..320)
            .map(|index| crate::models::GlossaryTerm {
                source: format!("VeryLongSourceTerm{index}_{}", "x".repeat(72)),
                target: format!("VeryLongTargetTerm{index}_{}", "y".repeat(72)),
                case_sensitive: true,
            })
            .collect();
        project.style_guide = (0..24)
            .map(|index| format!("Long export rule {index}: {}", "z".repeat(320)))
            .collect();
        project
    }

    #[test]
    fn test_export_ai_batch_flags_instruction_candidates_and_builds_prompt() {
        let mut project = make_test_project();
        project.style_guide = vec!["Keep the sibling tone natural in Spanish.".into()];
        project.glossary = vec![crate::models::GlossaryTerm {
            source: "Onee-chan".into(),
            target: "Onee-chan".into(),
            case_sensitive: true,
        }];
        let batch = export_ai_batch(&project, &[], Some(10)).unwrap();

        assert_eq!(batch.total_selected_entries, 2);
        assert_eq!(batch.exported_entries, 2);
        assert_eq!(batch.entries.len(), 2);
        assert_eq!(batch.response_map.len(), 2);
        assert!(!batch.batch_id.is_empty());
        assert!(batch.prompt.contains("\"p\": \"project-1\""));
        assert!(batch
            .prompt
            .contains(&format!("\"b\": \"{}\"", batch.batch_id)));
        assert!(batch.prompt.contains("\"k\": 1"));
        assert!(batch.prompt.contains("\"o\": \"Welcome to town.\""));
        assert!(batch.prompt.contains("\"t\": \"\""));
        assert!(batch.translation_sheet.contains("\"translations\""));
        assert!(batch
            .translation_sheet
            .contains("\"o\": \"Welcome to town.\""));
        assert!(batch
            .prompt
            .contains("Keep the sibling tone natural in Spanish."));
        assert!(batch.prompt.contains("\"source\": \"Onee-chan\""));
        assert!(batch.prompt.contains("\"project\": \"AI Batch Test\""));
        assert!(batch.prompt.contains("\"file\": \"data/Map001.json\""));

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
    fn test_export_ai_batches_trim_context_for_prompt_budget() {
        let mut project = make_test_project();
        project.style_guide = (1..=96)
            .map(|index| format!("Long external AI rule {index}: {}", "x".repeat(720)))
            .collect();

        let batches = export_ai_batches(&project, &[], Some(10)).unwrap();

        assert_eq!(batches.len(), 1);
        assert!(
            !batches[0].prompt.contains("Long external AI rule 96"),
            "external AI export should trim oversized context instead of emitting an over-budget prompt"
        );
    }

    #[test]
    fn test_export_ai_batch_rejects_unshrinkable_oversized_prompt() {
        let project = make_oversized_export_project();

        let error = export_ai_batch(&project, &[], Some(10)).unwrap_err();

        assert!(error.to_string().contains("External AI export chunk"));
        assert!(error
            .to_string()
            .contains("exceeds the estimated prompt budget"));
    }

    #[test]
    fn test_export_ai_batch_ignores_blank_source_entries() {
        let mut project = make_test_project();
        project.files[0].entries.push(TranslationEntry {
            id: "Map001:event:1:page:1:list:blank".into(),
            file_context: "Map001 > Event 1 > Show Text".into(),
            original: String::new(),
            translation: String::new(),
            status: EntryStatus::Untranslated,
            notes: String::new(),
        });

        let batch = export_ai_batch(&project, &[], Some(10)).unwrap();

        assert_eq!(batch.total_selected_entries, 2);
        assert_eq!(batch.exported_entries, 2);
        assert_eq!(batch.entries.len(), 2);
        assert!(!batch
            .entries
            .iter()
            .any(|entry| entry.entry_id == "Map001:event:1:page:1:list:blank"));
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
    fn test_parse_ai_batch_response_accepts_clean_translation_sheet() {
        let batch = export_ai_batch(&make_test_project(), &[], Some(10)).unwrap();
        let mut sheet: serde_json::Value = serde_json::from_str(&batch.translation_sheet).unwrap();
        sheet["translations"][0]["t"] = json!("Bienvenido al pueblo.");
        sheet["translations"][1]["t"] = json!("Pulsa Z para confirmar.");

        let parsed = parse_ai_batch_response_with_refs(
            &serde_json::to_string(&sheet).unwrap(),
            Some("project-1"),
            Some(&batch.batch_id),
            Some(&batch.response_map),
        )
        .unwrap();

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].file_path, "data/Map001.json");
        assert_eq!(parsed[0].entry_id, "Map001:event:1:page:1:list:0");
        assert_eq!(parsed[0].translation, "Bienvenido al pueblo.");
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
            .contains("batchId de la respuesta de IA no coincide"));
    }

    #[test]
    fn test_validate_ai_batch_response_complete_rejects_partial_response() {
        let response_map = vec![
            AiBatchResponseRef {
                local_id: 1,
                file_path: "data/Map001.json".into(),
                entry_id: "entry-1".into(),
            },
            AiBatchResponseRef {
                local_id: 2,
                file_path: "data/Map001.json".into(),
                entry_id: "entry-2".into(),
            },
        ];
        let translations = vec![AiBatchTranslation {
            file_path: "data/Map001.json".into(),
            entry_id: "entry-1".into(),
            translation: "Hola".into(),
            notes: None,
            skip: false,
        }];

        let error = validate_ai_batch_response_complete(&response_map, &translations).unwrap_err();

        assert!(error.to_string().contains("esperaban 2"));
    }

    #[test]
    fn test_validate_ai_batch_response_complete_rejects_duplicate_entries() {
        let response_map = vec![
            AiBatchResponseRef {
                local_id: 1,
                file_path: "data/Map001.json".into(),
                entry_id: "entry-1".into(),
            },
            AiBatchResponseRef {
                local_id: 2,
                file_path: "data/Map001.json".into(),
                entry_id: "entry-2".into(),
            },
        ];
        let duplicate = AiBatchTranslation {
            file_path: "data/Map001.json".into(),
            entry_id: "entry-1".into(),
            translation: "Hola".into(),
            notes: None,
            skip: false,
        };

        let error =
            validate_ai_batch_response_complete(&response_map, &[duplicate.clone(), duplicate])
                .unwrap_err();

        assert!(error.to_string().contains("duplicada"));
    }

    #[test]
    fn test_validate_ai_batch_response_complete_requires_text_or_explicit_skip() {
        let response_map = vec![AiBatchResponseRef {
            local_id: 1,
            file_path: "data/Map001.json".into(),
            entry_id: "entry-1".into(),
        }];
        let mut translation = AiBatchTranslation {
            file_path: "data/Map001.json".into(),
            entry_id: "entry-1".into(),
            translation: "  ".into(),
            notes: None,
            skip: false,
        };

        let error =
            validate_ai_batch_response_complete(&response_map, &[translation.clone()]).unwrap_err();
        assert!(error.to_string().contains("translation ni skip"));

        translation.skip = true;
        validate_ai_batch_response_complete(&response_map, &[translation]).unwrap();
    }

    #[test]
    fn test_ensure_ai_batch_project_matches_rejects_changed_project() {
        let error = ensure_ai_batch_project_matches("project-2", "project-1").unwrap_err();

        assert!(error.to_string().contains("proyecto abierto"));
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
        assert!(result.invalid_entries.is_empty());
        assert!(result.warnings.is_empty());

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

    #[test]
    fn test_apply_ai_batch_updates_blocks_integrity_breaks() {
        let mut project = make_test_project();
        project.files[0].entries[0].original = "\\C[2]Welcome\nHome".into();

        let result = apply_ai_batch_updates(
            &mut project,
            &[AiBatchTranslation {
                file_path: "data/Map001.json".into(),
                entry_id: "Map001:event:1:page:1:list:0".into(),
                translation: "Bienvenido a casa".into(),
                notes: None,
                skip: false,
            }],
            EntryStatus::Draft,
        );

        assert_eq!(result.applied_entries, 0);
        assert_eq!(result.invalid_entries.len(), 1);
        assert!(!result.warnings.is_empty());
        let translated = project
            .files
            .iter()
            .find(|file| file.relative_path == "data/Map001.json")
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == "Map001:event:1:page:1:list:0")
            .unwrap();
        assert!(translated.translation.is_empty());
    }

    #[test]
    fn test_apply_ai_batch_updates_normalizes_escaped_line_breaks() {
        let mut project = make_test_project();
        project.files[0].entries[0].original = "\\C[2]Welcome\nHome".into();

        let result = apply_ai_batch_updates(
            &mut project,
            &[AiBatchTranslation {
                file_path: "data/Map001.json".into(),
                entry_id: "Map001:event:1:page:1:list:0".into(),
                translation: "\\C[2]Bienvenida\\nA casa".into(),
                notes: None,
                skip: false,
            }],
            EntryStatus::Draft,
        );

        assert_eq!(result.applied_entries, 1);
        assert!(result.invalid_entries.is_empty());
        let translated = project
            .files
            .iter()
            .find(|file| file.relative_path == "data/Map001.json")
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == "Map001:event:1:page:1:list:0")
            .unwrap();
        assert_eq!(translated.translation, "\\C[2]Bienvenida\nA casa");
    }

    #[test]
    fn test_apply_ai_batch_updates_preserves_later_manual_edit() {
        let mut project = make_test_project();
        project.files[0].entries[0].translation = "Edición manual".into();
        project.files[0].entries[0].status = EntryStatus::Reviewed;
        project.files[0].entries[0].notes = "Nota manual".into();

        let result = apply_ai_batch_updates(
            &mut project,
            &[AiBatchTranslation {
                file_path: "data/Map001.json".into(),
                entry_id: "Map001:event:1:page:1:list:0".into(),
                translation: "Respuesta tardía".into(),
                notes: Some("Nota de IA".into()),
                skip: false,
            }],
            EntryStatus::Draft,
        );

        assert_eq!(result.applied_entries, 0);
        assert_eq!(result.skipped_entries, 1);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("dejó de estar pendiente")));
        assert_eq!(project.files[0].entries[0].translation, "Edición manual");
        assert_eq!(project.files[0].entries[0].status, EntryStatus::Reviewed);
        assert_eq!(project.files[0].entries[0].notes, "Nota manual");
    }
}

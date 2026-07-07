use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::AppError;
use crate::models::{AiBatchEntry, AiBatchResponseRef, AiBatchTranslation, GlossaryTerm, Project};
use crate::services::provider::{TranslationBatchContext, TranslationRequest, TranslationResponse};
use crate::services::translation_integrity::collect_special_token_spans;
use crate::services::translation_text::{is_japanese_language, is_spanish_language};

#[derive(Debug, Clone)]
pub struct CompactPromptBundle {
    pub system_prompt: String,
    pub user_payload: String,
    pub response_schema: Option<serde_json::Value>,
    pub entry_refs: Vec<ChunkEntryRef>,
}

impl CompactPromptBundle {
    pub fn render_single_prompt(&self) -> String {
        format!(
            "{}\n\nCompact JSON payload:\n{}",
            self.system_prompt, self.user_payload
        )
    }

    pub fn response_map(&self) -> Vec<AiBatchResponseRef> {
        self.entry_refs
            .iter()
            .map(|entry| AiBatchResponseRef {
                local_id: entry.local_id,
                file_path: entry.file_path.clone(),
                entry_id: entry.entry_id.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ChunkEntryRef {
    pub local_id: u16,
    pub file_path: String,
    pub entry_id: String,
    pub original_is_multiline: bool,
    pub masks: Vec<MaskReplacement>,
}

#[derive(Debug, Clone)]
pub struct MaskReplacement {
    pub placeholder: String,
    pub original: String,
}

#[derive(Debug, Serialize)]
pub struct CompactPromptPayload {
    #[serde(rename = "m")]
    pub meta: CompactPromptMeta,
    #[serde(rename = "c")]
    pub context: CompactPromptContext,
    #[serde(rename = "e")]
    pub entries: Vec<CompactPromptEntry>,
}

#[derive(Debug, Serialize)]
pub struct CompactPromptMeta {
    #[serde(rename = "sl")]
    pub source_lang: String,
    #[serde(rename = "tl")]
    pub target_lang: String,
    #[serde(rename = "fp")]
    pub file_path: String,
    #[serde(rename = "fk")]
    pub file_kind: String,
    #[serde(rename = "ci")]
    pub chunk_index: usize,
    #[serde(rename = "tc")]
    pub total_chunks: usize,
    #[serde(rename = "pn", skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(rename = "pid", skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(rename = "bid", skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(rename = "en", skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompactPromptContext {
    #[serde(rename = "sg", skip_serializing_if = "Vec::is_empty")]
    pub style_guide: Vec<String>,
    #[serde(rename = "gl", skip_serializing_if = "Vec::is_empty")]
    pub glossary: Vec<CompactGlossaryPair>,
    #[serde(rename = "th", skip_serializing_if = "Vec::is_empty")]
    pub term_hints: Vec<CompactTermHint>,
    #[serde(rename = "ex", skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<CompactExample>,
    #[serde(rename = "fh", skip_serializing_if = "Vec::is_empty")]
    pub file_hints: Vec<String>,
    #[serde(rename = "pv", skip_serializing_if = "Vec::is_empty")]
    pub previous_hints: Vec<String>,
    #[serde(rename = "nx", skip_serializing_if = "Vec::is_empty")]
    pub next_hints: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CompactGlossaryPair {
    #[serde(rename = "s")]
    pub source: String,
    #[serde(rename = "t")]
    pub target: String,
}

#[derive(Debug, Serialize)]
pub struct CompactTermHint {
    #[serde(rename = "s")]
    pub source: String,
    #[serde(rename = "t", skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(rename = "n")]
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct CompactExample {
    #[serde(rename = "c")]
    pub context: String,
    #[serde(rename = "o")]
    pub original: String,
    #[serde(rename = "t")]
    pub translation: String,
}

#[derive(Debug, Serialize)]
pub struct CompactPromptEntry {
    #[serde(rename = "k")]
    pub local_id: u16,
    #[serde(rename = "c")]
    pub context: String,
    #[serde(rename = "t")]
    pub text: String,
    #[serde(rename = "oc")]
    pub original_visible_chars: usize,
    #[serde(rename = "ol")]
    pub original_line_count: usize,
    #[serde(rename = "tb")]
    pub target_visible_budget: usize,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(rename = "ct", skip_serializing_if = "Option::is_none")]
    pub current_translation: Option<String>,
    #[serde(rename = "ic", skip_serializing_if = "Option::is_none")]
    pub instruction_candidate: Option<bool>,
    #[serde(rename = "ir", skip_serializing_if = "Option::is_none")]
    pub instruction_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompactResponseEnvelope {
    #[serde(rename = "p", default)]
    pub project_id: Option<String>,
    #[serde(rename = "b", default)]
    pub batch_id: Option<String>,
    pub translations: Vec<CompactResponseItem>,
}

#[derive(Debug, Deserialize)]
pub struct CompactResponseItem {
    #[serde(rename = "k")]
    pub local_id: u16,
    #[serde(rename = "t", default)]
    pub translated_text: String,
    #[serde(rename = "n", default)]
    pub notes: Option<String>,
    #[serde(rename = "s", default)]
    pub skip: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProviderEnvelope {
    translations: Vec<LegacyProviderItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProviderItem {
    entry_id: String,
    translated_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyExternalEnvelope {
    project_id: Option<String>,
    batch_id: Option<String>,
    translations: Vec<AiBatchTranslation>,
}

#[derive(Debug, Clone)]
pub struct ProviderBundleOptions {
    pub source_lang: String,
    pub target_lang: String,
}

pub fn build_provider_bundle(
    source_lang: &str,
    target_lang: &str,
    requests: &[TranslationRequest],
    glossary: &[GlossaryTerm],
    context: &TranslationBatchContext,
) -> Result<CompactPromptBundle, AppError> {
    let options = ProviderBundleOptions {
        source_lang: source_lang.to_string(),
        target_lang: target_lang.to_string(),
    };
    build_bundle(
        &options,
        Some((glossary, None)),
        None,
        requests
            .iter()
            .map(|request| CompactEntrySource {
                file_path: context.file_path.as_str(),
                entry_id: request.entry_id.as_str(),
                file_context: request.file_context.as_str(),
                original: request.text.as_str(),
                notes: Some(request.notes.as_str()),
                current_translation: None,
                instruction_candidate: None,
                instruction_reason: None,
            })
            .collect::<Vec<_>>()
            .as_slice(),
        context,
        ProviderKind::InApp,
    )
}

pub fn build_external_ai_bundle(
    project: &Project,
    batch_id: &str,
    entries: &[AiBatchEntry],
    context: &TranslationBatchContext,
) -> Result<CompactPromptBundle, AppError> {
    let options = ProviderBundleOptions {
        source_lang: project.source_lang.clone(),
        target_lang: project.target_lang.clone(),
    };
    build_bundle(
        &options,
        Some((
            &project.glossary,
            Some((
                project.id.as_str(),
                batch_id,
                format!("{:?}", project.engine),
            )),
        )),
        Some(project.name.as_str()),
        entries
            .iter()
            .map(|entry| CompactEntrySource {
                file_path: entry.file_path.as_str(),
                entry_id: entry.entry_id.as_str(),
                file_context: entry.file_context.as_str(),
                original: entry.original.as_str(),
                notes: None,
                current_translation: Some(entry.current_translation.as_str()),
                instruction_candidate: Some(entry.instruction_candidate),
                instruction_reason: entry.instruction_reason.as_deref(),
            })
            .collect::<Vec<_>>()
            .as_slice(),
        context,
        ProviderKind::ExternalAi,
    )
}

pub fn provider_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "translations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "k": { "type": "integer" },
                        "t": { "type": "string" }
                    },
                    "required": ["k", "t"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["translations"],
        "additionalProperties": false
    })
}

pub fn parse_provider_response(
    raw_response: &str,
    entry_refs: &[ChunkEntryRef],
) -> Result<Vec<TranslationResponse>, AppError> {
    let normalized = normalize_json_response(raw_response);

    if let Ok(parsed) = serde_json::from_str::<CompactResponseEnvelope>(&normalized) {
        return parse_compact_provider_response(parsed, entry_refs);
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&normalized) {
        if let Ok(parsed) = serde_json::from_value::<CompactResponseEnvelope>(value) {
            return parse_compact_provider_response(parsed, entry_refs);
        }
    }
    let repaired = repair_redundant_translation_value(&normalized);
    if repaired != normalized {
        if let Ok(parsed) = serde_json::from_str::<CompactResponseEnvelope>(&repaired) {
            return parse_compact_provider_response(parsed, entry_refs);
        }
    }

    let legacy: LegacyProviderEnvelope = serde_json::from_str(&normalized).map_err(|error| {
        AppError::Api(format!(
            "No se pudo leer la respuesta del proveedor: {error}"
        ))
    })?;
    parse_legacy_provider_response(legacy, entry_refs)
}

pub fn parse_external_ai_response(
    raw_response: &str,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
    entry_refs: Option<&[AiBatchResponseRef]>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    let normalized = normalize_json_response(raw_response);
    let value: serde_json::Value = serde_json::from_str(&normalized)?;

    match value {
        serde_json::Value::Array(items) => {
            if expected_project_id.is_some() || expected_batch_id.is_some() {
                return Err(AppError::Parse(
                    "La respuesta de IA debe incluir projectId/batchId cuando el lote exportado los define"
                        .into(),
                ));
            }
            serde_json::from_value(serde_json::Value::Array(items)).map_err(|error| {
                AppError::Parse(format!(
                    "El esquema de la respuesta de IA no es válido: {error}"
                ))
            })
        }
        serde_json::Value::Object(map) => {
            let object = serde_json::Value::Object(map.clone());
            let looks_legacy = map.contains_key("projectId")
                || map.contains_key("batchId")
                || map
                    .get("translations")
                    .and_then(|translations| translations.as_array())
                    .and_then(|translations| translations.first())
                    .and_then(|item| item.as_object())
                    .map(|item| {
                        item.contains_key("filePath")
                            || item.contains_key("entryId")
                            || item.contains_key("translation")
                    })
                    .unwrap_or(false);

            if !looks_legacy {
                if let Ok(envelope) =
                    serde_json::from_value::<CompactResponseEnvelope>(object.clone())
                {
                    validate_envelope_ids(
                        envelope.project_id.as_deref(),
                        envelope.batch_id.as_deref(),
                        expected_project_id,
                        expected_batch_id,
                    )?;
                    return parse_compact_external_response(envelope, entry_refs);
                }
            }

            let envelope: LegacyExternalEnvelope =
                serde_json::from_value(object.clone()).map_err(|error| {
                    AppError::Parse(format!(
                        "El esquema de la respuesta de IA no es válido: {error}"
                    ))
                })?;
            validate_envelope_ids(
                envelope.project_id.as_deref(),
                envelope.batch_id.as_deref(),
                expected_project_id,
                expected_batch_id,
            )?;
            Ok(envelope.translations)
        }
        _ => Err(AppError::Parse(
            "La respuesta del lote para IA debe ser un arreglo JSON o un objeto con 'translations'"
                .into(),
        )),
    }
}

fn validate_envelope_ids(
    actual_project_id: Option<&str>,
    actual_batch_id: Option<&str>,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
) -> Result<(), AppError> {
    if let Some(expected) = expected_project_id {
        if actual_project_id != Some(expected) {
            return Err(AppError::Parse(
                "El projectId de la respuesta de IA no coincide con el lote exportado".into(),
            ));
        }
    }
    if let Some(expected) = expected_batch_id {
        if actual_batch_id != Some(expected) {
            return Err(AppError::Parse(
                "El batchId de la respuesta de IA no coincide con el lote exportado".into(),
            ));
        }
    }
    Ok(())
}

fn parse_compact_provider_response(
    parsed: CompactResponseEnvelope,
    entry_refs: &[ChunkEntryRef],
) -> Result<Vec<TranslationResponse>, AppError> {
    if parsed.translations.len() != entry_refs.len() {
        return Err(AppError::Api(format!(
            "El proveedor devolvió {} traducciones, pero se esperaban {}",
            parsed.translations.len(),
            entry_refs.len()
        )));
    }

    let refs_by_id = entry_refs
        .iter()
        .map(|entry| (entry.local_id, entry))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut by_entry = HashMap::with_capacity(entry_refs.len());

    for translation in parsed.translations {
        let entry_ref = refs_by_id.get(&translation.local_id).ok_or_else(|| {
            AppError::Api(format!(
                "El proveedor devolvió un ID local inesperado: {}",
                translation.local_id
            ))
        })?;
        if !seen.insert(translation.local_id) {
            return Err(AppError::Api(format!(
                "El proveedor devolvió un ID local duplicado: {}",
                translation.local_id
            )));
        }

        by_entry.insert(
            entry_ref.entry_id.clone(),
            finalize_model_text(&translation.translated_text, entry_ref),
        );
    }

    entry_refs
        .iter()
        .map(|entry| {
            let translated_text = by_entry.remove(&entry.entry_id).ok_or_else(|| {
                AppError::Api(format!(
                    "El proveedor omitió la traducción de la entrada {}",
                    entry.entry_id
                ))
            })?;
            Ok(TranslationResponse {
                entry_id: entry.entry_id.clone(),
                translated_text,
            })
        })
        .collect()
}

fn parse_legacy_provider_response(
    legacy: LegacyProviderEnvelope,
    entry_refs: &[ChunkEntryRef],
) -> Result<Vec<TranslationResponse>, AppError> {
    if legacy.translations.len() != entry_refs.len() {
        return Err(AppError::Api(format!(
            "El proveedor devolvió {} traducciones, pero se esperaban {}",
            legacy.translations.len(),
            entry_refs.len()
        )));
    }

    let expected_ids = entry_refs
        .iter()
        .map(|entry| entry.entry_id.as_str())
        .collect::<HashSet<_>>();
    let refs_by_id = entry_refs
        .iter()
        .map(|entry| (entry.entry_id.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let mut seen_ids = HashSet::new();
    let mut by_id = HashMap::with_capacity(entry_refs.len());

    for translation in legacy.translations {
        if !expected_ids.contains(translation.entry_id.as_str()) {
            return Err(AppError::Api(format!(
                "El proveedor devolvió un ID de entrada inesperado: {}",
                translation.entry_id
            )));
        }
        if !seen_ids.insert(translation.entry_id.clone()) {
            return Err(AppError::Api(format!(
                "El proveedor devolvió un ID de entrada duplicado: {}",
                translation.entry_id
            )));
        }

        let entry_ref = refs_by_id
            .get(translation.entry_id.as_str())
            .expect("validated entry id must exist");
        by_id.insert(
            translation.entry_id,
            finalize_model_text(&translation.translated_text, entry_ref),
        );
    }

    entry_refs
        .iter()
        .map(|entry| {
            let translated_text = by_id.remove(&entry.entry_id).ok_or_else(|| {
                AppError::Api(format!(
                    "El proveedor omitió la traducción de la entrada {}",
                    entry.entry_id
                ))
            })?;
            Ok(TranslationResponse {
                entry_id: entry.entry_id.clone(),
                translated_text,
            })
        })
        .collect()
}

fn parse_compact_external_response(
    parsed: CompactResponseEnvelope,
    entry_refs: Option<&[AiBatchResponseRef]>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    let entry_refs = entry_refs.ok_or_else(|| {
        AppError::Parse(
            "La respuesta de IA usó ID locales compactos, pero no se entregó el mapa de respuesta"
                .into(),
        )
    })?;

    if parsed.translations.len() != entry_refs.len() {
        return Err(AppError::Parse(format!(
            "La respuesta de IA tiene {} traducciones, pero se esperaban {}",
            parsed.translations.len(),
            entry_refs.len()
        )));
    }

    let refs_by_id = entry_refs
        .iter()
        .map(|entry| (entry.local_id, entry))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut translations = Vec::with_capacity(parsed.translations.len());

    for item in parsed.translations {
        let entry_ref = refs_by_id.get(&item.local_id).ok_or_else(|| {
            AppError::Parse(format!(
                "La respuesta de IA devolvió un ID local inesperado: {}",
                item.local_id
            ))
        })?;
        if !seen.insert(item.local_id) {
            return Err(AppError::Parse(format!(
                "La respuesta de IA devolvió un ID local duplicado: {}",
                item.local_id
            )));
        }

        translations.push(AiBatchTranslation {
            file_path: entry_ref.file_path.clone(),
            entry_id: entry_ref.entry_id.clone(),
            translation: item.translated_text,
            notes: item.notes,
            skip: item.skip,
        });
    }

    Ok(translations)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    InApp,
    ExternalAi,
}

#[derive(Clone, Copy)]
struct CompactEntrySource<'a> {
    file_path: &'a str,
    entry_id: &'a str,
    file_context: &'a str,
    original: &'a str,
    notes: Option<&'a str>,
    current_translation: Option<&'a str>,
    instruction_candidate: Option<bool>,
    instruction_reason: Option<&'a str>,
}

fn build_bundle(
    options: &ProviderBundleOptions,
    glossary_and_ids: Option<(&[GlossaryTerm], Option<(&str, &str, String)>)>,
    project_name_override: Option<&str>,
    entries: &[CompactEntrySource<'_>],
    context: &TranslationBatchContext,
    provider_kind: ProviderKind,
) -> Result<CompactPromptBundle, AppError> {
    let (glossary, envelope_ids) = glossary_and_ids
        .map(|(glossary, envelope_ids)| (glossary, envelope_ids))
        .unwrap_or((&[], None));
    let meta = CompactPromptMeta {
        source_lang: options.source_lang.clone(),
        target_lang: options.target_lang.clone(),
        file_path: context.file_path.clone(),
        file_kind: context.file_kind.clone(),
        chunk_index: context.chunk_index,
        total_chunks: context.total_chunks,
        project_name: non_empty(
            project_name_override
                .unwrap_or(context.project_name.as_str())
                .trim(),
        ),
        project_id: envelope_ids.as_ref().map(|ids| ids.0.to_string()),
        batch_id: envelope_ids.as_ref().map(|ids| ids.1.to_string()),
        engine: envelope_ids.as_ref().map(|ids| ids.2.clone()),
    };
    let compact_context = CompactPromptContext {
        style_guide: context.style_guide.clone(),
        glossary: glossary
            .iter()
            .map(|term| CompactGlossaryPair {
                source: term.source.clone(),
                target: term.target.clone(),
            })
            .collect(),
        term_hints: context
            .term_hints
            .iter()
            .map(|hint| CompactTermHint {
                source: hint.source.clone(),
                target: hint.target.clone(),
                note: hint.note.clone(),
            })
            .collect(),
        examples: context
            .translation_examples
            .iter()
            .enumerate()
            .map(|(index, example)| CompactExample {
                context: compact_context_label(&example.file_context),
                original: mask_text_with_tag(&example.original, &format!("EX{index}")).text,
                translation: mask_text_with_tag(&example.translation, &format!("ET{index}")).text,
            })
            .collect(),
        file_hints: context
            .file_context_hints
            .iter()
            .map(|hint| compact_context_label(hint))
            .collect(),
        previous_hints: context.previous_chunk_hints.clone(),
        next_hints: context.next_chunk_hints.clone(),
    };

    let mut entry_refs = Vec::with_capacity(entries.len());
    let mut compact_entries = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let local_id = u16::try_from(index + 1)
            .map_err(|_| AppError::Project("Chunk exceeded supported local-id range".into()))?;
        let masked_original = mask_text_with_tag(entry.original, &format!("K{local_id}"));
        let masked_current_translation = entry
            .current_translation
            .filter(|text| !text.trim().is_empty())
            .map(|text| mask_text_with_tag(text, &format!("CT{local_id}")).text);
        let metrics = compact_length_metrics(entry.original);

        entry_refs.push(ChunkEntryRef {
            local_id,
            file_path: entry.file_path.to_string(),
            entry_id: entry.entry_id.to_string(),
            original_is_multiline: entry.original.contains('\n'),
            masks: masked_original.masks.clone(),
        });
        compact_entries.push(CompactPromptEntry {
            local_id,
            context: compact_context_label(entry.file_context),
            text: masked_original.text,
            original_visible_chars: metrics.visible_chars,
            original_line_count: metrics.line_count,
            target_visible_budget: metrics.target_budget,
            notes: entry.notes.and_then(non_empty),
            current_translation: masked_current_translation,
            instruction_candidate: entry.instruction_candidate.filter(|flag| *flag),
            instruction_reason: entry.instruction_reason.and_then(non_empty),
        });
    }

    let payload = CompactPromptPayload {
        meta,
        context: compact_context,
        entries: compact_entries,
    };

    Ok(CompactPromptBundle {
        system_prompt: build_system_prompt(
            provider_kind,
            &options.source_lang,
            &options.target_lang,
        ),
        user_payload: serde_json::to_string(&payload)?,
        response_schema: matches!(provider_kind, ProviderKind::InApp)
            .then(provider_response_schema),
        entry_refs,
    })
}

fn build_system_prompt(
    provider_kind: ProviderKind,
    source_lang: &str,
    target_lang: &str,
) -> String {
    let response_shape = match provider_kind {
        ProviderKind::InApp => r#"{"translations":[{"k":1,"t":"translated text"}]}"#,
        ProviderKind::ExternalAi => {
            r#"{"p":"project-id","b":"batch-id","translations":[{"k":1,"t":"translated text","n":"","s":false}]}"#
        }
    };
    let payload_description = match provider_kind {
        ProviderKind::InApp => {
            "The user message is a compact JSON payload with m=metadata, c=context and e=entries.\n"
        }
        ProviderKind::ExternalAi => {
            "The user message contains an editable JSON translation sheet; translate reference field o into target field t.\n"
        }
    };

    let extra_rules = match provider_kind {
        ProviderKind::InApp => {
            "- Return every requested entry exactly once.\n- Do not invent IDs or skip entries.\n"
        }
        ProviderKind::ExternalAi => concat!(
            "- Echo the exact project and batch IDs from the payload as p and b.\n",
            "- Return every requested entry exactly once.\n",
            "- If an entry should stay excluded, set s=true and leave t empty.\n",
            "- Keep n short and optional.\n"
        ),
    };
    let language_rules = language_pair_prompt_rules(source_lang, target_lang);

    format!(
        concat!(
            "You are localizing game text from {source_lang} to {target_lang}.\n",
            "{payload_description}",
            "Goal: natural playable localization, not a literal transcript. Preserve meaning, intent, subtext, emotional beat, character relationship and gameplay function.\n",
            "Preserve control codes, placeholders and escape sequences exactly.\n",
            "Masked placeholders such as __K1_CC0__ must survive unchanged and in the same count.\n",
            "Preserve choice/plugin condition prefixes such as if(s[12]) and en(s[20]) exactly; translate only the visible label after them.\n",
            "Do not add, remove or rename engine script commands, tags, variables, internal keys, labels, event logic or decorative formatting; return translated text only.\n",
            "Each entry includes oc=original visible chars, ol=original line count and tb=target visible character budget with cushion. Stay near or below tb for dialogue/UI by adapting and condensing instead of literal expansion.\n",
            "Keep line breaks only when they are structural or necessary; do not add extra dialogue lines, extra boxes or explanatory text.\n",
            "Use glossary/style/context when relevant, but do not add commentary or markdown.\n",
            "If an entry is a speaker/name/title line, translate only that label and keep it separate from following dialogue. Do not prepend speakers to dialogue, merge names with dialogue or duplicate speaker names.\n",
            "Keep character voice consistent across nearby lines. Rewrite literal or stiff phrasing into natural in-game dialogue while preserving meaning, intensity and relationship dynamics.\n",
            "Adapt jokes, teasing, intimacy, fear, anger, hesitation and politeness into target-language phrasing that reads like spoken dialogue, not dictionary output.\n",
            "Avoid fansub calques, clinical wording, regional slang, voseo, over-formal phrasing and awkward source-language word order unless the source character clearly uses that register.\n",
            "{language_rules}",
            "{extra_rules}",
            "Respond ONLY with valid JSON in this shape:\n{response_shape}"
        ),
        source_lang = source_lang,
        target_lang = target_lang,
        payload_description = payload_description,
        language_rules = language_rules,
        extra_rules = extra_rules,
        response_shape = response_shape
    )
}

fn language_pair_prompt_rules(source_lang: &str, target_lang: &str) -> &'static str {
    if is_japanese_language(source_lang) && is_spanish_language(target_lang) {
        concat!(
            "For Spanish targets, translate visible game database names, enemy/troop names, skill/item/armor names, game titles, menu terms and meaningful compact labels into Spanish.\n",
            "Do not leave English words unchanged just because they are short, uppercase or concatenated; split compounds naturally when needed, such as FIREHOUND to Sabueso de fuego.\n",
            "Preserve only clear proper names, staff names, handles, control codes, script/plugin identifiers, file names, variable names, placeholders and common RPG acronyms such as HP, MP, TP and EXP.\n",
            "For Japanese to Spanish, write natural neutral Spanish instead of literal calques.\n",
            "Examples: 学校の帰り道 is de vuelta de la escuela, not de regreso a la escuela; 一人称 in settings is forma de referirse a sí mismo, not primera persona.\n",
            "Adapt family-role terms as natural address or relationship language in context; avoid repeating phrases like tu hermana mayor when Spanish would use a name, pronoun, affectionate title or omission.\n",
            "Adapt sighs, moans and onomatopoeia into readable Spanish sound effects instead of leaving romaji or mechanical literal sounds.\n",
            "Adapt dialogue like dubbing or subtitles: preserve intent and timing, prefer compact Spanish that fits the original text box/window over word-for-word completeness.\n",
            "Keep UI and choice labels short enough for the original game UI/text windows; avoid sentence-length labels when a concise command works.\n",
            "Use Spanish punctuation in Spanish text; avoid Japanese punctuation such as 。, ？, ！, 「」 unless it is part of a preserved name or source-only artifact.\n",
            "For credits/staff rolls, translate role headings but preserve names, circles, brand names and supporter handles.\n"
        )
    } else if is_spanish_language(target_lang) {
        concat!(
            "For Spanish targets, translate visible game database names, enemy/troop names, skill/item/armor names, game titles, menu terms and meaningful compact labels into Spanish.\n",
            "Do not leave English words unchanged just because they are short, uppercase or concatenated; split compounds naturally when needed, such as FIREHOUND to Sabueso de fuego.\n",
            "Preserve only clear proper names, staff names, handles, control codes, script/plugin identifiers, file names, variable names, placeholders and common RPG acronyms such as HP, MP, TP and EXP.\n"
        )
    } else {
        ""
    }
}

fn compact_context_label(value: &str) -> String {
    let parts = value
        .split('>')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() <= 2 {
        return parts.join(" > ");
    }

    parts[parts.len().saturating_sub(2)..].join(" > ")
}

#[derive(Debug, Clone, Copy)]
struct CompactLengthMetrics {
    visible_chars: usize,
    line_count: usize,
    target_budget: usize,
}

fn compact_length_metrics(text: &str) -> CompactLengthMetrics {
    const SAFE_RPG_CHARS_PER_LINE: usize = 42;
    const JA_TO_ES_LENGTH_NUMERATOR: usize = 18;
    const JA_TO_ES_LENGTH_DENOMINATOR: usize = 10;
    const SHORT_TEXT_PADDING: usize = 8;

    let visible_chars = visible_prompt_char_count(text);
    let line_count = text.split('\n').count().max(1);
    let box_budget = line_count.saturating_mul(SAFE_RPG_CHARS_PER_LINE).max(8);
    let proportional_budget =
        visible_chars.saturating_mul(JA_TO_ES_LENGTH_NUMERATOR) / JA_TO_ES_LENGTH_DENOMINATOR;
    let padded_budget = visible_chars.saturating_add(SHORT_TEXT_PADDING);
    let target_budget = proportional_budget
        .max(padded_budget)
        .min(box_budget)
        .max(visible_chars.min(box_budget))
        .max(8);

    CompactLengthMetrics {
        visible_chars,
        line_count,
        target_budget,
    }
}

fn visible_prompt_char_count(text: &str) -> usize {
    let spans = collect_special_token_spans(text);
    let mut total = 0usize;
    let mut last_index = 0usize;
    for span in spans {
        total += count_visible_chars(&text[last_index..span.start]);
        last_index = span.end;
    }
    total + count_visible_chars(&text[last_index..])
}

fn count_visible_chars(text: &str) -> usize {
    text.chars()
        .filter(|ch| !matches!(ch, '\n' | '\r' | '\t'))
        .count()
}

fn finalize_model_text(text: &str, entry_ref: &ChunkEntryRef) -> String {
    let mut restored = text.replace("\r\n", "\n");
    for mask in &entry_ref.masks {
        restored = restored.replace(mask.placeholder.as_str(), mask.original.as_str());
    }
    if entry_ref.original_is_multiline {
        restored = restored.replace("\\r\\n", "\n");
        restored = restored.replace("\\n", "\n");
    }
    restored
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

fn normalize_json_response(raw: &str) -> String {
    let trimmed = strip_markdown_fences(raw);
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed.to_string();
    }

    extract_balanced_json(trimmed)
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

fn repair_redundant_translation_value(raw: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""t":"(?:[^"\\]|\\.)*":"#).unwrap())
        .replace_all(raw, "\"t\":")
        .into_owned()
}

fn extract_balanced_json(raw: &str) -> Option<&str> {
    for (start, ch) in raw.char_indices() {
        if ch != '{' && ch != '[' {
            continue;
        }

        let mut stack = vec![matching_json_close(ch)];
        let mut in_string = false;
        let mut escaping = false;
        for (offset, current) in raw[start + ch.len_utf8()..].char_indices() {
            if in_string {
                if escaping {
                    escaping = false;
                } else if current == '\\' {
                    escaping = true;
                } else if current == '"' {
                    in_string = false;
                }
                continue;
            }

            match current {
                '"' => in_string = true,
                '{' | '[' => stack.push(matching_json_close(current)),
                '}' | ']' => {
                    if stack.pop() != Some(current) {
                        break;
                    }
                    if stack.is_empty() {
                        let end = start + ch.len_utf8() + offset + current.len_utf8();
                        return Some(&raw[start..end]);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn matching_json_close(open: char) -> char {
    match open {
        '{' => '}',
        '[' => ']',
        _ => unreachable!("only JSON object/array openers are supported"),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone)]
struct MaskedText {
    text: String,
    masks: Vec<MaskReplacement>,
}

fn mask_text_with_tag(text: &str, tag: &str) -> MaskedText {
    let spans = collect_special_token_spans(text);
    if spans.is_empty() {
        return MaskedText {
            text: text.to_string(),
            masks: Vec::new(),
        };
    }

    let mut output = String::with_capacity(text.len());
    let mut last_index = 0usize;
    let mut masks = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        output.push_str(&text[last_index..span.start]);
        let placeholder = format!("__{tag}_CC{index}__");
        output.push_str(&placeholder);
        masks.push(MaskReplacement {
            placeholder,
            original: span.token.clone(),
        });
        last_index = span.end;
    }
    output.push_str(&text[last_index..]);

    MaskedText {
        text: output,
        masks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, EntryStatus, TranslationEntry, TranslationFile};
    use crate::services::provider::TranslationExample;

    fn sample_context() -> TranslationBatchContext {
        TranslationBatchContext {
            project_name: "Buebito".into(),
            file_path: "data/CommonEvents.json".into(),
            file_kind: "Events".into(),
            chunk_index: 2,
            total_chunks: 12,
            approximate_char_count: 42,
            style_guide: vec!["Keep honorifics.".into()],
            term_hints: Vec::new(),
            translation_examples: vec![TranslationExample {
                file_path: "data/CommonEvents.json".into(),
                file_context: "CommonEvents > text #1".into(),
                original: "\\N[1]\nHello".into(),
                translation: "\\N[1]\nHola".into(),
                notes: String::new(),
            }],
            file_context_hints: vec!["CommonEvents > text #2".into()],
            previous_chunk_hints: vec!["Prev: speaker keeps flirting".into()],
            next_chunk_hints: vec!["Next: scene moves to bedroom".into()],
        }
    }

    #[test]
    fn test_build_provider_bundle_uses_local_ids_and_masks_tokens() {
        let bundle = build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "Map001 > Show Text".into(),
                text: "\\N[1]\nHello".into(),
                notes: String::new(),
            }],
            &[GlossaryTerm {
                source: "Onee-chan".into(),
                target: "Onee-chan".into(),
                case_sensitive: true,
            }],
            &sample_context(),
        )
        .unwrap();

        assert!(bundle.user_payload.contains("\"k\":1"));
        assert!(bundle.user_payload.contains("__K1_CC0__"));
        assert!(bundle.user_payload.contains("\"ol\":2"));
        assert!(bundle.user_payload.contains("\"tb\":"));
        assert!(!bundle.user_payload.contains("entry-1"));
        assert!(bundle.system_prompt.contains("\"k\":1"));
        assert!(bundle
            .system_prompt
            .contains("tb=target visible character budget"));
    }

    #[test]
    fn test_build_provider_bundle_adds_spanish_database_name_rules() {
        let bundle = build_provider_bundle(
            "en",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "Enemies > #1 > name".into(),
                text: "FIREHOUND".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        assert!(bundle.system_prompt.contains("enemy/troop names"));
        assert!(bundle.system_prompt.contains("FIREHOUND"));
        assert!(bundle.system_prompt.contains("HP, MP, TP and EXP"));
    }

    #[test]
    fn test_parse_provider_response_restores_real_ids_and_newlines() {
        let bundle = build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "\\N[1]\nHello".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        let parsed = parse_provider_response(
            r#"{"translations":[{"k":1,"t":"__K1_CC0__\\nHola"}]}"#,
            &bundle.entry_refs,
        )
        .unwrap();

        assert_eq!(parsed[0].entry_id, "entry-1");
        assert_eq!(parsed[0].translated_text, "\\N[1]\nHola");
    }

    #[test]
    fn test_parse_provider_response_extracts_json_after_reasoning() {
        let bundle = build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "Hello".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        let parsed = parse_provider_response(
            r#"<think>
I should return the required JSON.
</think>
{"translations":[{"k":1,"t":"Hola"}]}
"#,
            &bundle.entry_refs,
        )
        .unwrap();

        assert_eq!(parsed[0].entry_id, "entry-1");
        assert_eq!(parsed[0].translated_text, "Hola");
    }

    #[test]
    fn test_parse_provider_response_keeps_last_duplicate_translation_field() {
        let bundle = build_provider_bundle(
            "en",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "Hello".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        let parsed = parse_provider_response(
            r#"{"translations":[{"k":1,"t":"Speaker","t":"Speaker\nHola"}]}"#,
            &bundle.entry_refs,
        )
        .unwrap();

        assert_eq!(parsed[0].translated_text, "Speaker\nHola");
    }

    #[test]
    fn test_parse_provider_response_repairs_redundant_translation_prefix() {
        assert_eq!(
            repair_redundant_translation_value(
                r#"{"translations":[{"k":1,"t":"Speaker":"Speaker\nHola"}]}"#
            ),
            r#"{"translations":[{"k":1,"t":"Speaker\nHola"}]}"#
        );
        let bundle = build_provider_bundle(
            "en",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "Hello".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        let parsed = parse_provider_response(
            r#"{"translations":[{"k":1,"t":"Speaker":"Speaker\nHola"}]}"#,
            &bundle.entry_refs,
        )
        .unwrap();

        assert_eq!(parsed[0].translated_text, "Speaker\nHola");
    }

    #[test]
    fn test_parse_external_ai_response_accepts_compact_response_map() {
        let project = Project {
            id: "project-1".into(),
            name: "Test".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mz,
            game_dir: "D:\\Games\\Test".into(),
            files: vec![TranslationFile {
                relative_path: "data/CommonEvents.json".into(),
                entries: vec![TranslationEntry {
                    id: "entry-1".into(),
                    file_context: "ctx".into(),
                    original: "Hello".into(),
                    translation: String::new(),
                    status: EntryStatus::Untranslated,
                    notes: String::new(),
                }],
            }],
            glossary: Vec::new(),
            style_guide: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let bundle = build_external_ai_bundle(
            &project,
            "batch-1",
            &[AiBatchEntry {
                file_path: "data/CommonEvents.json".into(),
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                original: "Hello".into(),
                current_translation: String::new(),
                instruction_candidate: false,
                instruction_reason: None,
            }],
            &sample_context(),
        )
        .unwrap();

        let parsed = parse_external_ai_response(
            r#"{"p":"project-1","b":"batch-1","translations":[{"k":1,"t":"Hola","n":"ok","s":false}]}"#,
            Some("project-1"),
            Some("batch-1"),
            Some(&bundle.response_map()),
        )
        .unwrap();

        assert_eq!(parsed[0].file_path, "data/CommonEvents.json");
        assert_eq!(parsed[0].entry_id, "entry-1");
        assert_eq!(parsed[0].translation, "Hola");
    }

    #[test]
    fn test_parse_external_ai_response_rejects_legacy_array_when_ids_are_expected() {
        let error = parse_external_ai_response(
            r#"[{"filePath":"data/CommonEvents.json","entryId":"entry-1","translation":"Hola"}]"#,
            Some("project-1"),
            Some("batch-1"),
            None,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("projectId/batchId cuando el lote exportado los define"));
    }

    #[test]
    fn test_build_provider_bundle_masks_name_honorific_compounds_as_one_unit() {
        let bundle = build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "\\N[1]-kun! \\N[2]-onee-chan's cooking is super delicious!".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        assert!(bundle.user_payload.contains("__K1_CC0__"));
        assert!(bundle.user_payload.contains("__K1_CC1__"));
        assert!(!bundle.user_payload.contains("\\N[1]-kun"));
        assert!(!bundle.user_payload.contains("\\N[2]-onee-chan"));
    }

    #[test]
    fn test_build_provider_bundle_masks_choice_condition_dsl() {
        let bundle = build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "Map003:25:0:choice:47:1".into(),
                file_context: "Map003 > choice #9 option 2".into(),
                text: "if(s[211])夫に露出していたことを打ち明ける".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        assert!(bundle.user_payload.contains("__K1_CC0__"));
        assert!(!bundle.user_payload.contains("if(s[211])"));
        assert!(bundle
            .system_prompt
            .contains("Preserve choice/plugin condition prefixes"));
    }

    #[test]
    fn test_build_provider_bundle_includes_japanese_to_spanish_rules() {
        let bundle = build_provider_bundle(
            "Japanese",
            "Spanish",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "学校の帰り道".into(),
                notes: String::new(),
            }],
            &[],
            &sample_context(),
        )
        .unwrap();

        assert!(!bundle.system_prompt.contains("RPG Maker game text"));
        assert!(bundle.system_prompt.contains("natural neutral Spanish"));
        assert!(bundle
            .system_prompt
            .contains("natural playable localization"));
        assert!(bundle.system_prompt.contains("speaker/name/title line"));
        assert!(bundle.system_prompt.contains("dubbing or subtitles"));
        assert!(bundle.system_prompt.contains("de vuelta de la escuela"));
        assert!(bundle
            .system_prompt
            .contains("forma de referirse a sí mismo"));
        assert!(bundle.system_prompt.contains("family-role terms"));
        assert!(bundle
            .system_prompt
            .contains("readable Spanish sound effects"));
        assert!(bundle.system_prompt.contains("fansub calques"));
    }
}

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::AppError;
use crate::models::{AiBatchEntry, AiBatchResponseRef, AiBatchTranslation, GlossaryTerm, Project};
use crate::services::provider::{TranslationBatchContext, TranslationRequest, TranslationResponse};
use crate::services::translation_integrity::collect_special_token_spans;

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
    let normalized = strip_markdown_fences(raw_response);

    if let Ok(parsed) = serde_json::from_str::<CompactResponseEnvelope>(normalized) {
        return parse_compact_provider_response(parsed, entry_refs);
    }

    let legacy: LegacyProviderEnvelope = serde_json::from_str(normalized)
        .map_err(|error| AppError::Api(format!("Failed to parse provider response: {error}")))?;
    parse_legacy_provider_response(legacy, entry_refs)
}

pub fn parse_external_ai_response(
    raw_response: &str,
    expected_project_id: Option<&str>,
    expected_batch_id: Option<&str>,
    entry_refs: Option<&[AiBatchResponseRef]>,
) -> Result<Vec<AiBatchTranslation>, AppError> {
    let normalized = strip_markdown_fences(raw_response);
    let value: serde_json::Value = serde_json::from_str(normalized)?;

    match value {
        serde_json::Value::Array(items) => serde_json::from_value(serde_json::Value::Array(items))
            .map_err(|error| AppError::Parse(format!("Invalid AI batch response schema: {error}"))),
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
                    AppError::Parse(format!("Invalid AI batch response schema: {error}"))
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
            "AI batch response must be a JSON array or an object with 'translations'".into(),
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
                "AI batch response projectId does not match the current export batch".into(),
            ));
        }
    }
    if let Some(expected) = expected_batch_id {
        if actual_batch_id != Some(expected) {
            return Err(AppError::Parse(
                "AI batch response batchId does not match the current export batch".into(),
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
            "Provider returned {} translations but expected {}",
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
                "Provider returned an unexpected local entry ID: {}",
                translation.local_id
            ))
        })?;
        if !seen.insert(translation.local_id) {
            return Err(AppError::Api(format!(
                "Provider returned a duplicate local entry ID: {}",
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
                    "Provider omitted a translation for entry ID {}",
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
            "Provider returned {} translations but expected {}",
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
                "Provider returned an unexpected entry ID: {}",
                translation.entry_id
            )));
        }
        if !seen_ids.insert(translation.entry_id.clone()) {
            return Err(AppError::Api(format!(
                "Provider returned a duplicate entry ID: {}",
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
                    "Provider omitted a translation for entry ID {}",
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
            "AI batch response used compact local IDs but no response map was provided".into(),
        )
    })?;

    if parsed.translations.len() != entry_refs.len() {
        return Err(AppError::Parse(format!(
            "AI batch response has {} translations but expected {}",
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
                "AI batch response returned an unexpected local entry ID: {}",
                item.local_id
            ))
        })?;
        if !seen.insert(item.local_id) {
            return Err(AppError::Parse(format!(
                "AI batch response returned a duplicate local entry ID: {}",
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
        ProviderKind::InApp => {
            "{\"translations\":[{\"k\":1,\"t\":\"...\"}]}"
        }
        ProviderKind::ExternalAi => {
            "{\"p\":\"project-id\",\"b\":\"batch-id\",\"translations\":[{\"k\":1,\"t\":\"...\",\"n\":\"optional short note\",\"s\":false}]}"
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

    format!(
        concat!(
            "You are translating RPG Maker game text from {source_lang} to {target_lang}.\n",
            "The user message is a compact JSON payload with metadata, context and entries.\n",
            "Preserve control codes, placeholders, escape sequences and line breaks exactly.\n",
            "Masked placeholders such as __K1_CC0__ must survive unchanged and in the same count.\n",
            "Use glossary/style/context when relevant, but do not add commentary or markdown.\n",
            "{extra_rules}",
            "Respond ONLY with valid JSON in this exact shape: {response_shape}"
        ),
        source_lang = source_lang,
        target_lang = target_lang,
        extra_rules = extra_rules,
        response_shape = response_shape
    )
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
        assert!(!bundle.user_payload.contains("entry-1"));
        assert!(bundle.system_prompt.contains("\"k\":1"));
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
}

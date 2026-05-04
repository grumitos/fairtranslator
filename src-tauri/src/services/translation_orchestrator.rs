use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::{EntryStatus, Project, ProviderType, TranslationEntry, TranslationFile};
use crate::services::ai_batch;
use crate::services::codex::CodexProvider;
use crate::services::openai::OpenAiProvider;
use crate::services::provider::{
    TranslationBatchContext, TranslationExample, TranslationRequest, TranslationTermHint,
};

const DEFAULT_MAX_ENTRIES_PER_CHUNK: usize = 20;
const LLM_MAX_ENTRIES_PER_CHUNK: usize = 24;
const EXTERNAL_AI_DEFAULT_MAX_ENTRIES_PER_CHUNK: usize = 200;
const MAX_TERM_HINTS: usize = 12;
const MAX_TRANSLATION_EXAMPLES: usize = 6;
const MAX_FILE_CONTEXT_HINTS: usize = 5;
const MAX_NEIGHBOR_HINTS: usize = 3;
const MAX_ESTIMATED_TOTAL_TOKENS: usize = 18_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkPlanningMode {
    Standard,
    OpenAi,
    Codex,
    ExternalAi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTranslationChunk {
    pub relative_path: String,
    pub entries: Vec<TranslationRequest>,
    pub context: TranslationBatchContext,
    pub estimated_prompt_tokens: usize,
    pub planner_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedTranslationExample {
    pub file_path: String,
    pub file_kind: String,
    pub file_context: String,
    pub original: String,
    pub translation: String,
    pub notes: String,
    pub status_priority: usize,
    pub selection_priority: usize,
    pub translation_len: usize,
    pub original_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerCacheData {
    pub fingerprint: String,
    pub prepared_examples: Vec<PreparedTranslationExample>,
    #[serde(default)]
    pub planned_chunks: Vec<PlannedTranslationChunk>,
}

#[derive(Debug, Clone)]
pub struct TranslationPlanBuildResult {
    pub plan: Vec<PlannedTranslationChunk>,
    pub planner_cache: PlannerCacheData,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanningProgressSnapshot {
    pub phase: String,
    pub files_total: usize,
    pub files_done: usize,
    pub pending_entries: usize,
    pub planned_chunks: usize,
    pub current_file: Option<String>,
}

pub fn entry_is_pending_translation(entry: &TranslationEntry) -> bool {
    entry.status == EntryStatus::Untranslated && !entry.original.trim().is_empty()
}

fn mode_for_provider(provider: &ProviderType) -> ChunkPlanningMode {
    match provider {
        ProviderType::Openai => ChunkPlanningMode::OpenAi,
        ProviderType::Codex => ChunkPlanningMode::Codex,
        ProviderType::Deepl | ProviderType::Google => ChunkPlanningMode::Standard,
    }
}

pub fn build_translation_plan(
    project: &Project,
    file_paths: &[String],
    provider: &ProviderType,
) -> Vec<PlannedTranslationChunk> {
    let mode = mode_for_provider(provider);
    build_translation_plan_for_mode(project, file_paths, mode, max_entries_for_mode(mode, None))
        .plan
}

pub fn build_external_ai_plan(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
) -> Vec<PlannedTranslationChunk> {
    let mode = ChunkPlanningMode::ExternalAi;
    build_translation_plan_for_mode(
        project,
        file_paths,
        mode,
        max_entries_for_mode(mode, max_entries),
    )
    .plan
}

pub fn planner_cache_fingerprint(
    project: &Project,
    file_paths: &[String],
    mode: &ProviderType,
) -> String {
    planner_cache_fingerprint_for_mode(
        project,
        file_paths,
        mode_for_provider(mode),
        max_entries_for_mode(mode_for_provider(mode), None),
    )
}

pub fn build_translation_plan_with_cache_and_progress<F>(
    project: &Project,
    file_paths: &[String],
    provider: &ProviderType,
    planner_cache: Option<&PlannerCacheData>,
    progress: &mut F,
) -> TranslationPlanBuildResult
where
    F: FnMut(PlanningProgressSnapshot),
{
    let mode = mode_for_provider(provider);
    build_translation_plan_for_mode_with_cache_and_progress(
        project,
        file_paths,
        mode,
        max_entries_for_mode(mode, None),
        planner_cache,
        progress,
    )
}

pub fn build_external_ai_plan_with_cache_and_progress<F>(
    project: &Project,
    file_paths: &[String],
    max_entries: Option<usize>,
    planner_cache: Option<&PlannerCacheData>,
    progress: &mut F,
) -> TranslationPlanBuildResult
where
    F: FnMut(PlanningProgressSnapshot),
{
    let mode = ChunkPlanningMode::ExternalAi;
    build_translation_plan_for_mode_with_cache_and_progress(
        project,
        file_paths,
        mode,
        max_entries_for_mode(mode, max_entries),
        planner_cache,
        progress,
    )
}

pub fn ensure_translation_plan_within_budget(
    project: &Project,
    plan: &[PlannedTranslationChunk],
    provider: &ProviderType,
) -> Result<(), AppError> {
    ensure_plan_within_budget_for_mode(project, plan, mode_for_provider(provider))
}

pub fn ensure_external_ai_plan_within_budget(
    project: &Project,
    plan: &[PlannedTranslationChunk],
) -> Result<(), AppError> {
    ensure_plan_within_budget_for_mode(project, plan, ChunkPlanningMode::ExternalAi)
}

fn ensure_plan_within_budget_for_mode(
    _project: &Project,
    plan: &[PlannedTranslationChunk],
    mode: ChunkPlanningMode,
) -> Result<(), AppError> {
    if mode == ChunkPlanningMode::Standard {
        return Ok(());
    }

    for chunk in plan {
        let estimated = chunk.estimated_prompt_tokens;
        if estimated <= MAX_ESTIMATED_TOTAL_TOKENS {
            continue;
        }

        let first_entry_id = chunk
            .entries
            .first()
            .map(|entry| entry.entry_id.as_str())
            .unwrap_or("unknown");
        let provider_label = match mode {
            ChunkPlanningMode::OpenAi => "OpenAI",
            ChunkPlanningMode::Codex => "Codex",
            ChunkPlanningMode::ExternalAi => "External AI export",
            ChunkPlanningMode::Standard => "provider",
        };

        return Err(AppError::Project(format!(
            "{provider_label} chunk '{}' {}/{} exceeds the estimated prompt budget ({} > {}). First entry '{}'. Reduce glossary/style-guide/context size or split the oversized source entry before running this flow.",
            chunk.relative_path,
            chunk.context.chunk_index,
            chunk.context.total_chunks,
            estimated,
            MAX_ESTIMATED_TOTAL_TOKENS,
            first_entry_id
        )));
    }

    Ok(())
}

fn build_translation_plan_for_mode(
    project: &Project,
    file_paths: &[String],
    mode: ChunkPlanningMode,
    max_entries_per_chunk: usize,
) -> TranslationPlanBuildResult {
    let mut noop = |_progress: PlanningProgressSnapshot| {};
    build_translation_plan_for_mode_with_cache_and_progress(
        project,
        file_paths,
        mode,
        max_entries_per_chunk,
        None,
        &mut noop,
    )
}

fn build_translation_plan_for_mode_with_cache_and_progress<F>(
    project: &Project,
    file_paths: &[String],
    mode: ChunkPlanningMode,
    max_entries_per_chunk: usize,
    planner_cache: Option<&PlannerCacheData>,
    progress: &mut F,
) -> TranslationPlanBuildResult
where
    F: FnMut(PlanningProgressSnapshot),
{
    let selected_files: Vec<&TranslationFile> = project
        .files
        .iter()
        .filter(|file| file_paths.is_empty() || file_paths.contains(&file.relative_path))
        .collect();
    let fingerprint =
        planner_cache_fingerprint_for_mode(project, file_paths, mode, max_entries_per_chunk);
    if let Some(cached_plan) = reuse_cached_plan(&selected_files, planner_cache, &fingerprint) {
        progress(PlanningProgressSnapshot {
            phase: "planning".into(),
            files_total: selected_files.len(),
            files_done: selected_files.len(),
            pending_entries: cached_plan
                .iter()
                .map(|chunk| chunk.entries.len())
                .sum::<usize>(),
            planned_chunks: cached_plan.len(),
            current_file: cached_plan.last().map(|chunk| chunk.relative_path.clone()),
        });
        return TranslationPlanBuildResult {
            plan: cached_plan,
            planner_cache: planner_cache
                .expect("cache exists when cached plan is reused")
                .clone(),
        };
    }
    let style_guide = build_style_guide(project, &selected_files);
    let term_hints = collect_term_hints(project, &selected_files);
    let prepared_examples = collect_prepared_translation_examples(
        project,
        &selected_files,
        planner_cache,
        &fingerprint,
    );
    let total_pending_entries = selected_files
        .iter()
        .flat_map(|file| file.entries.iter())
        .filter(|entry| entry_is_pending_translation(entry))
        .count();

    let mut plan = Vec::new();
    progress(PlanningProgressSnapshot {
        phase: "planning".into(),
        files_total: selected_files.len(),
        files_done: 0,
        pending_entries: total_pending_entries,
        planned_chunks: 0,
        current_file: None,
    });

    for (file_index, file) in selected_files.iter().enumerate() {
        let file_kind = file_kind_for_path(&file.relative_path).to_string();
        let pending_entries: Vec<TranslationRequest> = file
            .entries
            .iter()
            .filter(|entry| entry_is_pending_translation(entry))
            .map(|entry| TranslationRequest {
                entry_id: entry.id.clone(),
                file_context: entry.file_context.clone(),
                text: entry.original.clone(),
                notes: entry.notes.clone(),
            })
            .collect();

        if pending_entries.is_empty() {
            continue;
        }

        let chunked_entries = chunk_requests(
            project,
            &file.relative_path,
            &file_kind,
            &pending_entries,
            &style_guide,
            &term_hints,
            &prepared_examples,
            mode,
            max_entries_per_chunk,
        );
        let total_chunks = chunked_entries.len();

        for index in 0..chunked_entries.len() {
            let previous_chunk = index
                .checked_sub(1)
                .and_then(|prev| chunked_entries.get(prev));
            let next_chunk = chunked_entries.get(index + 1);
            let chunk = chunked_entries[index].clone();
            let (context, estimated_prompt_tokens) = build_context_for_chunk(
                project,
                &file.relative_path,
                &file_kind,
                &chunk,
                &style_guide,
                &term_hints,
                &prepared_examples,
                previous_chunk.map(Vec::as_slice),
                next_chunk.map(Vec::as_slice),
                index + 1,
                total_chunks,
                mode,
            );

            plan.push(PlannedTranslationChunk {
                relative_path: file.relative_path.clone(),
                entries: chunk,
                context,
                estimated_prompt_tokens,
                planner_fingerprint: Some(fingerprint.clone()),
            });
        }

        progress(PlanningProgressSnapshot {
            phase: "planning".into(),
            files_total: selected_files.len(),
            files_done: file_index + 1,
            pending_entries: total_pending_entries,
            planned_chunks: plan.len(),
            current_file: Some(file.relative_path.clone()),
        });
    }

    let cached_plan = plan.clone();
    TranslationPlanBuildResult {
        plan,
        planner_cache: PlannerCacheData {
            fingerprint,
            prepared_examples,
            planned_chunks: cached_plan,
        },
    }
}

fn build_style_guide(project: &Project, selected_files: &[&TranslationFile]) -> Vec<String> {
    let mut notes = project.style_guide.clone();
    let has_ui = selected_files.iter().any(|file| {
        matches!(
            file_kind_for_path(&file.relative_path),
            "System" | "Database"
        )
    });
    let has_dialogue = selected_files
        .iter()
        .any(|file| matches!(file_kind_for_path(&file.relative_path), "Maps" | "Events"));
    let has_existing_memory = project.files.iter().any(|file| {
        file.entries.iter().any(|entry| {
            !entry.translation.trim().is_empty() && entry.status != EntryStatus::Untranslated
        })
    });
    let has_mixed_source = project.source_lang.to_ascii_lowercase().contains("mixed")
        || selected_files.iter().any(|file| {
            file.entries.iter().any(|entry| {
                contains_ascii_words(&entry.original) && contains_non_ascii(&entry.original)
            })
        });
    let has_honorifics = selected_files.iter().any(|file| {
        file.entries
            .iter()
            .any(|entry| contains_honorific_hint(&entry.original))
    });

    if has_ui {
        notes.push(
            "Keep menu labels, commands and database names concise enough for RPG UI.".into(),
        );
    }
    if has_dialogue {
        notes.push("Favor natural in-game dialogue and keep speaker register consistent across nearby lines.".into());
    }
    if has_existing_memory {
        notes.push("Match established project terminology and phrasing when prior translations already exist.".into());
    }
    if has_mixed_source {
        notes.push(
            "Translate leftover English text into the target language while preserving established names and special terms."
                .into(),
        );
    }
    if has_honorifics {
        notes.push(
            "Treat honorifics, family-role terms and relationship language consistently across related scenes."
                .into(),
        );
    }
    if is_japanese_language(&project.source_lang) && is_spanish_language(&project.target_lang) {
        notes.push("For Japanese-to-Spanish RPG Maker text, prefer natural neutral Spanish over literal Japanese wording; e.g. render 学校の帰り道 as de vuelta de la escuela, and 一人称 UI settings as forma de referirse a si mismo.".into());
        notes.push("Preserve RPG Maker choice/plugin condition prefixes exactly, such as if(s[12]) and en(s[20]); translate only the visible choice label after those prefixes and control codes.".into());
        notes.push("Keep Spanish UI and choice labels concise so they fit RPG Maker windows, and avoid Japanese punctuation marks in Spanish output unless they are part of preserved names or source artifacts.".into());
        notes.push("For credits and staff rolls, translate role headings while preserving names, circles, brand names and supporter handles.".into());
    }

    let mut deduped = Vec::new();
    for note in notes {
        let trimmed = note.trim();
        if trimmed.is_empty() {
            continue;
        }
        if deduped
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            continue;
        }
        deduped.push(trimmed.to_string());
    }

    deduped
}

fn is_japanese_language(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower == "ja" || lower.contains("japanese") || lower.contains("japon")
}

fn is_spanish_language(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower == "es"
        || lower.contains("spanish")
        || lower.contains("español")
        || lower.contains("espanol")
}

fn collect_term_hints(
    project: &Project,
    selected_files: &[&TranslationFile],
) -> Vec<TranslationTermHint> {
    let selected_paths = selected_files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    let mut dedupe = project
        .glossary
        .iter()
        .map(|term| normalize_key(&term.source))
        .collect::<HashSet<_>>();
    let mut hints = Vec::new();

    for file in &project.files {
        let file_selected =
            selected_paths.is_empty() || selected_paths.contains(file.relative_path.as_str());
        if !file_selected {
            continue;
        }

        for entry in &file.entries {
            if !looks_like_term_entry(entry.id.as_str(), entry.file_context.as_str()) {
                continue;
            }

            let source = entry.original.trim();
            if source.is_empty() {
                continue;
            }

            let key = normalize_key(source);
            if dedupe.contains(&key) {
                continue;
            }

            let target = if entry.translation.trim().is_empty() {
                None
            } else {
                Some(entry.translation.trim().to_string())
            };
            let note = if target.is_some() {
                "Established project term"
            } else {
                "Important RPG term; keep it consistent if it recurs"
            };

            dedupe.insert(key);
            hints.push(TranslationTermHint {
                source: source.to_string(),
                target,
                note: note.into(),
            });
        }
    }

    hints
}

fn collect_prepared_translation_examples(
    project: &Project,
    selected_files: &[&TranslationFile],
    planner_cache: Option<&PlannerCacheData>,
    fingerprint: &str,
) -> Vec<PreparedTranslationExample> {
    if let Some(cache) = planner_cache {
        if cache.fingerprint == fingerprint {
            return cache.prepared_examples.clone();
        }
    }

    let selected_paths = selected_files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    let mut examples = Vec::new();

    for file in &project.files {
        let priority =
            if selected_paths.is_empty() || selected_paths.contains(file.relative_path.as_str()) {
                0usize
            } else {
                1usize
            };

        for entry in &file.entries {
            if entry.translation.trim().is_empty() || entry.status == EntryStatus::Untranslated {
                continue;
            }

            examples.push(PreparedTranslationExample {
                file_path: file.relative_path.clone(),
                file_kind: file_kind_for_path(&file.relative_path).to_string(),
                file_context: entry.file_context.clone(),
                original: entry.original.clone(),
                translation: entry.translation.clone(),
                notes: entry.notes.clone(),
                status_priority: status_priority(&entry.status),
                selection_priority: priority,
                translation_len: entry.translation.chars().count(),
                original_terms: lexical_terms_vec(&entry.original),
            });
        }
    }

    examples.sort_by(|a, b| {
        a.status_priority
            .cmp(&b.status_priority)
            .then(a.selection_priority.cmp(&b.selection_priority))
            .then(a.translation_len.cmp(&b.translation_len))
    });

    examples
}

fn reuse_cached_plan(
    selected_files: &[&TranslationFile],
    planner_cache: Option<&PlannerCacheData>,
    fingerprint: &str,
) -> Option<Vec<PlannedTranslationChunk>> {
    let cache = planner_cache?;
    if cache.fingerprint != fingerprint || cache.planned_chunks.is_empty() {
        return None;
    }

    let selected_paths = selected_files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    let pending_ids = selected_files
        .iter()
        .flat_map(|file| {
            file.entries
                .iter()
                .filter(|entry| entry_is_pending_translation(entry))
                .map(|entry| entry.id.as_str())
        })
        .collect::<HashSet<_>>();

    let mut chunks = cache
        .planned_chunks
        .iter()
        .filter(|chunk| selected_paths.contains(chunk.relative_path.as_str()))
        .filter_map(|chunk| {
            let entries = chunk
                .entries
                .iter()
                .filter(|entry| pending_ids.contains(entry.entry_id.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            if entries.is_empty() {
                return None;
            }

            let mut reused = chunk.clone();
            reused.entries = entries;
            reused.context.approximate_char_count = reused
                .entries
                .iter()
                .map(|entry| entry.text.chars().count())
                .sum();
            reused.planner_fingerprint = Some(fingerprint.to_string());
            Some(reused)
        })
        .collect::<Vec<_>>();

    let mut totals_by_file = HashMap::<String, usize>::new();
    for chunk in &chunks {
        *totals_by_file
            .entry(chunk.relative_path.clone())
            .or_insert(0) += 1;
    }

    let mut indexes_by_file = HashMap::<String, usize>::new();
    for chunk in &mut chunks {
        let index = indexes_by_file
            .entry(chunk.relative_path.clone())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        chunk.context.chunk_index = *index;
        chunk.context.total_chunks = *totals_by_file.get(&chunk.relative_path).unwrap_or(&1);
    }

    Some(chunks)
}

fn planner_cache_fingerprint_for_mode(
    project: &Project,
    file_paths: &[String],
    mode: ChunkPlanningMode,
    max_entries_per_chunk: usize,
) -> String {
    #[derive(Serialize)]
    struct SourceEntryFingerprint<'a> {
        id: &'a str,
        file_context: &'a str,
        original: &'a str,
        notes: &'a str,
    }

    #[derive(Serialize)]
    struct SourceFileFingerprint<'a> {
        relative_path: &'a str,
        entries: Vec<SourceEntryFingerprint<'a>>,
    }

    #[derive(Serialize)]
    struct Fingerprint<'a> {
        version: &'a str,
        project_id: &'a str,
        source_lang: &'a str,
        target_lang: &'a str,
        mode: &'a str,
        max_entries_per_chunk: usize,
        max_estimated_total_tokens: usize,
        selected_files: Vec<&'a str>,
        style_guide: &'a [String],
        glossary: Vec<(&'a str, &'a str, bool)>,
        source_files: Vec<SourceFileFingerprint<'a>>,
    }

    let mut selected_files = if file_paths.is_empty() {
        project
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>()
    } else {
        file_paths.iter().map(String::as_str).collect::<Vec<_>>()
    };
    selected_files.sort_unstable();

    let mode_label = match mode {
        ChunkPlanningMode::Standard => "standard",
        ChunkPlanningMode::OpenAi => "openai",
        ChunkPlanningMode::Codex => "codex",
        ChunkPlanningMode::ExternalAi => "external-ai",
    };
    let glossary = project
        .glossary
        .iter()
        .map(|term| {
            (
                term.source.as_str(),
                term.target.as_str(),
                term.case_sensitive,
            )
        })
        .collect::<Vec<_>>();
    let selected_file_set = selected_files.iter().copied().collect::<HashSet<_>>();
    let source_files = project
        .files
        .iter()
        .filter(|file| selected_file_set.contains(file.relative_path.as_str()))
        .map(|file| SourceFileFingerprint {
            relative_path: file.relative_path.as_str(),
            entries: file
                .entries
                .iter()
                .map(|entry| SourceEntryFingerprint {
                    id: entry.id.as_str(),
                    file_context: entry.file_context.as_str(),
                    original: entry.original.as_str(),
                    notes: entry.notes.as_str(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    let fingerprint_payload = serde_json::to_string(&Fingerprint {
        version: "planner-cache-v3-source-plan",
        project_id: project.id.as_str(),
        source_lang: project.source_lang.as_str(),
        target_lang: project.target_lang.as_str(),
        mode: mode_label,
        max_entries_per_chunk,
        max_estimated_total_tokens: MAX_ESTIMATED_TOTAL_TOKENS,
        selected_files,
        style_guide: &project.style_guide,
        glossary,
        source_files,
    })
    .expect("planner fingerprint serialization should not fail");

    format!(
        "planner-cache-v3-source-plan:{}",
        stable_hash_hex(fingerprint_payload.as_bytes())
    )
}

fn stable_hash_hex(bytes: &[u8]) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn max_entries_for_mode(mode: ChunkPlanningMode, max_entries: Option<usize>) -> usize {
    match mode {
        ChunkPlanningMode::Standard => DEFAULT_MAX_ENTRIES_PER_CHUNK,
        ChunkPlanningMode::OpenAi | ChunkPlanningMode::Codex => LLM_MAX_ENTRIES_PER_CHUNK,
        ChunkPlanningMode::ExternalAi => max_entries
            .unwrap_or(EXTERNAL_AI_DEFAULT_MAX_ENTRIES_PER_CHUNK)
            .max(1),
    }
}

fn chunk_requests(
    project: &Project,
    file_path: &str,
    file_kind: &str,
    entries: &[TranslationRequest],
    style_guide: &[String],
    term_hints: &[TranslationTermHint],
    translation_examples: &[PreparedTranslationExample],
    mode: ChunkPlanningMode,
    max_entries_per_chunk: usize,
) -> Vec<Vec<TranslationRequest>> {
    if entries.is_empty() {
        return Vec::new();
    }

    if mode == ChunkPlanningMode::Standard {
        return entries
            .chunks(DEFAULT_MAX_ENTRIES_PER_CHUNK)
            .map(|chunk| chunk.to_vec())
            .collect();
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    for entry in entries {
        let exceeds_entry_limit = current.len() >= max_entries_per_chunk;
        let exceeds_prompt_budget = !current.is_empty()
            && chunk_would_exceed_budget(
                project,
                file_path,
                file_kind,
                &current,
                entry,
                style_guide,
                term_hints,
                translation_examples,
                mode,
            );

        if exceeds_entry_limit || exceeds_prompt_budget {
            chunks.push(current);
            current = Vec::new();
        }

        current.push(entry.clone());
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn chunk_would_exceed_budget(
    project: &Project,
    file_path: &str,
    file_kind: &str,
    current: &[TranslationRequest],
    next_entry: &TranslationRequest,
    style_guide: &[String],
    term_hints: &[TranslationTermHint],
    translation_examples: &[PreparedTranslationExample],
    mode: ChunkPlanningMode,
) -> bool {
    let mut candidate = current.to_vec();
    candidate.push(next_entry.clone());

    let (_, estimated_tokens) = build_context_for_chunk(
        project,
        file_path,
        file_kind,
        &candidate,
        style_guide,
        term_hints,
        translation_examples,
        None,
        None,
        1,
        1,
        mode,
    );

    estimated_tokens > MAX_ESTIMATED_TOTAL_TOKENS
}

fn build_context_for_chunk(
    project: &Project,
    file_path: &str,
    file_kind: &str,
    entries: &[TranslationRequest],
    style_guide: &[String],
    term_hints: &[TranslationTermHint],
    translation_examples: &[PreparedTranslationExample],
    previous_chunk: Option<&[TranslationRequest]>,
    next_chunk: Option<&[TranslationRequest]>,
    chunk_index: usize,
    total_chunks: usize,
    mode: ChunkPlanningMode,
) -> (TranslationBatchContext, usize) {
    let approximate_char_count = entries.iter().map(|entry| entry.text.chars().count()).sum();
    let mut context = TranslationBatchContext {
        project_name: project.name.clone(),
        file_path: file_path.to_string(),
        file_kind: file_kind.to_string(),
        chunk_index,
        total_chunks,
        approximate_char_count,
        style_guide: style_guide.to_vec(),
        term_hints: select_relevant_term_hints(term_hints, entries),
        translation_examples: select_relevant_examples(
            translation_examples,
            file_path,
            file_kind,
            entries,
        ),
        file_context_hints: collect_file_context_hints(entries),
        previous_chunk_hints: collect_neighbor_hints(previous_chunk),
        next_chunk_hints: collect_neighbor_hints(next_chunk),
    };

    let estimated_tokens = trim_context_to_budget(project, entries, mode, &mut context);
    (context, estimated_tokens)
}

fn trim_context_to_budget(
    project: &Project,
    entries: &[TranslationRequest],
    mode: ChunkPlanningMode,
    context: &mut TranslationBatchContext,
) -> usize {
    if mode == ChunkPlanningMode::Standard {
        return 0;
    }

    let mut estimated = estimated_prompt_tokens(project, mode, entries, context);
    while estimated > MAX_ESTIMATED_TOTAL_TOKENS {
        if context.translation_examples.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        if context.term_hints.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        if context.file_context_hints.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        if context.next_chunk_hints.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        if context.previous_chunk_hints.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        if context.style_guide.pop().is_some() {
            estimated = estimated_prompt_tokens(project, mode, entries, context);
            continue;
        }
        break;
    }

    estimated
}

fn estimated_prompt_tokens(
    project: &Project,
    mode: ChunkPlanningMode,
    entries: &[TranslationRequest],
    context: &TranslationBatchContext,
) -> usize {
    match mode {
        ChunkPlanningMode::Standard => 0,
        ChunkPlanningMode::OpenAi => OpenAiProvider::estimate_total_tokens(
            &project.source_lang,
            &project.target_lang,
            entries,
            &project.glossary,
            context,
        ),
        ChunkPlanningMode::Codex => CodexProvider::estimate_total_tokens(
            &project.source_lang,
            &project.target_lang,
            entries,
            &project.glossary,
            context,
        ),
        ChunkPlanningMode::ExternalAi => {
            ai_batch::estimate_ai_batch_prompt_tokens(project, entries, context)
        }
    }
}

fn collect_file_context_hints(entries: &[TranslationRequest]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut hints = Vec::new();

    for entry in entries {
        let normalized = entry.file_context.trim();
        if normalized.is_empty() || !seen.insert(normalized.to_string()) {
            continue;
        }
        hints.push(normalized.to_string());
        if hints.len() >= MAX_FILE_CONTEXT_HINTS {
            break;
        }
    }

    hints
}

fn collect_neighbor_hints(entries: Option<&[TranslationRequest]>) -> Vec<String> {
    let Some(entries) = entries else {
        return Vec::new();
    };

    let mut hints = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries.iter().take(MAX_NEIGHBOR_HINTS) {
        let hint = format!(
            "{} | {}",
            compact_context_hint(&entry.file_context),
            compact_text_preview(&entry.text, 96)
        );
        if seen.insert(hint.clone()) {
            hints.push(hint);
        }
    }
    hints
}

fn select_relevant_term_hints(
    term_hints: &[TranslationTermHint],
    chunk: &[TranslationRequest],
) -> Vec<TranslationTermHint> {
    if term_hints.is_empty() {
        return Vec::new();
    }

    let chunk_blob = chunk
        .iter()
        .map(|entry| format!("{} {}", entry.file_context, entry.text))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();

    let mut matched = Vec::new();
    let mut fallback = Vec::new();

    for hint in term_hints {
        let source = hint.source.to_ascii_lowercase();
        let target = hint
            .target
            .as_deref()
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if chunk_blob.contains(&source) || (!target.is_empty() && chunk_blob.contains(&target)) {
            matched.push(hint.clone());
        } else if hint.target.is_some() {
            fallback.push(hint.clone());
        }
    }

    matched.extend(
        fallback
            .into_iter()
            .take(MAX_TERM_HINTS.saturating_sub(matched.len())),
    );
    matched.truncate(MAX_TERM_HINTS);
    matched
}

fn select_relevant_examples(
    examples: &[PreparedTranslationExample],
    file_path: &str,
    file_kind: &str,
    chunk: &[TranslationRequest],
) -> Vec<TranslationExample> {
    if examples.is_empty() {
        return Vec::new();
    }

    let chunk_terms = lexical_terms_set(
        &chunk
            .iter()
            .map(|entry| format!("{} {}", entry.file_context, entry.text))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let mut best = Vec::<(usize, usize, PreparedTranslationExample)>::new();

    for example in examples {
        if example.translation.trim().is_empty() {
            continue;
        }

        let mut score = 0usize;
        if example.file_path == file_path {
            score += 100;
        }
        if example.file_kind == file_kind {
            score += 25;
        }
        score += overlap_score_vec(&chunk_terms, &example.original_terms);
        let candidate = (usize::MAX - score, example.translation_len, example.clone());

        let insert_at = best
            .iter()
            .position(|current| {
                candidate.0 < current.0 || (candidate.0 == current.0 && candidate.1 < current.1)
            })
            .unwrap_or(best.len());
        if insert_at < MAX_TRANSLATION_EXAMPLES {
            best.insert(insert_at, candidate);
            if best.len() > MAX_TRANSLATION_EXAMPLES {
                best.pop();
            }
        } else if best.len() < MAX_TRANSLATION_EXAMPLES {
            best.push(candidate);
        }
    }

    best.into_iter()
        .map(|(_, _, example)| TranslationExample {
            file_path: example.file_path,
            file_context: example.file_context,
            original: example.original,
            translation: example.translation,
            notes: example.notes,
        })
        .collect()
}

fn status_priority(status: &EntryStatus) -> usize {
    match status {
        EntryStatus::Final => 0,
        EntryStatus::Reviewed => 1,
        EntryStatus::Draft => 2,
        EntryStatus::Untranslated => 3,
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn looks_like_term_entry(entry_id: &str, file_context: &str) -> bool {
    let id = entry_id.to_ascii_lowercase();
    let context = file_context.to_ascii_lowercase();
    id.contains(":name:")
        || id.contains(":nickname:")
        || id.contains("displayname")
        || context.contains("> name")
        || context.contains("displayname")
        || context.contains("nickname")
}

fn file_kind_for_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("system.json") {
        "System"
    } else if lower.ends_with("js/plugins.js") {
        "Plugins"
    } else if lower.contains("commonevents") || lower.contains("troops") {
        "Events"
    } else if lower.contains("\\map") || lower.contains("/map") || lower.contains("map") {
        "Maps"
    } else if lower.ends_with(".json") {
        "Database"
    } else {
        "Other"
    }
}

fn contains_honorific_hint(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("-san")
        || normalized.contains("-chan")
        || normalized.contains("-kun")
        || text.contains("お姉")
        || text.contains("兄")
        || text.contains("姉")
}

fn contains_ascii_words(text: &str) -> bool {
    text.split_whitespace()
        .any(|token| token.chars().all(|ch| ch.is_ascii_alphabetic()) && token.len() > 2)
}

fn contains_non_ascii(text: &str) -> bool {
    text.chars().any(|ch| !ch.is_ascii())
}

fn lexical_terms_vec(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|token| {
            let trimmed = token.trim();
            if trimmed.len() < 3 {
                return None;
            }
            Some(trimmed.to_ascii_lowercase())
        })
        .collect()
}

fn lexical_terms_set(text: &str) -> HashSet<String> {
    lexical_terms_vec(text).into_iter().collect()
}

fn overlap_score_vec(left: &HashSet<String>, right: &[String]) -> usize {
    right
        .iter()
        .filter(|token| left.contains(token.as_str()))
        .count()
}

fn compact_context_hint(value: &str) -> String {
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

fn compact_text_preview(value: &str, max_chars: usize) -> String {
    let normalized = value.replace('\n', " / ").replace('\r', "");
    let compact = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, GlossaryTerm, TranslationEntry};

    fn make_entry(
        id: &str,
        file_context: &str,
        original: &str,
        translation: &str,
        status: EntryStatus,
    ) -> TranslationEntry {
        TranslationEntry {
            id: id.into(),
            file_context: file_context.into(),
            original: original.into(),
            translation: translation.into(),
            status,
            notes: String::new(),
        }
    }

    fn make_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "Test".into(),
            source_lang: "mixed Japanese and English".into(),
            target_lang: "es".into(),
            engine: Engine::Mz,
            game_dir: "D:/Games/Test".into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![
                        make_entry(
                            "System:terms:attack",
                            "System > terms > attack",
                            "Attack",
                            "",
                            EntryStatus::Untranslated,
                        ),
                        make_entry(
                            "Actors:name:1",
                            "Actors > #1 > name",
                            "勇者",
                            "Heroe",
                            EntryStatus::Reviewed,
                        ),
                    ],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![
                        make_entry(
                            "Map001:line:1",
                            "Map001 > Event 1 > Show Text",
                            "Hello there",
                            "",
                            EntryStatus::Untranslated,
                        ),
                        make_entry(
                            "Map001:line:2",
                            "Map001 > Event 1 > Show Text",
                            "My name is 勇者",
                            "",
                            EntryStatus::Untranslated,
                        ),
                        make_entry(
                            "Map001:line:3",
                            "Map001 > Event 1 > Show Text",
                            "Welcome back",
                            "Bienvenido de nuevo",
                            EntryStatus::Final,
                        ),
                    ],
                },
            ],
            glossary: vec![GlossaryTerm {
                source: "勇者".into(),
                target: "Heroe".into(),
                case_sensitive: true,
            }],
            style_guide: vec!["Use informal neutral Spanish.".into()],
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn make_oversized_budget_project() -> Project {
        let mut project = make_project();
        project.files = vec![TranslationFile {
            relative_path: "data/System.json".into(),
            entries: vec![make_entry(
                "System:oversized",
                "System > oversized",
                &"A".repeat(18_000),
                "",
                EntryStatus::Untranslated,
            )],
        }];
        project.glossary = (0..320)
            .map(|index| GlossaryTerm {
                source: format!("VeryLongSourceTerm{index}_{}", "x".repeat(72)),
                target: format!("VeryLongTargetTerm{index}_{}", "y".repeat(72)),
                case_sensitive: true,
            })
            .collect();
        project.style_guide = (0..24)
            .map(|index| format!("Long rule {index}: {}", "z".repeat(320)))
            .collect();
        project
    }

    #[test]
    fn test_build_translation_plan_uses_contextual_chunks() {
        let project = make_project();
        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);

        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0].context.file_kind, "System");
        assert_eq!(plan[1].context.file_kind, "Maps");
        assert!(!plan[1].context.translation_examples.is_empty());
        assert!(plan[1]
            .context
            .file_context_hints
            .iter()
            .any(|hint| hint.contains("Show Text")));
    }

    #[test]
    fn test_planner_fingerprint_ignores_translation_progress() {
        let mut project = make_project();
        let initial = planner_cache_fingerprint(&project, &[], &ProviderType::Codex);

        project.updated_at = "2026-01-02T00:00:00Z".into();
        project.files[1].entries[0].translation = "Hola".into();
        project.files[1].entries[0].status = EntryStatus::Draft;

        assert_eq!(
            initial,
            planner_cache_fingerprint(&project, &[], &ProviderType::Codex)
        );

        project.files[1].entries[1].original = "Changed source".into();

        assert_ne!(
            initial,
            planner_cache_fingerprint(&project, &[], &ProviderType::Codex)
        );
    }

    #[test]
    fn test_planner_cache_reuses_chunks_and_filters_completed_entries() {
        let project = make_project();
        let mut first_snapshots = Vec::new();
        let first = build_translation_plan_with_cache_and_progress(
            &project,
            &[],
            &ProviderType::Codex,
            None,
            &mut |snapshot| first_snapshots.push(snapshot),
        );
        assert!(!first.planner_cache.planned_chunks.is_empty());

        let mut resumed = project.clone();
        resumed.updated_at = "2026-01-02T00:00:00Z".into();
        resumed.files[0].entries[0].translation = "Atacar".into();
        resumed.files[0].entries[0].status = EntryStatus::Draft;

        let mut second_snapshots = Vec::new();
        let second = build_translation_plan_with_cache_and_progress(
            &resumed,
            &[],
            &ProviderType::Codex,
            Some(&first.planner_cache),
            &mut |snapshot| second_snapshots.push(snapshot),
        );
        let planned_ids = second
            .plan
            .iter()
            .flat_map(|chunk| chunk.entries.iter().map(|entry| entry.entry_id.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(second_snapshots.len(), 1);
        assert_eq!(second_snapshots[0].files_done, resumed.files.len());
        assert!(!planned_ids.contains(&"System:terms:attack"));
        assert!(planned_ids.contains(&"Map001:line:1"));
        assert!(planned_ids.contains(&"Map001:line:2"));
    }

    #[test]
    fn test_build_translation_plan_preserves_saved_style_guide_rules() {
        let mut project = make_project();
        project.style_guide = vec![
            "Rule 1".into(),
            "Rule 2".into(),
            "Rule 3".into(),
            "Rule 4".into(),
            "Rule 5".into(),
            "Rule 6".into(),
        ];

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);

        assert!(plan.iter().all(|chunk| chunk
            .context
            .style_guide
            .iter()
            .any(|rule| rule == "Rule 6")));
    }

    #[test]
    fn test_build_translation_plan_adds_japanese_to_spanish_style_rules() {
        let mut project = make_project();
        project.source_lang = "ja".into();
        project.target_lang = "es".into();
        project.files[1].entries.push(make_entry(
            "Map003:25:0:choice:47:1",
            "Map003 > Event 25 > choice #9 option 2",
            "if(s[211])夫に露出していたことを打ち明ける",
            "",
            EntryStatus::Untranslated,
        ));

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);
        let style_blob = plan
            .iter()
            .flat_map(|chunk| chunk.context.style_guide.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");

        assert!(style_blob.contains("de vuelta de la escuela"));
        assert!(style_blob.contains("if(s[12])"));
        assert!(style_blob.contains("choice labels concise"));
    }

    #[test]
    fn test_build_translation_plan_trims_context_to_fit_prompt_budget() {
        let mut project = make_project();
        project.style_guide = (1..=96)
            .map(|index| format!("Long rule {index}: {}", "x".repeat(720)))
            .collect();

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);
        let map_chunk = plan
            .iter()
            .find(|chunk| chunk.relative_path == "data/Map001.json")
            .unwrap();

        assert!(
            map_chunk.context.style_guide.len() < project.style_guide.len(),
            "style-guide context should be trimmed when the prompt budget would overflow"
        );
        assert!(
            estimated_prompt_tokens(
                &project,
                ChunkPlanningMode::Codex,
                &map_chunk.entries,
                &map_chunk.context
            ) <= MAX_ESTIMATED_TOTAL_TOKENS
        );
    }

    #[test]
    fn test_build_translation_plan_keeps_late_relevant_term_hints() {
        let mut project = make_project();
        let filler_terms = (0..32).map(|index| {
            make_entry(
                &format!("Actors:name:filler:{index}"),
                &format!("Actors > filler #{index} > name"),
                &format!("RelicName{index}"),
                "",
                EntryStatus::Untranslated,
            )
        });
        project.files[0].entries.extend(filler_terms);
        project.files[1].entries.push(make_entry(
            "Map001:line:4",
            "Map001 > Event 1 > Show Text",
            "RelicName31 must stay consistent",
            "",
            EntryStatus::Untranslated,
        ));

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);
        let map_chunk = plan
            .iter()
            .find(|chunk| chunk.relative_path == "data/Map001.json")
            .unwrap();

        assert!(map_chunk
            .context
            .term_hints
            .iter()
            .any(|hint| hint.source == "RelicName31"));
    }

    #[test]
    fn test_build_translation_plan_keeps_late_relevant_examples() {
        let mut project = make_project();
        for index in 0..32 {
            project.files[0].entries.push(make_entry(
                &format!("System:example:{index}"),
                "System > examples",
                &format!("Unrelated example {index}"),
                &format!("Ejemplo no relacionado {index}"),
                EntryStatus::Final,
            ));
        }
        project.files[0].entries.push(make_entry(
            "System:example:relic",
            "System > lore",
            "Relic of Dawn",
            "Reliquia del Alba",
            EntryStatus::Final,
        ));
        project.files[1].entries.push(make_entry(
            "Map001:line:4",
            "Map001 > Event 2 > Show Text",
            "The Relic of Dawn is here",
            "",
            EntryStatus::Untranslated,
        ));

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);
        let map_chunk = plan
            .iter()
            .find(|chunk| chunk.relative_path == "data/Map001.json")
            .unwrap();

        assert!(map_chunk
            .context
            .translation_examples
            .iter()
            .any(|example| example.original == "Relic of Dawn"));
    }

    #[test]
    fn test_build_translation_plan_skips_blank_original_entries() {
        let mut project = make_project();
        project.files[1].entries.push(make_entry(
            "Map001:line:blank",
            "Map001 > Event 3 > Show Text",
            "",
            "",
            EntryStatus::Untranslated,
        ));

        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);
        let all_entry_ids = plan
            .iter()
            .flat_map(|chunk| chunk.entries.iter().map(|entry| entry.entry_id.as_str()))
            .collect::<Vec<_>>();

        assert!(!all_entry_ids.contains(&"Map001:line:blank"));
    }

    #[test]
    fn test_translation_plan_reports_unshrinkable_oversized_codex_chunk() {
        let project = make_oversized_budget_project();
        let plan = build_translation_plan(&project, &[], &ProviderType::Codex);

        let error = ensure_translation_plan_within_budget(&project, &plan, &ProviderType::Codex)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("exceeds the estimated prompt budget"));
        assert!(error.to_string().contains("System:oversized"));
    }
}

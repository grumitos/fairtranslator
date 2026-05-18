use std::collections::HashSet;

use serde::Serialize;

use crate::models::{EntryStatus, Project, TranslationEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverageSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverageIssueKind {
    MissingTranslation,
    SourceScriptResidue,
    UnchangedText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageIssue {
    pub severity: CoverageSeverity,
    pub kind: CoverageIssueKind,
    pub file_path: String,
    pub entry_id: String,
    pub message: String,
    pub original_preview: String,
    pub translation_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCoverageReport {
    pub project_name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub total_entries: usize,
    pub skipped_blank_entries: usize,
    pub translated_entries: usize,
    pub missing_translations: usize,
    pub source_script_residue: usize,
    pub unchanged_translations: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub complete: bool,
    pub issues: Vec<CoverageIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageOptions {
    pub strict_unchanged: bool,
    pub max_issues: usize,
}

impl Default for CoverageOptions {
    fn default() -> Self {
        Self {
            strict_unchanged: false,
            max_issues: 100,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRepairResult {
    pub marked_entries: usize,
}

pub fn mark_project_coverage_issues_for_repair(
    project: &mut Project,
    report: &TranslationCoverageReport,
) -> CoverageRepairResult {
    let issue_ids = report
        .issues
        .iter()
        .filter(|issue| {
            matches!(
                issue.kind,
                CoverageIssueKind::MissingTranslation
                    | CoverageIssueKind::SourceScriptResidue
                    | CoverageIssueKind::UnchangedText
            )
        })
        .map(|issue| (issue.file_path.as_str(), issue.entry_id.as_str()))
        .collect::<HashSet<_>>();

    if issue_ids.is_empty() {
        return CoverageRepairResult::default();
    }

    let repair_note = "Reparación de cobertura: la salida anterior no pasó la QA del idioma destino. Traduce el texto visible al idioma destino; no devuelvas texto idéntico del idioma fuente salvo que sea un código protegido, marcador, acrónimo de RPG o nombre propio claro.";
    let mut marked_entries = 0usize;

    for file in &mut project.files {
        for entry in &mut file.entries {
            if !issue_ids.contains(&(file.relative_path.as_str(), entry.id.as_str())) {
                continue;
            }

            if !entry.original.trim().is_empty() {
                entry.status = EntryStatus::Untranslated;
                entry.translation.clear();
                if !entry.notes.contains(repair_note) {
                    if !entry.notes.trim().is_empty() {
                        entry.notes.push('\n');
                    }
                    entry.notes.push_str(repair_note);
                }
                marked_entries += 1;
            }
        }
    }

    CoverageRepairResult { marked_entries }
}

pub fn validate_project_translation_coverage(
    project: &Project,
    options: CoverageOptions,
) -> TranslationCoverageReport {
    let mut total_entries = 0usize;
    let mut skipped_blank_entries = 0usize;
    let mut translated_entries = 0usize;
    let mut missing_translations = 0usize;
    let mut source_script_residue = 0usize;
    let mut unchanged_translations = 0usize;
    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let mut issues = Vec::new();

    for file in &project.files {
        for entry in &file.entries {
            if entry.original.trim().is_empty() {
                skipped_blank_entries += 1;
                continue;
            }

            total_entries += 1;
            let translation = entry.translation.trim();
            let is_translated =
                entry.status != EntryStatus::Untranslated && !translation.is_empty();

            if is_translated {
                translated_entries += 1;
            } else {
                missing_translations += 1;
                push_issue(
                    &mut issues,
                    options.max_issues,
                    CoverageIssue {
                        severity: CoverageSeverity::Error,
                        kind: CoverageIssueKind::MissingTranslation,
                        file_path: file.relative_path.clone(),
                        entry_id: entry.id.clone(),
                        message: "Entrada sin traducir o traducción vacía.".into(),
                        original_preview: preview(&entry.original),
                        translation_preview: preview(&entry.translation),
                    },
                );
                error_count += 1;
                continue;
            }

            if has_source_script_residue(project, entry, translation) {
                source_script_residue += 1;
                push_issue(
                    &mut issues,
                    options.max_issues,
                    CoverageIssue {
                        severity: CoverageSeverity::Error,
                        kind: CoverageIssueKind::SourceScriptResidue,
                        file_path: file.relative_path.clone(),
                        entry_id: entry.id.clone(),
                        message: "La traducción conserva caracteres del idioma fuente.".into(),
                        original_preview: preview(&entry.original),
                        translation_preview: preview(translation),
                    },
                );
                error_count += 1;
            }

            if is_unchanged_translatable_text(file.relative_path.as_str(), entry, translation) {
                unchanged_translations += 1;
                let severity = if options.strict_unchanged {
                    CoverageSeverity::Error
                } else {
                    CoverageSeverity::Warning
                };
                push_issue(
                    &mut issues,
                    options.max_issues,
                    CoverageIssue {
                        severity,
                        kind: CoverageIssueKind::UnchangedText,
                        file_path: file.relative_path.clone(),
                        entry_id: entry.id.clone(),
                        message: "La traducción no cambió; confirma que sea nombre/código.".into(),
                        original_preview: preview(&entry.original),
                        translation_preview: preview(translation),
                    },
                );
                match severity {
                    CoverageSeverity::Error => error_count += 1,
                    CoverageSeverity::Warning => warning_count += 1,
                }
            }
        }
    }

    TranslationCoverageReport {
        project_name: project.name.clone(),
        source_lang: project.source_lang.clone(),
        target_lang: project.target_lang.clone(),
        total_entries,
        skipped_blank_entries,
        translated_entries,
        missing_translations,
        source_script_residue,
        unchanged_translations,
        error_count,
        warning_count,
        complete: error_count == 0,
        issues,
    }
}

fn push_issue(issues: &mut Vec<CoverageIssue>, max_issues: usize, issue: CoverageIssue) {
    if issues.len() < max_issues {
        issues.push(issue);
    }
}

fn has_source_script_residue(
    project: &Project,
    entry: &TranslationEntry,
    translation: &str,
) -> bool {
    if is_japanese_language(&project.target_lang) {
        return false;
    }

    (is_japanese_language(&project.source_lang) || contains_japanese_script(&entry.original))
        && contains_japanese_script(translation)
}

fn is_unchanged_translatable_text(
    file_path: &str,
    entry: &TranslationEntry,
    translation: &str,
) -> bool {
    let original = normalize_visible(&entry.original);
    let translation = normalize_visible(translation);
    if original.is_empty() || original != translation {
        return false;
    }

    if is_allowed_unchanged_exception(file_path, entry, &original) {
        return false;
    }

    let alphabetic_count = original.chars().filter(|ch| ch.is_alphabetic()).count();
    alphabetic_count >= 3
}

fn is_allowed_unchanged_exception(
    file_path: &str,
    entry: &TranslationEntry,
    visible_text: &str,
) -> bool {
    if is_game_title_entry(file_path, entry) || is_actor_identity_entry(entry) {
        return true;
    }

    is_common_rpg_acronym(visible_text)
        || is_shared_spanish_term(visible_text)
        || is_bracketed_proper_name_cue(visible_text)
        || is_enemy_or_troop_proper_name_list(entry, visible_text)
}

fn is_game_title_entry(file_path: &str, entry: &TranslationEntry) -> bool {
    entry.id.eq_ignore_ascii_case("System:gameTitle")
        || entry.id.eq_ignore_ascii_case("index.html:title")
        || (file_path.eq_ignore_ascii_case("index.html")
            && entry.file_context.to_ascii_lowercase().contains("title"))
}

fn is_actor_identity_entry(entry: &TranslationEntry) -> bool {
    let id = entry.id.to_ascii_lowercase();
    id.starts_with("actors:name:")
        || id.starts_with("actors:nickname:")
        || entry.file_context.to_ascii_lowercase().contains("actors >")
            && (entry.file_context.to_ascii_lowercase().contains("> name")
                || entry
                    .file_context
                    .to_ascii_lowercase()
                    .contains("> nickname"))
}

fn is_common_rpg_acronym(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();

    matches!(
        normalized.as_str(),
        "HP" | "MP"
            | "TP"
            | "EXP"
            | "XP"
            | "LV"
            | "LVL"
            | "ATK"
            | "DEF"
            | "MAT"
            | "MDF"
            | "AGI"
            | "LUK"
    )
}

fn is_shared_spanish_term(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "elixir" | "perfume")
}

fn is_bracketed_proper_name_cue(value: &str) -> bool {
    let trimmed = value.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return false;
    }

    let inner = trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .trim_matches(|ch: char| matches!(ch, '.' | '!' | '?' | '…'))
        .trim();
    if inner.is_empty() || inner.split_whitespace().count() > 2 {
        return false;
    }

    inner
        .split_whitespace()
        .all(|token| token.chars().next().is_some_and(|ch| ch.is_uppercase()))
}

fn is_enemy_or_troop_proper_name_list(entry: &TranslationEntry, value: &str) -> bool {
    let id = entry.id.to_ascii_lowercase();
    if !(id.starts_with("enemies:name:") || id.starts_with("troops:name:")) {
        return false;
    }

    value
        .split(',')
        .flat_map(|part| part.split_whitespace())
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphabetic()))
        .filter(|token| !token.is_empty())
        .all(is_likely_proper_name_token)
}

fn is_likely_proper_name_token(token: &str) -> bool {
    let normalized = token.to_ascii_lowercase();
    if normalized.len() < 2 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }

    !is_exact_english_residue(&normalized) && english_morpheme_score(&normalized) < 2
}

fn is_exact_english_residue(value: &str) -> bool {
    matches!(
        value,
        "attack"
            | "armor"
            | "axe"
            | "blue"
            | "bender"
            | "brigand"
            | "comet"
            | "crony"
            | "death"
            | "defend"
            | "demon"
            | "drier"
            | "elixir"
            | "faint"
            | "fire"
            | "fireball"
            | "fruit"
            | "ghost"
            | "good"
            | "heal"
            | "hospital"
            | "hound"
            | "ice"
            | "imp"
            | "perfume"
            | "pork"
            | "reserved"
            | "restoration"
            | "shield"
            | "shovel"
            | "sidewinder"
            | "smite"
            | "space"
            | "storm"
            | "sword"
            | "tornado"
            | "zombie"
    )
}

fn english_morpheme_score(value: &str) -> usize {
    const MORPHEMES: &[&str] = &[
        "armor", "attack", "ball", "blade", "boat", "bow", "break", "burst", "cave", "death",
        "demon", "dragon", "evil", "fire", "fish", "fork", "guard", "hat", "heal", "hell", "hound",
        "horror", "ice", "lord", "magic", "master", "power", "shot", "shield", "slash", "storm",
        "strike", "swipe", "sword", "twin", "venom", "whirl", "wizard", "work",
    ];

    MORPHEMES
        .iter()
        .filter(|morpheme| value.contains(**morpheme))
        .count()
}

fn normalize_visible(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn contains_japanese_script(value: &str) -> bool {
    value.chars().any(|ch| {
        matches!(
            ch as u32,
            0x3040..=0x309F
                | 0x30A0..=0x30FF
                | 0x31F0..=0x31FF
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0xFF66..=0xFF9D
        )
    })
}

fn is_japanese_language(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized == "ja" || normalized == "jp" || normalized.contains("japanese")
}

fn preview(value: &str) -> String {
    const MAX_CHARS: usize = 96;
    let mut chars = value.chars();
    let preview = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, TranslationFile};

    fn entry(id: &str, original: &str, translation: &str, status: EntryStatus) -> TranslationEntry {
        TranslationEntry {
            id: id.into(),
            file_context: "ctx".into(),
            original: original.into(),
            translation: translation.into(),
            status,
            notes: String::new(),
        }
    }

    fn project(entries: Vec<TranslationEntry>) -> Project {
        Project {
            id: "project".into(),
            name: "Coverage".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\Coverage".into(),
            files: vec![TranslationFile {
                relative_path: "data/Map001.json".into(),
                entries,
            }],
            glossary: Vec::new(),
            style_guide: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn complete_when_every_entry_has_target_text() {
        let project = project(vec![entry(
            "Map001:1:0:text:0",
            "こんにちは",
            "Hola",
            EntryStatus::Reviewed,
        )]);

        let report = validate_project_translation_coverage(&project, CoverageOptions::default());

        assert!(report.complete);
        assert_eq!(report.total_entries, 1);
        assert_eq!(report.translated_entries, 1);
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn flags_untranslated_entries_as_errors() {
        let project = project(vec![entry(
            "Map001:1:0:text:0",
            "Hello",
            "",
            EntryStatus::Untranslated,
        )]);

        let report = validate_project_translation_coverage(&project, CoverageOptions::default());

        assert!(!report.complete);
        assert_eq!(report.missing_translations, 1);
        assert_eq!(report.error_count, 1);
        assert_eq!(report.issues[0].kind, CoverageIssueKind::MissingTranslation);
    }

    #[test]
    fn skips_blank_original_entries() {
        let project = project(vec![entry(
            "Map001:1:0:text:0",
            "",
            "",
            EntryStatus::Untranslated,
        )]);

        let report = validate_project_translation_coverage(&project, CoverageOptions::default());

        assert!(report.complete);
        assert_eq!(report.total_entries, 0);
        assert_eq!(report.skipped_blank_entries, 1);
        assert_eq!(report.error_count, 0);
    }

    #[test]
    fn flags_japanese_residue_for_spanish_targets() {
        let project = project(vec![entry(
            "Map001:1:0:text:0",
            "こんにちは",
            "Hola こんにちは",
            EntryStatus::Reviewed,
        )]);

        let report = validate_project_translation_coverage(&project, CoverageOptions::default());

        assert!(!report.complete);
        assert_eq!(report.source_script_residue, 1);
        assert_eq!(
            report.issues[0].kind,
            CoverageIssueKind::SourceScriptResidue
        );
    }

    #[test]
    fn unchanged_text_warns_by_default_and_errors_in_strict_mode() {
        let project = project(vec![entry(
            "Skills:name:1",
            "Fire Hound",
            "Fire Hound",
            EntryStatus::Reviewed,
        )]);

        let report = validate_project_translation_coverage(&project, CoverageOptions::default());
        assert!(report.complete);
        assert_eq!(report.warning_count, 1);
        assert_eq!(report.issues[0].severity, CoverageSeverity::Warning);

        let strict = validate_project_translation_coverage(
            &project,
            CoverageOptions {
                strict_unchanged: true,
                ..CoverageOptions::default()
            },
        );
        assert!(!strict.complete);
        assert_eq!(strict.error_count, 1);
        assert_eq!(strict.issues[0].severity, CoverageSeverity::Error);
    }

    #[test]
    fn unchanged_allowed_title_actor_and_rpg_acronym_do_not_fail_strict_mode() {
        let project = project(vec![
            entry(
                "System:gameTitle",
                "Mogu Man",
                "Mogu Man",
                EntryStatus::Reviewed,
            ),
            entry(
                "Actors:name:1",
                "Mogu Man",
                "Mogu Man",
                EntryStatus::Reviewed,
            ),
            entry("System:terms.basic.8", "EXP", "EXP", EntryStatus::Reviewed),
            entry(
                "Map010:3:0:text:205",
                "[Nathan.]",
                "[Nathan.]",
                EntryStatus::Reviewed,
            ),
            entry("Items:name:4", "Perfume", "Perfume", EntryStatus::Reviewed),
            entry("Items:name:7", "Elixir", "Elixir", EntryStatus::Reviewed),
        ]);

        let strict = validate_project_translation_coverage(
            &project,
            CoverageOptions {
                strict_unchanged: true,
                ..CoverageOptions::default()
            },
        );

        assert!(strict.complete);
        assert_eq!(strict.unchanged_translations, 0);
        assert_eq!(strict.error_count, 0);
    }

    #[test]
    fn unchanged_enemy_names_allow_proper_names_but_not_english_compounds() {
        let project = project(vec![
            entry("Enemies:name:1", "DORCUS", "DORCUS", EntryStatus::Reviewed),
            entry(
                "Troops:name:60",
                "LARASKYE, XERZA, PATRICK",
                "LARASKYE, XERZA, PATRICK",
                EntryStatus::Reviewed,
            ),
            entry(
                "Enemies:name:2",
                "FIREHOUND",
                "FIREHOUND",
                EntryStatus::Reviewed,
            ),
            entry(
                "Troops:name:9",
                "MASTERBOATER",
                "MASTERBOATER",
                EntryStatus::Reviewed,
            ),
        ]);

        let strict = validate_project_translation_coverage(
            &project,
            CoverageOptions {
                strict_unchanged: true,
                ..CoverageOptions::default()
            },
        );

        assert!(!strict.complete);
        assert_eq!(strict.unchanged_translations, 2);
        assert_eq!(strict.error_count, 2);
        assert_eq!(strict.issues[0].original_preview, "FIREHOUND");
        assert_eq!(strict.issues[1].original_preview, "MASTERBOATER");
    }

    #[test]
    fn marks_coverage_issues_for_repair() {
        let mut project = project(vec![
            entry(
                "Skills:name:1",
                "Fire Hound",
                "Fire Hound",
                EntryStatus::Reviewed,
            ),
            entry("Actors:name:2", "", "", EntryStatus::Untranslated),
        ]);
        let report = validate_project_translation_coverage(
            &project,
            CoverageOptions {
                strict_unchanged: true,
                ..CoverageOptions::default()
            },
        );

        let result = mark_project_coverage_issues_for_repair(&mut project, &report);

        assert_eq!(result.marked_entries, 1);
        assert_eq!(
            project.files[0].entries[0].status,
            EntryStatus::Untranslated
        );
        assert!(project.files[0].entries[0].translation.is_empty());
        assert!(project.files[0].entries[0]
            .notes
            .contains("Reparación de cobertura"));
        assert_eq!(
            project.files[0].entries[1].status,
            EntryStatus::Untranslated
        );
    }
}

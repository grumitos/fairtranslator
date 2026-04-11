use std::collections::BTreeMap;

use crate::models::{EntryStatus, Project, TranslationEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegritySeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityMode {
    AutomatedApply,
    Export,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityIssue {
    pub severity: IntegritySeverity,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntegrityReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialTokenSpan {
    pub start: usize,
    pub end: usize,
    pub token: String,
}

impl IntegrityReport {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

pub fn validate_translation(
    file_path: &str,
    entry: &TranslationEntry,
    translation: &str,
    mode: IntegrityMode,
) -> IntegrityReport {
    let mut issues = Vec::new();

    if translation.trim().is_empty() {
        issues.push(IntegrityIssue {
            severity: IntegritySeverity::Error,
            message: format!(
                "{} -> {} was empty or whitespace-only after translation.",
                file_path, entry.id
            ),
        });
        return fold_issues(issues);
    }

    let original_tokens = extract_special_tokens(&entry.original);
    let translated_tokens = extract_special_tokens(translation);
    if original_tokens != translated_tokens {
        let missing = multiset_difference(&original_tokens, &translated_tokens);
        let unexpected = multiset_difference(&translated_tokens, &original_tokens);
        let mut detail = String::new();
        if !missing.is_empty() {
            detail.push_str(&format!("missing {}", missing.join(", ")));
        }
        if !unexpected.is_empty() {
            if !detail.is_empty() {
                detail.push_str("; ");
            }
            detail.push_str(&format!("unexpected {}", unexpected.join(", ")));
        }

        issues.push(IntegrityIssue {
            severity: IntegritySeverity::Error,
            message: format!(
                "{} -> {} changed control codes or placeholders ({detail}).",
                file_path, entry.id
            ),
        });
    }

    let original_line_count = line_count(&entry.original);
    let translated_line_count = line_count(translation);
    if original_line_count != translated_line_count {
        let is_multiline_command = entry.id.contains(":text:") || entry.id.contains(":scroll:");
        let severity = match mode {
            IntegrityMode::AutomatedApply => IntegritySeverity::Error,
            IntegrityMode::Export if is_multiline_command => IntegritySeverity::Error,
            IntegrityMode::Export => IntegritySeverity::Warning,
        };
        issues.push(IntegrityIssue {
            severity,
            message: format!(
                "{} -> {} changed line count from {} to {}.",
                file_path, entry.id, original_line_count, translated_line_count
            ),
        });
    }

    if is_compact_ui_field(entry) {
        let original_width = longest_line_width(&entry.original);
        let translated_width = longest_line_width(translation);
        if translated_width >= original_width.saturating_add(12)
            && translated_width >= original_width.saturating_mul(3) / 2
            && translated_width >= 24
        {
            issues.push(IntegrityIssue {
                severity: IntegritySeverity::Warning,
                message: format!(
                    "{} -> {} grew from ~{} to ~{} chars and may overflow a short UI field.",
                    file_path, entry.id, original_width, translated_width
                ),
            });
        }
    }

    fold_issues(issues)
}

pub fn normalize_translation_for_apply(entry: &TranslationEntry, translation: &str) -> String {
    let mut normalized = translation.replace("\r\n", "\n");
    if entry.original.contains('\n') {
        normalized = normalized.replace("\\r\\n", "\n");
        normalized = normalized.replace("\\n", "\n");
    }
    normalized
}

pub fn validate_project_for_export(project: &Project) -> IntegrityReport {
    let mut report = IntegrityReport::default();

    for file in &project.files {
        for entry in &file.entries {
            if entry.status == EntryStatus::Untranslated || entry.translation.trim().is_empty() {
                continue;
            }

            let entry_report = validate_translation(
                &file.relative_path,
                entry,
                &entry.translation,
                IntegrityMode::Export,
            );
            report.errors.extend(entry_report.errors);
            report.warnings.extend(entry_report.warnings);
        }
    }

    report
}

pub fn summarize_export_errors(errors: &[String]) -> String {
    const MAX_ERRORS: usize = 8;
    let shown = errors.iter().take(MAX_ERRORS).cloned().collect::<Vec<_>>();
    let remaining = errors.len().saturating_sub(shown.len());
    let mut lines = vec![
        "Export blocked by translation integrity checks.".to_string(),
        "Fix the following entries before exporting:".to_string(),
    ];
    lines.extend(shown.into_iter().map(|issue| format!("- {issue}")));
    if remaining > 0 {
        lines.push(format!("- ... and {remaining} more issues"));
    }
    lines.join("\n")
}

fn fold_issues(issues: Vec<IntegrityIssue>) -> IntegrityReport {
    let mut report = IntegrityReport::default();
    for issue in issues {
        match issue.severity {
            IntegritySeverity::Error => report.errors.push(issue.message),
            IntegritySeverity::Warning => report.warnings.push(issue.message),
        }
    }
    report
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.split('\n').count()
    }
}

fn longest_line_width(text: &str) -> usize {
    text.split('\n')
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

fn is_compact_ui_field(entry: &TranslationEntry) -> bool {
    if entry.id.contains(":choice:")
        || entry.id.contains(":displayName")
        || entry.id.contains(":changeName:")
        || entry.id.contains(":changeNickname:")
        || entry.id == "System:gameTitle"
        || entry.id.contains(":name:")
    {
        return true;
    }

    let original_width = longest_line_width(&entry.original);
    original_width > 0 && original_width <= 24 && !entry.original.contains('\n')
}

fn multiset_difference(left: &[String], right: &[String]) -> Vec<String> {
    let mut remaining = counts(right);
    let mut diff = Vec::new();

    for token in left {
        let count = remaining.entry(token.clone()).or_insert(0);
        if *count > 0 {
            *count -= 1;
        } else {
            diff.push(token.clone());
        }
    }

    diff
}

fn counts(tokens: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for token in tokens {
        *counts.entry(token.clone()).or_insert(0) += 1;
    }
    counts
}

fn extract_special_tokens(text: &str) -> Vec<String> {
    let mut tokens = collect_special_token_spans(text)
        .into_iter()
        .map(|span| span.token)
        .collect::<Vec<_>>();
    tokens.sort();
    tokens
}

pub fn collect_special_token_spans(text: &str) -> Vec<SpecialTokenSpan> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let byte_offsets = char_byte_offsets(text);

    while index < chars.len() {
        if let Some(length) = match_name_compound(&chars[index..]) {
            spans.push(SpecialTokenSpan {
                start: byte_offsets[index],
                end: byte_offsets[index + length],
                token: chars[index..index + length].iter().collect(),
            });
            index += length;
            continue;
        }

        if chars[index] == '\\' {
            if let Some(length) = match_control_code(&chars[index..]) {
                spans.push(SpecialTokenSpan {
                    start: byte_offsets[index],
                    end: byte_offsets[index + length],
                    token: chars[index..index + length].iter().collect(),
                });
                index += length;
                continue;
            }
        }

        if chars[index] == '%' {
            let mut cursor = index + 1;
            while cursor < chars.len() && chars[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor > index + 1 {
                spans.push(SpecialTokenSpan {
                    start: byte_offsets[index],
                    end: byte_offsets[cursor],
                    token: chars[index..cursor].iter().collect(),
                });
                index = cursor;
                continue;
            }
        }

        if let Some(length) = match_named_placeholder(&chars[index..]) {
            spans.push(SpecialTokenSpan {
                start: byte_offsets[index],
                end: byte_offsets[index + length],
                token: chars[index..index + length].iter().collect(),
            });
            index += length;
            continue;
        }

        index += 1;
    }

    spans
}

fn char_byte_offsets(text: &str) -> Vec<usize> {
    let mut offsets = text
        .char_indices()
        .map(|(byte_index, _)| byte_index)
        .collect::<Vec<_>>();
    offsets.push(text.len());
    offsets
}

fn match_control_code(slice: &[char]) -> Option<usize> {
    if slice.len() < 2 || slice[0] != '\\' {
        return None;
    }

    let marker = slice[1];
    if matches!(
        marker,
        '$' | '.' | '|' | '!' | '>' | '<' | '^' | '{' | '}' | '\\'
    ) {
        return Some(2);
    }

    if !marker.is_ascii_alphabetic() {
        return None;
    }

    let mut cursor = 2usize;
    while cursor < slice.len() && slice[cursor].is_ascii_alphabetic() {
        cursor += 1;
    }

    if cursor < slice.len() && slice[cursor] == '[' {
        cursor += 1;
        while cursor < slice.len() && !matches!(slice[cursor], ']' | '\n' | '\r') {
            cursor += 1;
        }
        if cursor < slice.len() && slice[cursor] == ']' {
            cursor += 1;
        }
    }

    Some(cursor)
}

fn match_name_compound(slice: &[char]) -> Option<usize> {
    let base_length = match_control_code(slice)?;
    let base = slice[..base_length].iter().collect::<String>();
    if !(base.starts_with("\\N[") || base.starts_with("\\P[")) {
        return None;
    }

    if slice.get(base_length) != Some(&'-') {
        return None;
    }

    let mut cursor = base_length + 1;
    let mut saw_ascii_alpha = false;
    while cursor < slice.len() {
        let ch = slice[cursor];
        if ch.is_ascii_alphabetic() {
            saw_ascii_alpha = true;
            cursor += 1;
            continue;
        }
        if ch == '-' {
            cursor += 1;
            continue;
        }
        break;
    }

    saw_ascii_alpha.then_some(cursor)
}

fn match_named_placeholder(slice: &[char]) -> Option<usize> {
    let (open, close) = match slice.first().copied() {
        Some('{') => ('{', '}'),
        Some('<') => ('<', '>'),
        _ => return None,
    };

    let mut cursor = 1usize;
    while cursor < slice.len() {
        let ch = slice[cursor];
        if ch == close {
            return if cursor > 1 { Some(cursor + 1) } else { None };
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            cursor += 1;
            continue;
        }
        return None;
    }

    let _ = open;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EntryStatus;

    fn make_entry(id: &str, original: &str) -> TranslationEntry {
        TranslationEntry {
            id: id.to_string(),
            file_context: "ctx".into(),
            original: original.to_string(),
            translation: String::new(),
            status: EntryStatus::Untranslated,
            notes: String::new(),
        }
    }

    #[test]
    fn test_validate_translation_detects_control_code_mismatch() {
        let entry = make_entry("Map001:1:0:text:0", "\\C[4]Hero uses %1.");
        let report = validate_translation(
            "data/Map001.json",
            &entry,
            "Heroe usa %1.",
            IntegrityMode::AutomatedApply,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("control codes or placeholders"));
    }

    #[test]
    fn test_validate_translation_blocks_line_break_drift_for_automated_apply() {
        let entry = make_entry("Map001:1:0:text:0", "Line 1\nLine 2");
        let report = validate_translation(
            "data/Map001.json",
            &entry,
            "Linea 1 Linea 2",
            IntegrityMode::AutomatedApply,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("changed line count"));
    }

    #[test]
    fn test_validate_translation_warns_on_short_field_growth() {
        let entry = make_entry("System:gameTitle", "Tiny Quest");
        let report = validate_translation(
            "data/System.json",
            &entry,
            "La Gran Aventura del Heroe Legendario",
            IntegrityMode::Export,
        );

        assert!(report.errors.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("may overflow"));
    }

    #[test]
    fn test_normalize_translation_for_apply_restores_escaped_line_breaks() {
        let entry = make_entry("Map001:1:0:text:0", "\\C[4]Line 1\nLine 2");
        let normalized = normalize_translation_for_apply(&entry, "\\C[4]\\nLinea 1\\nLinea 2");

        assert_eq!(normalized, "\\C[4]\nLinea 1\nLinea 2");
    }

    #[test]
    fn test_validate_project_for_export_ignores_blank_translations() {
        let project = Project {
            id: "project-1".into(),
            name: "Test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: crate::models::Engine::Mv,
            game_dir: "D:\\Games\\Test".into(),
            files: vec![crate::models::TranslationFile {
                relative_path: "data/System.json".into(),
                entries: vec![TranslationEntry {
                    id: "System:gameTitle".into(),
                    file_context: "ctx".into(),
                    original: "Tiny Quest".into(),
                    translation: "   ".into(),
                    status: EntryStatus::Draft,
                    notes: String::new(),
                }],
            }],
            glossary: Vec::new(),
            style_guide: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let report = validate_project_for_export(&project);
        assert!(report.errors.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_collect_special_token_spans_prefers_name_honorific_compounds() {
        let spans =
            collect_special_token_spans("\\C[14]\\N[4]\\C[0]\n\\N[1]-kun! \\N[2]-onee-chan's food");
        let tokens = spans.into_iter().map(|span| span.token).collect::<Vec<_>>();

        assert_eq!(
            tokens,
            vec![
                "\\C[14]".to_string(),
                "\\N[4]".to_string(),
                "\\C[0]".to_string(),
                "\\N[1]-kun".to_string(),
                "\\N[2]-onee-chan".to_string(),
            ]
        );
    }

    #[test]
    fn test_validate_translation_detects_broken_name_honorific_compound() {
        let entry = make_entry(
            "CommonEvents:411:text:3",
            "\\C[14]\\N[4]\\C[0]\n\\N[1]-kun! \\N[2]-onee-chan's cooking is super delicious!\nLook, look!",
        );
        let report = validate_translation(
            "data/CommonEvents.json",
            &entry,
            "\\C[14]\\N[4]\\C[0]\n\\N[1]-kun! \\N[2]¡La comida de -onee-chan está súper deliciosa!\n¡Mira, mira!",
            IntegrityMode::AutomatedApply,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("missing \\N[2]-onee-chan"));
        assert!(report.errors[0].contains("unexpected \\N[2]"));
    }
}

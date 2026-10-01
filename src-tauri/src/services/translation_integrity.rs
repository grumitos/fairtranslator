use std::collections::BTreeMap;

use crate::models::{Engine, EntryStatus, Project, TranslationEntry};
use crate::services::text_fit;

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
    engine: &Engine,
) -> IntegrityReport {
    let mut issues = Vec::new();
    let structural_severity = match mode {
        IntegrityMode::AutomatedApply => IntegritySeverity::Error,
        IntegrityMode::Export => IntegritySeverity::Warning,
    };

    if translation.trim().is_empty() {
        issues.push(IntegrityIssue {
            severity: IntegritySeverity::Error,
            message: format!(
                "{} -> {} quedó vacía o solo con espacios tras traducir.",
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
            detail.push_str(&format!("faltan {}", missing.join(", ")));
        }
        if !unexpected.is_empty() {
            if !detail.is_empty() {
                detail.push_str("; ");
            }
            detail.push_str(&format!("sobran {}", unexpected.join(", ")));
        }

        issues.push(IntegrityIssue {
            severity: IntegritySeverity::Error,
            message: format!(
                "{} -> {} cambió códigos de control o marcadores ({detail}).",
                file_path, entry.id
            ),
        });
    }

    // The exporter rewraps messages, scrolling text and descriptions to the
    // window, so their line count is free; elsewhere each line maps to a slot.
    if text_fit::exporter_rewraps(engine, &entry.id) {
        if let Some(label) = speaker_label_line(&entry.original) {
            if !keeps_speaker_label(translation) {
                issues.push(IntegrityIssue {
                    severity: structural_severity,
                    message: format!(
                        "{} -> {} unió la línea del hablante «{}» con el diálogo.",
                        file_path, entry.id, label
                    ),
                });
            }
        }
    } else {
        let original_line_count = line_count(&entry.original);
        let translated_line_count = line_count(translation);
        if original_line_count != translated_line_count {
            issues.push(IntegrityIssue {
                severity: structural_severity,
                message: format!(
                    "{} -> {} cambió la cantidad de líneas de {} a {}.",
                    file_path, entry.id, original_line_count, translated_line_count
                ),
            });
        }
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
                    "{} -> {} creció de ~{} a ~{} caracteres y puede desbordar un campo corto de UI.",
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
                &project.engine,
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
        "Exportación bloqueada por comprobaciones de integridad.".to_string(),
        "Corrige estas entradas antes de exportar:".to_string(),
    ];
    lines.extend(shown.into_iter().map(|issue| format!("- {issue}")));
    if remaining > 0 {
        lines.push(format!("- ... y {remaining} incidencias más"));
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

/// First line of a multi-line message that only labels the speaker, such as
/// `【Name】`, `[Name]` or `Name:`. The exporter keeps it on its own line.
fn speaker_label_line(original: &str) -> Option<&str> {
    let (first, rest) = original.split_once('\n')?;
    let first = first.trim();
    (!rest.trim().is_empty() && is_label(first, 24)).then_some(first)
}

/// The translated first line still stands alone as a label. Any bracket style
/// counts, since models may swap `【】` for `[]`.
fn keeps_speaker_label(translation: &str) -> bool {
    translation
        .split_once('\n')
        .is_some_and(|(first, rest)| !rest.trim().is_empty() && is_label(first.trim(), 32))
}

fn is_label(line: &str, max_chars: usize) -> bool {
    let enclosed = (line.starts_with('【') && line.ends_with('】'))
        || (line.starts_with('［') && line.ends_with('］'))
        || (line.starts_with('[') && line.ends_with(']'));
    line.chars().count() <= max_chars && (enclosed || line.ends_with([':', '：']))
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
        if let Some(length) = match_choice_control_dsl(&chars[index..]) {
            spans.push(SpecialTokenSpan {
                start: byte_offsets[index],
                end: byte_offsets[index + length],
                token: chars[index..index + length].iter().collect(),
            });
            index += length;
            continue;
        }

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

    if matches!(marker, 'n' | 'r' | 't') && slice.get(2) != Some(&'[') {
        return Some(2);
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

fn match_choice_control_dsl(slice: &[char]) -> Option<usize> {
    let name_len = slice
        .iter()
        .take(13)
        .take_while(|ch| ch.is_ascii_alphabetic())
        .count();
    if !(2..=12).contains(&name_len) || slice.get(name_len) != Some(&'(') {
        return None;
    }

    let name = slice[..name_len]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        "if" | "en" | "enable" | "disable" | "show" | "hide" | "dis"
    ) {
        return None;
    }

    let mut cursor = name_len + 1;
    let mut saw_bracketed_operand = false;
    while cursor < slice.len() {
        let ch = slice[cursor];
        if ch == '\n' || ch == '\r' {
            return None;
        }
        if ch == '[' {
            saw_bracketed_operand = true;
        }
        if ch == ')' {
            return saw_bracketed_operand.then_some(cursor + 1);
        }
        if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '[' | ']' | '_' | '-' | '+' | '!' | '=' | '<' | '>' | '&' | '|' | ' '
            )
        {
            cursor += 1;
            continue;
        }
        return None;
    }

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
            &Engine::Mv,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("códigos de control o marcadores"));
    }

    #[test]
    fn test_validate_translation_detects_choice_dsl_mismatch() {
        let entry = make_entry(
            "Map003:25:0:choice:47:1",
            "if(s[211])夫に露出していたことを打ち明ける",
        );
        let report = validate_translation(
            "data/Map003.json",
            &entry,
            "si(s[211])Confesarlo",
            IntegrityMode::AutomatedApply,
            &Engine::Mv,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("códigos de control o marcadores"));
    }

    #[test]
    fn line_breaks_are_free_only_where_the_exporter_rewraps() {
        let check = |id: &str, original: &str, translation: &str, mode, engine: &Engine| {
            validate_translation(
                "data/Map001.json",
                &make_entry(id, original),
                translation,
                mode,
                engine,
            )
        };
        let apply = IntegrityMode::AutomatedApply;

        // Messages, scrolling text and database descriptions are rewrapped.
        for id in [
            "Map001:1:0:text:0",
            "CommonEvents:3:scroll:12",
            "Items:description:5",
        ] {
            let report = check(
                id,
                "Line 1\nLine 2\nLine 3",
                "Linea 1 Linea 2 Linea 3",
                apply,
                &Engine::Mv,
            );
            assert_eq!(report, IntegrityReport::default(), "{id}");
        }

        // Plugin parameters (even nested `…:text:0` keys) and Unity keep each line.
        for (id, engine) in [
            ("Plugins:0:parameters:Messages:text:0", Engine::Mz),
            ("Map001:1:0:text:0", Engine::Unity),
        ] {
            let report = check(id, "Line 1\nLine 2", "Linea 1 Linea 2", apply, &engine);
            assert!(report.errors[0].contains("cantidad de líneas"), "{id}");
            let report = check(
                id,
                "Line 1\nLine 2",
                "Linea 1\nLinea 2\nLinea 3",
                IntegrityMode::Export,
                &engine,
            );
            assert!(report.errors.is_empty());
            assert!(report.warnings[0].contains("cantidad de líneas"));
        }

        // A speaker label must stay on its own line even where breaks are free
        // (in any bracket style).
        let id = "Map001:1:0:text:0";
        for (original, kept, merged) in [
            (
                "【Harold】\nWe leave at dawn.\nBe ready.",
                "[Harold]\nSalimos al alba. Prepárate.",
                "【Harold】 Salimos al alba.\nPrepárate.",
            ),
            (
                "\\N[1]:\nWe leave at dawn.",
                "\\N[1]:\nSalimos al alba.",
                "\\N[1]: Salimos al alba.",
            ),
        ] {
            assert!(
                !check(id, original, kept, apply, &Engine::Mv).has_errors(),
                "{kept}"
            );
            let report = check(id, original, merged, apply, &Engine::Mv);
            assert!(report.errors[0].contains("línea del hablante"), "{merged}");
        }
    }

    #[test]
    fn test_validate_translation_warns_on_short_field_growth() {
        let entry = make_entry("System:gameTitle", "Tiny Quest");
        let report = validate_translation(
            "data/System.json",
            &entry,
            "La Gran Aventura del Heroe Legendario",
            IntegrityMode::Export,
            &Engine::Mv,
        );

        assert!(report.errors.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("puede desbordar"));
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
    fn test_collect_special_token_spans_handles_long_plain_alpha_text() {
        let text = "A".repeat(20_000);

        let spans = collect_special_token_spans(&text);

        assert!(spans.is_empty());
    }

    #[test]
    fn test_escaped_newline_does_not_consume_following_translated_word() {
        let spans = collect_special_token_spans(r"Error\nTras cargar");
        let tokens = spans.into_iter().map(|span| span.token).collect::<Vec<_>>();

        assert_eq!(tokens, vec![r"\n".to_string()]);
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
            &Engine::Mv,
        );

        assert!(report.has_errors());
        assert!(report.errors[0].contains("faltan \\N[2]-onee-chan"));
        assert!(report.errors[0].contains("sobran \\N[2]"));
    }
}

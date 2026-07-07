use regex::RegexBuilder;
use uuid::Uuid;

use crate::models::GlossaryTerm;
use crate::services::translation_integrity::collect_special_token_spans;

#[derive(Debug, Clone)]
pub(crate) struct TextReplacement {
    pub(crate) token: String,
    value: String,
}

#[derive(Debug, Clone)]
struct ReplacementSpan {
    start: usize,
    end: usize,
    value: String,
}

pub(crate) fn protect_special_literals(text: &str) -> (String, Vec<TextReplacement>) {
    let mut spans = collect_special_token_spans(text)
        .into_iter()
        .map(|span| ReplacementSpan {
            start: span.start,
            end: span.end,
            value: span.token,
        })
        .collect::<Vec<_>>();

    spans.extend(
        text.match_indices('\n')
            .map(|(start, newline)| ReplacementSpan {
                start,
                end: start + newline.len(),
                value: newline.to_string(),
            }),
    );
    spans.sort_by_key(|span| span.start);

    apply_spans(text, &spans, "LIT")
}

pub(crate) fn protect_glossary_terms(
    text: &str,
    glossary: &[GlossaryTerm],
) -> (String, Vec<TextReplacement>) {
    if glossary.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let mut terms = glossary
        .iter()
        .filter(|term| !term.source.is_empty() && !term.target.is_empty())
        .collect::<Vec<_>>();
    terms.sort_by(|left, right| right.source.len().cmp(&left.source.len()));

    let mut spans = Vec::new();
    for term in terms {
        let regex = RegexBuilder::new(&regex::escape(&term.source))
            .case_insensitive(!term.case_sensitive)
            .build()
            .expect("escaped glossary regex should compile");

        for matched in regex.find_iter(text) {
            if !has_term_boundaries(text, matched.start(), matched.end(), &term.source) {
                continue;
            }
            if spans.iter().any(|span: &ReplacementSpan| {
                matched.start() < span.end && span.start < matched.end()
            }) {
                continue;
            }
            spans.push(ReplacementSpan {
                start: matched.start(),
                end: matched.end(),
                value: term.target.clone(),
            });
        }
    }

    spans.sort_by_key(|span| span.start);
    apply_spans(text, &spans, "TERM")
}

pub(crate) fn restore_replacements(
    translated_text: String,
    replacements: &[TextReplacement],
) -> String {
    let mut restored = translated_text;
    for replacement in replacements {
        restored = restored.replace(&replacement.token, &replacement.value);
    }
    restored
}

fn apply_spans(
    text: &str,
    spans: &[ReplacementSpan],
    label: &str,
) -> (String, Vec<TextReplacement>) {
    if spans.is_empty() {
        return (text.to_string(), Vec::new());
    }

    let token_prefix = unique_token_prefix(text, label);
    let mut protected = String::with_capacity(text.len());
    let mut replacements = Vec::with_capacity(spans.len());
    let mut cursor = 0usize;

    for (index, span) in spans.iter().enumerate() {
        if span.start < cursor {
            continue;
        }

        protected.push_str(&text[cursor..span.start]);
        let token = format!("{token_prefix}_{index}__");
        protected.push_str(&token);
        replacements.push(TextReplacement {
            token,
            value: span.value.clone(),
        });
        cursor = span.end;
    }

    protected.push_str(&text[cursor..]);
    (protected, replacements)
}

fn unique_token_prefix(text: &str, label: &str) -> String {
    loop {
        let prefix = format!("__FT_{}_{}__", label, Uuid::new_v4().simple());
        if !text.contains(&prefix) {
            return prefix;
        }
    }
}

fn has_term_boundaries(text: &str, start: usize, end: usize, term: &str) -> bool {
    let first = term.chars().next();
    let last = term.chars().last();
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();

    let start_ok = !first.is_some_and(is_word_char) || before.is_none_or(|ch| !is_word_char(ch));
    let end_ok = !last.is_some_and(is_word_char) || after.is_none_or(|ch| !is_word_char(ch));

    start_ok && end_ok
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glossary_token_roundtrip() {
        let glossary = vec![GlossaryTerm {
            source: "勇者".to_string(),
            target: "Heroe".to_string(),
            case_sensitive: true,
        }];

        let (protected, replacements) = protect_glossary_terms("El 勇者 ataca", &glossary);
        assert!(protected.contains("__FT_TERM_"));

        let replacement_token = replacements[0].token.clone();
        let restored =
            restore_replacements(format!("The {replacement_token} attacks"), &replacements);
        assert_eq!(restored, "The Heroe attacks");
    }

    #[test]
    fn glossary_protection_respects_boundaries() {
        let glossary = vec![GlossaryTerm {
            source: "art".to_string(),
            target: "arte".to_string(),
            case_sensitive: false,
        }];

        let (protected, replacements) = protect_glossary_terms("start art artful", &glossary);

        assert_eq!(replacements.len(), 1);
        assert!(protected.contains("start "));
        assert!(protected.contains(" artful"));
    }

    #[test]
    fn special_literal_roundtrip_preserves_control_codes_and_newlines() {
        let (protected, replacements) = protect_special_literals("\\C[2]Hello\n%1 <hero>");
        assert_eq!(replacements.len(), 4);
        assert!(!protected.contains("\\C[2]"));
        assert!(!protected.contains('\n'));

        let restored = restore_replacements(protected, &replacements);
        assert_eq!(restored, "\\C[2]Hello\n%1 <hero>");
    }
}

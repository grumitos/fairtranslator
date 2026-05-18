use std::collections::HashSet;

pub(crate) fn is_japanese_language(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower == "ja" || lower == "jp" || lower.contains("japanese") || lower.contains("japon")
}

pub(crate) fn is_spanish_language(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower == "es"
        || lower.contains("spanish")
        || lower.contains("español")
        || lower.contains("espanol")
}

pub(crate) fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(crate) fn looks_like_term_entry(entry_id: &str, file_context: &str) -> bool {
    let id = entry_id.to_ascii_lowercase();
    let context = file_context.to_ascii_lowercase();
    id.contains(":name:")
        || id.contains(":nickname:")
        || id.contains("displayname")
        || context.contains("> name")
        || context.contains("displayname")
        || context.contains("nickname")
}

pub(crate) fn file_kind_for_path(path: &str) -> &'static str {
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

pub(crate) fn contains_honorific_hint(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    normalized.contains("-san")
        || normalized.contains("-chan")
        || normalized.contains("-kun")
        || text.contains("お姉")
        || text.contains("兄")
        || text.contains("姉")
}

pub(crate) fn contains_ascii_words(text: &str) -> bool {
    text.split_whitespace()
        .any(|token| token.chars().all(|ch| ch.is_ascii_alphabetic()) && token.len() > 2)
}

pub(crate) fn contains_non_ascii(text: &str) -> bool {
    text.chars().any(|ch| !ch.is_ascii())
}

pub(crate) fn lexical_terms_vec(text: &str) -> Vec<String> {
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

pub(crate) fn lexical_terms_set(text: &str) -> HashSet<String> {
    lexical_terms_vec(text).into_iter().collect()
}

pub(crate) fn overlap_score_vec(left: &HashSet<String>, right: &[String]) -> usize {
    right
        .iter()
        .filter(|token| left.contains(token.as_str()))
        .count()
}

pub(crate) fn compact_context_hint(value: &str) -> String {
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

pub(crate) fn compact_text_preview(value: &str, max_chars: usize) -> String {
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

    #[test]
    fn classifies_common_rpg_maker_paths() {
        assert_eq!(file_kind_for_path("data/System.json"), "System");
        assert_eq!(file_kind_for_path("js/plugins.js"), "Plugins");
        assert_eq!(file_kind_for_path("data/Map001.json"), "Maps");
        assert_eq!(file_kind_for_path("data/Actors.json"), "Database");
    }

    #[test]
    fn text_preview_compacts_whitespace_and_limits_chars() {
        assert_eq!(
            compact_text_preview(" Line 1\n\nLine 2 with extra spaces ", 12),
            "Line 1 / / L..."
        );
    }

    #[test]
    fn lexical_overlap_scores_reused_terms() {
        let left = lexical_terms_set("Harold opens the castle gate");
        let right = lexical_terms_vec("Castle gate translated previously");

        assert_eq!(overlap_score_vec(&left, &right), 2);
    }
}

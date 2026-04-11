/// Google Cloud Translation provider using the v2 (Basic) REST API.
///
/// Uses API key authentication (no OAuth/service account needed).
/// Endpoint: `https://translation.googleapis.com/language/translate/v2`
use regex::RegexBuilder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::provider::{
    TranslationBatchContext, TranslationProvider, TranslationRequest, TranslationResponse,
};
use crate::services::translation_integrity::collect_special_token_spans;

/// Maximum texts per API call. Google recommends ≤128 per request.
const MAX_BATCH_SIZE: usize = 100;

/// Default Google Translate API endpoint
const DEFAULT_ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

pub struct GoogleProvider {
    client: Client,
    api_key: String,
    endpoint: String,
}

#[derive(Debug, Clone)]
struct TextReplacement {
    token: String,
    value: String,
}

#[derive(Debug, Clone)]
struct ReplacementSpan {
    start: usize,
    end: usize,
    value: String,
}

impl GoogleProvider {
    pub fn new(api_key: String, endpoint: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            endpoint: endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
        }
    }

    /// Google Translate uses ISO 639-1 codes directly for most languages.
    fn normalize_lang(lang: &str) -> String {
        lang.to_lowercase()
    }

    fn protect_special_literals(text: &str) -> (String, Vec<TextReplacement>) {
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

        Self::apply_spans(text, &spans, "LIT")
    }

    /// Replace known glossary terms with stable placeholders before translation.
    fn protect_glossary_terms(
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
                if !Self::has_term_boundaries(text, matched.start(), matched.end(), &term.source) {
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
        Self::apply_spans(text, &spans, "TERM")
    }

    fn has_term_boundaries(text: &str, start: usize, end: usize, term: &str) -> bool {
        let first = term.chars().next();
        let last = term.chars().last();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();

        let start_ok = !first.is_some_and(Self::is_word_char)
            || before.is_none_or(|ch| !Self::is_word_char(ch));
        let end_ok =
            !last.is_some_and(Self::is_word_char) || after.is_none_or(|ch| !Self::is_word_char(ch));

        start_ok && end_ok
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn apply_spans(
        text: &str,
        spans: &[ReplacementSpan],
        label: &str,
    ) -> (String, Vec<TextReplacement>) {
        if spans.is_empty() {
            return (text.to_string(), Vec::new());
        }

        let token_prefix = Self::unique_token_prefix(text, label);
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

    fn restore_replacements(translated_text: String, replacements: &[TextReplacement]) -> String {
        let mut restored = translated_text;
        for replacement in replacements {
            restored = restored.replace(&replacement.token, &replacement.value);
        }
        restored
    }
}

/// Google Translate API request body
#[derive(Serialize)]
struct GoogleRequest {
    q: Vec<String>,
    target: String,
    source: String,
    format: String,
}

/// Google Translate API response
#[derive(Deserialize)]
struct GoogleResponse {
    data: GoogleData,
}

#[derive(Deserialize)]
struct GoogleData {
    translations: Vec<GoogleTranslation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTranslation {
    translated_text: String,
}

/// Google Translate detect response (used for validation)
#[derive(Deserialize)]
struct GoogleDetectResponse {
    data: GoogleDetectData,
}

#[derive(Deserialize)]
struct GoogleDetectData {
    detections: Vec<Vec<GoogleDetection>>,
}

#[derive(Deserialize)]
struct GoogleDetection {
    language: String,
}

#[async_trait::async_trait]
impl TranslationProvider for GoogleProvider {
    fn name(&self) -> &'static str {
        "Google Translate"
    }

    async fn validate(&self) -> Result<(), AppError> {
        // Use the detect endpoint with a simple text to validate the API key
        let detect_url = self.endpoint.replace("/v2", "/v2/detect");

        let response = self
            .client
            .post(&detect_url)
            .query(&[("key", &self.api_key)])
            .json(&serde_json::json!({ "q": "hello" }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "Google Translate API validation failed ({}): {}",
                status, body
            )));
        }

        let detect: GoogleDetectResponse = response.json().await.map_err(|e| {
            AppError::Api(format!(
                "Google Translate validation response parse error: {}",
                e
            ))
        })?;

        let has_detection = detect
            .data
            .detections
            .iter()
            .flatten()
            .any(|entry| !entry.language.trim().is_empty());
        if !has_detection {
            return Err(AppError::Api(
                "Google Translate validation returned no detections".to_string(),
            ));
        }

        Ok(())
    }

    async fn translate_batch(
        &self,
        requests: &[TranslationRequest],
        source_lang: &str,
        target_lang: &str,
        glossary: &[GlossaryTerm],
        _context: &TranslationBatchContext,
    ) -> Result<Vec<TranslationResponse>, AppError> {
        let mut all_responses = Vec::with_capacity(requests.len());
        let target = Self::normalize_lang(target_lang);
        let source = Self::normalize_lang(source_lang);

        for chunk in requests.chunks(MAX_BATCH_SIZE) {
            let prepared: Vec<(String, Vec<TextReplacement>, Vec<TextReplacement>)> = chunk
                .iter()
                .map(|request| {
                    let (literal_protected, literal_replacements) =
                        Self::protect_special_literals(&request.text);
                    let (glossary_protected, glossary_replacements) =
                        Self::protect_glossary_terms(&literal_protected, glossary);
                    (
                        glossary_protected,
                        literal_replacements,
                        glossary_replacements,
                    )
                })
                .collect();

            let texts: Vec<String> = prepared.iter().map(|(text, _, _)| text.clone()).collect();

            let google_request = GoogleRequest {
                q: texts,
                target: target.clone(),
                source: source.clone(),
                format: "text".to_string(),
            };

            let response = self
                .client
                .post(&self.endpoint)
                .query(&[("key", &self.api_key)])
                .json(&google_request)
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Api(format!(
                    "Google Translate API error ({}): {}",
                    status, body
                )));
            }

            let google_response: GoogleResponse = response.json().await?;

            if google_response.data.translations.len() != chunk.len() {
                return Err(AppError::Api(format!(
                    "Google Translate returned {} translations but expected {}",
                    google_response.data.translations.len(),
                    chunk.len()
                )));
            }

            for ((req, translation), (_, literal_replacements, glossary_replacements)) in chunk
                .iter()
                .zip(google_response.data.translations.into_iter())
                .zip(prepared.iter())
            {
                all_responses.push(TranslationResponse {
                    entry_id: req.entry_id.clone(),
                    translated_text: Self::restore_replacements(
                        Self::restore_replacements(
                            translation.translated_text,
                            glossary_replacements,
                        ),
                        literal_replacements,
                    ),
                });
            }
        }

        Ok(all_responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_endpoint() {
        let provider = GoogleProvider::new("test-key".to_string(), None);
        assert_eq!(provider.endpoint, DEFAULT_ENDPOINT);
    }

    #[test]
    fn test_custom_endpoint() {
        let provider = GoogleProvider::new(
            "test-key".to_string(),
            Some("https://custom.google.com/v2".to_string()),
        );
        assert_eq!(provider.endpoint, "https://custom.google.com/v2");
    }

    #[test]
    fn test_normalize_lang() {
        assert_eq!(GoogleProvider::normalize_lang("EN"), "en");
        assert_eq!(GoogleProvider::normalize_lang("ja"), "ja");
        assert_eq!(GoogleProvider::normalize_lang("ES"), "es");
    }

    #[test]
    fn test_provider_name() {
        let provider = GoogleProvider::new("test-key".to_string(), None);
        assert_eq!(provider.name(), "Google Translate");
    }

    #[test]
    fn test_glossary_token_roundtrip() {
        let glossary = vec![GlossaryTerm {
            source: "勇者".to_string(),
            target: "Heroe".to_string(),
            case_sensitive: true,
        }];

        let (protected, replacements) =
            GoogleProvider::protect_glossary_terms("El 勇者 ataca", &glossary);
        assert!(protected.contains("__FT_TERM_"));

        let replacement_token = replacements[0].token.clone();
        let restored = GoogleProvider::restore_replacements(
            format!("The {replacement_token} attacks"),
            &replacements,
        );
        assert_eq!(restored, "The Heroe attacks");
    }

    #[test]
    fn test_glossary_protection_respects_boundaries() {
        let glossary = vec![GlossaryTerm {
            source: "art".to_string(),
            target: "arte".to_string(),
            case_sensitive: false,
        }];

        let (protected, replacements) =
            GoogleProvider::protect_glossary_terms("start art artful", &glossary);

        assert_eq!(replacements.len(), 1);
        assert!(protected.contains("start "));
        assert!(protected.contains(" artful"));
    }

    #[test]
    fn test_special_literal_roundtrip_preserves_control_codes_and_newlines() {
        let (protected, replacements) =
            GoogleProvider::protect_special_literals("\\C[2]Hello\n%1 <hero>");
        assert_eq!(replacements.len(), 4);
        assert!(!protected.contains("\\C[2]"));
        assert!(!protected.contains('\n'));

        let restored = GoogleProvider::restore_replacements(protected, &replacements);
        assert_eq!(restored, "\\C[2]Hello\n%1 <hero>");
    }
}

/// Google Cloud Translation provider using the v2 (Basic) REST API.
///
/// Uses API key authentication (no OAuth/service account needed).
/// Endpoint: `https://translation.googleapis.com/language/translate/v2`
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::provider::{
    provider_http_client, TranslationBatchContext, TranslationProvider, TranslationRequest,
    TranslationResponse,
};
use crate::services::translation_replacements::{
    protect_glossary_terms, protect_special_literals, restore_replacements, TextReplacement,
};

/// Maximum texts per API call. Google recommends ≤128 per request.
const MAX_BATCH_SIZE: usize = 100;

/// Default Google Translate API endpoint
const DEFAULT_ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

pub struct GoogleProvider {
    client: Client,
    api_key: String,
    endpoint: String,
}

impl GoogleProvider {
    pub fn new(api_key: String, endpoint: Option<String>) -> Result<Self, AppError> {
        Ok(Self {
            client: provider_http_client()?,
            api_key,
            endpoint: endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
        })
    }

    /// Google Translate uses ISO 639-1 codes directly for most languages.
    fn normalize_lang(lang: &str) -> String {
        lang.to_lowercase()
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
            return Err(AppError::Api(format!(
                "Google Translate API validation failed ({})",
                status
            )));
        }

        let detect: GoogleDetectResponse = response.json().await?;

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
                        protect_special_literals(&request.text);
                    let (glossary_protected, glossary_replacements) =
                        protect_glossary_terms(&literal_protected, glossary);
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
                return Err(AppError::Api(format!(
                    "Google Translate API error ({})",
                    status
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
                    translated_text: restore_replacements(
                        restore_replacements(translation.translated_text, glossary_replacements),
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
        let provider = GoogleProvider::new("test-key".to_string(), None).unwrap();
        assert_eq!(provider.endpoint, DEFAULT_ENDPOINT);
    }

    #[test]
    fn test_custom_endpoint() {
        let provider = GoogleProvider::new(
            "test-key".to_string(),
            Some("https://custom.google.com/v2".to_string()),
        )
        .unwrap();
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
        let provider = GoogleProvider::new("test-key".to_string(), None).unwrap();
        assert_eq!(provider.name(), "Google Translate");
    }
}

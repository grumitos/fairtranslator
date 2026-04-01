/// DeepL translation provider using the DeepL API v2.
///
/// Uses the `/v2/translate` endpoint. Supports both free and pro API keys
/// (free keys use `api-free.deepl.com`, pro keys use `api.deepl.com`).
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::provider::{TranslationProvider, TranslationRequest, TranslationResponse};

/// Maximum texts per API call. DeepL supports up to 50 texts per request.
const MAX_BATCH_SIZE: usize = 50;

/// Free-tier endpoint
const FREE_ENDPOINT: &str = "https://api-free.deepl.com/v2/translate";

/// Pro endpoint
const PRO_ENDPOINT: &str = "https://api.deepl.com/v2/translate";

pub struct DeepLProvider {
    client: Client,
    api_key: String,
    endpoint: String,
}

impl DeepLProvider {
    pub fn new(api_key: String, endpoint: Option<String>) -> Self {
        let endpoint = endpoint.unwrap_or_else(|| {
            // DeepL free API keys end with ":fx"
            if api_key.ends_with(":fx") {
                FREE_ENDPOINT.to_string()
            } else {
                PRO_ENDPOINT.to_string()
            }
        });

        Self {
            client: Client::new(),
            api_key,
            endpoint,
        }
    }

    /// Convert ISO 639-1 language codes to DeepL's expected format.
    /// DeepL uses uppercase codes and some have regional variants (e.g., "EN-US", "PT-BR").
    fn normalize_lang(lang: &str) -> String {
        match lang.to_lowercase().as_str() {
            "en" => "EN".to_string(),
            "es" => "ES".to_string(),
            "ja" => "JA".to_string(),
            "zh" => "ZH-HANS".to_string(),
            "pt" => "PT-BR".to_string(),
            other => other.to_uppercase(),
        }
    }

    /// Build context hint from glossary terms.
    /// DeepL's `context` field is best-effort and not a strict glossary.
    fn build_glossary_context(glossary: &[GlossaryTerm]) -> String {
        if glossary.is_empty() {
            return String::new();
        }

        let mut lines = Vec::with_capacity(glossary.len());
        for term in glossary {
            let case_tag = if term.case_sensitive {
                " (case-sensitive)"
            } else {
                ""
            };
            lines.push(format!("- {} => {}{}", term.source, term.target, case_tag));
        }

        format!(
            "Preferred terminology. Use these terms consistently when possible:\n{}",
            lines.join("\n")
        )
    }
}

/// DeepL API request body
#[derive(Serialize)]
struct DeepLRequest {
    text: Vec<String>,
    target_lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
}

/// DeepL API response
#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
}

/// DeepL usage response (for validation)
#[derive(Deserialize)]
struct DeepLUsage {
    character_count: u64,
    character_limit: u64,
}

#[async_trait::async_trait]
impl TranslationProvider for DeepLProvider {
    fn name(&self) -> &'static str {
        "DeepL"
    }

    async fn validate(&self) -> Result<(), AppError> {
        // Use the /v2/usage endpoint to validate the API key
        let usage_url = self.endpoint.replace("/v2/translate", "/v2/usage");

        let response = self
            .client
            .get(&usage_url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "DeepL API validation failed ({}): {}",
                status, body
            )));
        }

        let usage: DeepLUsage = response.json().await?;
        log::info!(
            "DeepL API key valid. Usage: {}/{} characters",
            usage.character_count,
            usage.character_limit
        );

        Ok(())
    }

    async fn translate_batch(
        &self,
        requests: &[TranslationRequest],
        source_lang: &str,
        target_lang: &str,
        glossary: &[GlossaryTerm],
    ) -> Result<Vec<TranslationResponse>, AppError> {
        let mut all_responses = Vec::with_capacity(requests.len());
        let target = Self::normalize_lang(target_lang);
        let source = Self::normalize_lang(source_lang);
        let context = Self::build_glossary_context(glossary);
        let context = if context.is_empty() {
            None
        } else {
            Some(context)
        };

        for chunk in requests.chunks(MAX_BATCH_SIZE) {
            let texts: Vec<String> = chunk.iter().map(|r| r.text.clone()).collect();

            let deepl_request = DeepLRequest {
                text: texts,
                target_lang: target.clone(),
                source_lang: Some(source.clone()),
                context: context.clone(),
            };

            let response = self
                .client
                .post(&self.endpoint)
                .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
                .json(&deepl_request)
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Api(format!(
                    "DeepL API error ({}): {}",
                    status, body
                )));
            }

            let deepl_response: DeepLResponse = response.json().await?;

            if deepl_response.translations.len() != chunk.len() {
                return Err(AppError::Api(format!(
                    "DeepL returned {} translations but expected {}",
                    deepl_response.translations.len(),
                    chunk.len()
                )));
            }

            for (req, translation) in chunk.iter().zip(deepl_response.translations.into_iter()) {
                all_responses.push(TranslationResponse {
                    entry_id: req.entry_id.clone(),
                    translated_text: translation.text,
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
    fn test_free_key_detection() {
        let provider = DeepLProvider::new("abc123:fx".to_string(), None);
        assert_eq!(provider.endpoint, FREE_ENDPOINT);
    }

    #[test]
    fn test_pro_key_detection() {
        let provider = DeepLProvider::new("abc123-pro-key".to_string(), None);
        assert_eq!(provider.endpoint, PRO_ENDPOINT);
    }

    #[test]
    fn test_custom_endpoint() {
        let provider = DeepLProvider::new(
            "test-key".to_string(),
            Some("https://custom.deepl.com/v2/translate".to_string()),
        );
        assert_eq!(provider.endpoint, "https://custom.deepl.com/v2/translate");
    }

    #[test]
    fn test_normalize_lang() {
        assert_eq!(DeepLProvider::normalize_lang("en"), "EN");
        assert_eq!(DeepLProvider::normalize_lang("ja"), "JA");
        assert_eq!(DeepLProvider::normalize_lang("zh"), "ZH-HANS");
        assert_eq!(DeepLProvider::normalize_lang("pt"), "PT-BR");
        assert_eq!(DeepLProvider::normalize_lang("ko"), "KO");
    }

    #[test]
    fn test_provider_name() {
        let provider = DeepLProvider::new("test:fx".to_string(), None);
        assert_eq!(provider.name(), "DeepL");
    }

    #[test]
    fn test_glossary_context_generation() {
        let glossary = vec![GlossaryTerm {
            source: "勇者".to_string(),
            target: "Heroe".to_string(),
            case_sensitive: true,
        }];

        let context = DeepLProvider::build_glossary_context(&glossary);
        assert!(context.contains("勇者"));
        assert!(context.contains("Heroe"));
    }
}

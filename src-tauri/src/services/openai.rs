/// OpenAI translation provider using the Chat Completions API.
///
/// Sends batched translation requests as a single prompt with a structured
/// JSON response format to translate multiple texts at once.
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::provider::{TranslationProvider, TranslationRequest, TranslationResponse};

/// Maximum number of entries to send in a single API call.
/// OpenAI can handle large prompts, but we cap to avoid token limits.
const MAX_BATCH_SIZE: usize = 20;

/// Default model to use if none is specified in config
const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// Default API endpoint
const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    endpoint: String,
    organization: Option<String>,
    project: Option<String>,
}

impl OpenAiProvider {
    pub fn new(
        api_key: String,
        model: Option<String>,
        endpoint: Option<String>,
        organization: Option<String>,
        project: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            endpoint: endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
            organization: organization.filter(|value| !value.trim().is_empty()),
            project: project.filter(|value| !value.trim().is_empty()),
        }
    }

    fn apply_auth_headers(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request.header("Authorization", format!("Bearer {}", self.api_key));
        let request = if let Some(organization) = &self.organization {
            request.header("OpenAI-Organization", organization)
        } else {
            request
        };

        if let Some(project) = &self.project {
            request.header("OpenAI-Project", project)
        } else {
            request
        }
    }

    /// Build the system prompt with glossary context
    fn build_system_prompt(
        source_lang: &str,
        target_lang: &str,
        glossary: &[GlossaryTerm],
    ) -> String {
        let mut prompt = format!(
            "You are a professional translator for RPG Maker video games. \
             Translate the following texts from {source_lang} to {target_lang}. \
             Preserve all RPG Maker control codes (e.g., \\V[n], \\N[n], \\C[n], \\{{}}, \\{{}}, \\$, \\., \\|, \\!, \\>, \\<, \\^). \
             Keep the tone and style appropriate for a video game. \
             Maintain proper nouns as-is unless a glossary translation is provided."
        );

        if !glossary.is_empty() {
            prompt.push_str("\n\nGlossary (use these translations consistently):\n");
            for term in glossary {
                prompt.push_str(&format!("- \"{}\" → \"{}\"\n", term.source, term.target));
            }
        }

        prompt.push_str(
            "\n\nRespond ONLY with a JSON array of translated strings in the same order as the input. \
             No explanations, no markdown, just the JSON array."
        );

        prompt
    }
}

/// OpenAI Chat Completions request body
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI Chat Completions response
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[async_trait::async_trait]
impl TranslationProvider for OpenAiProvider {
    fn name(&self) -> &'static str {
        "OpenAI"
    }

    async fn validate(&self) -> Result<(), AppError> {
        // Send a minimal request to verify the API key works
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Say \"ok\"".to_string(),
            }],
            temperature: 0.0,
        };

        let response = self
            .apply_auth_headers(self.client.post(&self.endpoint))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "OpenAI API validation failed ({}): {}",
                status, body
            )));
        }

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

        // Process in chunks to avoid token limits
        for chunk in requests.chunks(MAX_BATCH_SIZE) {
            let system_prompt = Self::build_system_prompt(source_lang, target_lang, glossary);

            // Build the user message as a JSON array of source texts
            let texts: Vec<&str> = chunk.iter().map(|r| r.text.as_str()).collect();
            let user_content = serde_json::to_string(&texts)
                .map_err(|e| AppError::Api(format!("Failed to serialize texts: {}", e)))?;

            let request = ChatRequest {
                model: self.model.clone(),
                messages: vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: system_prompt,
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: user_content,
                    },
                ],
                temperature: 0.3,
            };

            let response = self
                .apply_auth_headers(self.client.post(&self.endpoint))
                .json(&request)
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Api(format!(
                    "OpenAI API error ({}): {}",
                    status, body
                )));
            }

            let chat_response: ChatResponse = response.json().await?;

            let content = chat_response
                .choices
                .first()
                .ok_or_else(|| AppError::Api("OpenAI returned no choices".to_string()))?
                .message
                .content
                .trim()
                .to_string();

            // Strip markdown code fences if the model wraps its response
            let json_str = content
                .strip_prefix("```json")
                .or_else(|| content.strip_prefix("```"))
                .and_then(|s| s.strip_suffix("```"))
                .map(|s| s.trim())
                .unwrap_or(&content);

            // Parse the JSON array of translated strings
            let translations: Vec<String> = serde_json::from_str(json_str).map_err(|e| {
                AppError::Api(format!(
                    "Failed to parse OpenAI response as JSON array: {}. Response: {}",
                    e, content
                ))
            })?;

            if translations.len() != chunk.len() {
                return Err(AppError::Api(format!(
                    "OpenAI returned {} translations but expected {}",
                    translations.len(),
                    chunk.len()
                )));
            }

            for (req, translated) in chunk.iter().zip(translations.into_iter()) {
                all_responses.push(TranslationResponse {
                    entry_id: req.entry_id.clone(),
                    translated_text: translated,
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
    fn test_build_system_prompt_without_glossary() {
        let prompt = OpenAiProvider::build_system_prompt("ja", "es", &[]);
        assert!(prompt.contains("ja"));
        assert!(prompt.contains("es"));
        assert!(prompt.contains("JSON array"));
        assert!(!prompt.contains("Glossary"));
    }

    #[test]
    fn test_build_system_prompt_with_glossary() {
        let glossary = vec![GlossaryTerm {
            source: "勇者".to_string(),
            target: "Héroe".to_string(),
            case_sensitive: true,
        }];
        let prompt = OpenAiProvider::build_system_prompt("ja", "es", &glossary);
        assert!(prompt.contains("Glossary"));
        assert!(prompt.contains("勇者"));
        assert!(prompt.contains("Héroe"));
    }

    #[test]
    fn test_provider_name() {
        let provider = OpenAiProvider::new("test-key".to_string(), None, None, None, None);
        assert_eq!(provider.name(), "OpenAI");
    }

    #[test]
    fn test_default_model_and_endpoint() {
        let provider = OpenAiProvider::new("test-key".to_string(), None, None, None, None);
        assert_eq!(provider.model, DEFAULT_MODEL);
        assert_eq!(provider.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(provider.organization, None);
        assert_eq!(provider.project, None);
    }

    #[test]
    fn test_custom_model_and_endpoint() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            Some("gpt-4o".to_string()),
            Some("https://custom.api.com/v1/chat".to_string()),
            Some("org_123".to_string()),
            Some("proj_456".to_string()),
        );
        assert_eq!(provider.model, "gpt-4o");
        assert_eq!(provider.endpoint, "https://custom.api.com/v1/chat");
        assert_eq!(provider.organization.as_deref(), Some("org_123"));
        assert_eq!(provider.project.as_deref(), Some("proj_456"));
    }

    #[test]
    fn test_system_prompt_mentions_control_codes() {
        let prompt = OpenAiProvider::build_system_prompt("ja", "es", &[]);
        // Should mention RPG Maker control codes
        assert!(prompt.contains("\\V[n]") || prompt.contains("control codes"));
        assert!(prompt.contains("\\N[n]") || prompt.contains("RPG Maker"));
    }

    #[test]
    fn test_system_prompt_multiple_glossary_terms() {
        let glossary = vec![
            GlossaryTerm {
                source: "勇者".to_string(),
                target: "Héroe".to_string(),
                case_sensitive: true,
            },
            GlossaryTerm {
                source: "魔王".to_string(),
                target: "Rey Demonio".to_string(),
                case_sensitive: false,
            },
        ];
        let prompt = OpenAiProvider::build_system_prompt("ja", "es", &glossary);
        assert!(prompt.contains("勇者"));
        assert!(prompt.contains("Héroe"));
        assert!(prompt.contains("魔王"));
        assert!(prompt.contains("Rey Demonio"));
    }

    #[test]
    fn test_empty_scoping_headers_are_normalized_to_none() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            None,
            None,
            Some("   ".to_string()),
            Some(String::new()),
        );
        assert_eq!(provider.organization, None);
        assert_eq!(provider.project, None);
    }

    #[test]
    fn test_apply_auth_headers_includes_optional_scoping() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            None,
            None,
            Some("org_123".to_string()),
            Some("proj_456".to_string()),
        );

        let request = provider
            .apply_auth_headers(provider.client.post(&provider.endpoint))
            .build()
            .unwrap();

        assert_eq!(
            request.headers().get("Authorization").unwrap(),
            "Bearer test-key"
        );
        assert_eq!(
            request.headers().get("OpenAI-Organization").unwrap(),
            "org_123"
        );
        assert_eq!(request.headers().get("OpenAI-Project").unwrap(), "proj_456");
    }
}

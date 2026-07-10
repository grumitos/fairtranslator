/// OpenAI translation provider using the Chat Completions API.
///
/// Sends batched translation requests as a single prompt with a structured
/// JSON response format to translate multiple texts at once.
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::ai_models::{self, ProviderModel};
use crate::services::compact_prompt;
use crate::services::provider::{
    approximate_token_count, TranslationBatchContext, TranslationProvider, TranslationRequest,
    TranslationResponse,
};

/// Maximum number of entries to send in a single API call.
/// OpenAI can handle large prompts, but we cap to avoid token limits.
const MAX_BATCH_SIZE: usize = 24;
const OPENAI_CHAT_WRAPPER_TOKEN_OVERHEAD: usize = 96;
const OPENAI_RESPONSE_HEADROOM_TOKENS: usize = 2048;

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
    ) -> Result<Self, AppError> {
        let model = model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Api("Select an OpenAI model before translating".into()))?;

        Ok(Self {
            client: Client::new(),
            api_key,
            model,
            endpoint: endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string()),
            organization: organization.filter(|value| !value.trim().is_empty()),
            project: project.filter(|value| !value.trim().is_empty()),
        })
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

    pub async fn list_models(
        api_key: &str,
        endpoint: Option<String>,
        organization: Option<String>,
        project: Option<String>,
    ) -> Result<Vec<ProviderModel>, AppError> {
        let client = Client::new();
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        let mut headers = Vec::new();
        if let Some(organization) = organization.as_deref() {
            headers.push(("OpenAI-Organization", organization));
        }
        if let Some(project) = project.as_deref() {
            headers.push(("OpenAI-Project", project));
        }

        ai_models::list_openai_compatible_models(
            &client,
            &endpoint,
            Some(api_key),
            headers.as_slice(),
        )
        .await
    }

    /// Build the system prompt with glossary and orchestration context.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn build_system_prompt(
        source_lang: &str,
        target_lang: &str,
        glossary: &[GlossaryTerm],
        context: &TranslationBatchContext,
    ) -> String {
        compact_prompt::build_provider_bundle(source_lang, target_lang, &[], glossary, context)
            .expect("building OpenAI prompt shell should not fail")
            .system_prompt
    }

    pub(crate) fn estimate_total_tokens(
        source_lang: &str,
        target_lang: &str,
        requests: &[TranslationRequest],
        glossary: &[GlossaryTerm],
        context: &TranslationBatchContext,
    ) -> usize {
        let bundle = compact_prompt::build_provider_bundle(
            source_lang,
            target_lang,
            requests,
            glossary,
            context,
        )
        .expect("serializing compact OpenAI prompt should not fail");
        let system_prompt = bundle.system_prompt;
        let user_content = bundle.user_payload;

        approximate_token_count(&system_prompt)
            + approximate_token_count(&user_content)
            + OPENAI_CHAT_WRAPPER_TOKEN_OVERHEAD
            + OPENAI_RESPONSE_HEADROOM_TOKENS
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
            return Err(AppError::Api(format!(
                "OpenAI API validation failed ({})",
                status
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
        context: &TranslationBatchContext,
    ) -> Result<Vec<TranslationResponse>, AppError> {
        let mut all_responses = Vec::with_capacity(requests.len());

        // Process in chunks to avoid token limits
        for chunk in requests.chunks(MAX_BATCH_SIZE) {
            let bundle = compact_prompt::build_provider_bundle(
                source_lang,
                target_lang,
                chunk,
                glossary,
                context,
            )?;
            let system_prompt = bundle.system_prompt.clone();
            let user_content = bundle.user_payload.clone();

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
                return Err(AppError::Api(format!("OpenAI API error ({})", status)));
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

            let json_str = content
                .strip_prefix("```json")
                .or_else(|| content.strip_prefix("```"))
                .and_then(|s| s.strip_suffix("```"))
                .map(|s| s.trim())
                .unwrap_or(&content);

            let parsed = compact_prompt::parse_provider_response(json_str, &bundle.entry_refs)
                .map_err(|error| {
                    AppError::Api(format!(
                        "Failed to parse OpenAI response as JSON object: {}",
                        error
                    ))
                })?;
            all_responses.extend(parsed);
        }

        Ok(all_responses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_prompt_without_glossary() {
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "Hello".into(),
                notes: String::new(),
            }],
            &[],
            &TranslationBatchContext::default(),
        )
        .unwrap();
        assert!(bundle.system_prompt.contains("ja"));
        assert!(bundle.system_prompt.contains("es"));
        assert!(bundle.system_prompt.contains("\"translations\""));
        assert!(!bundle.user_payload.contains("\"gl\""));
    }

    #[test]
    fn test_build_system_prompt_with_glossary() {
        let glossary = vec![GlossaryTerm {
            source: "勇者".to_string(),
            target: "Héroe".to_string(),
            case_sensitive: true,
        }];
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "勇者".into(),
                notes: String::new(),
            }],
            &glossary,
            &TranslationBatchContext::default(),
        )
        .unwrap();
        assert!(bundle
            .user_payload
            .contains("\"gl\":[{\"s\":\"勇者\",\"t\":\"Héroe\"}]"));
    }

    #[test]
    fn test_provider_name() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            Some("test-model".to_string()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(provider.name(), "OpenAI");
    }

    #[test]
    fn test_requires_model_and_defaults_endpoint() {
        let error = match OpenAiProvider::new("test-key".to_string(), None, None, None, None) {
            Ok(_) => panic!("OpenAI provider should require an explicit model"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("Select an OpenAI model"));

        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            Some("test-model".to_string()),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(provider.model, "test-model");
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
        )
        .unwrap();
        assert_eq!(provider.model, "gpt-4o");
        assert_eq!(provider.endpoint, "https://custom.api.com/v1/chat");
        assert_eq!(provider.organization.as_deref(), Some("org_123"));
        assert_eq!(provider.project.as_deref(), Some("proj_456"));
    }

    #[test]
    fn test_system_prompt_mentions_control_codes() {
        let prompt = OpenAiProvider::build_system_prompt(
            "ja",
            "es",
            &[],
            &TranslationBatchContext::default(),
        );
        assert!(prompt.contains("\\V[n]") || prompt.contains("control codes"));
        assert!(prompt.contains("placeholders"));
        assert!(prompt.contains("game text"));
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
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".into(),
                file_context: "ctx".into(),
                text: "勇者 魔王".into(),
                notes: String::new(),
            }],
            &glossary,
            &TranslationBatchContext::default(),
        )
        .unwrap();
        assert!(bundle.user_payload.contains("勇者"));
        assert!(bundle.user_payload.contains("Héroe"));
        assert!(bundle.user_payload.contains("魔王"));
        assert!(bundle.user_payload.contains("Rey Demonio"));
    }

    #[test]
    fn test_empty_scoping_headers_are_normalized_to_none() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            Some("test-model".to_string()),
            None,
            Some("   ".to_string()),
            Some(String::new()),
        )
        .unwrap();
        assert_eq!(provider.organization, None);
        assert_eq!(provider.project, None);
    }

    #[test]
    fn test_apply_auth_headers_includes_optional_scoping() {
        let provider = OpenAiProvider::new(
            "test-key".to_string(),
            Some("test-model".to_string()),
            None,
            Some("org_123".to_string()),
            Some("proj_456".to_string()),
        )
        .unwrap();

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

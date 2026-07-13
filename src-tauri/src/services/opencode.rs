use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::ai_models::{self, ProviderModel};
use crate::services::compact_prompt;
use crate::services::endpoint::normalize_https_endpoint_override;
use crate::services::provider::{
    approximate_token_count, provider_http_client, TranslationBatchContext, TranslationProvider,
    TranslationRequest, TranslationResponse,
};

const MAX_BATCH_SIZE: usize = 128;
const OPENCODE_WRAPPER_TOKEN_OVERHEAD: usize = 128;
const OPENCODE_RESPONSE_TOKEN_BUDGET: usize = 3200;
const OPENCODE_MAX_OUTPUT_TOKENS: usize = 4096;
const OPENCODE_TRANSIENT_RETRIES: usize = 2;
const DEFAULT_ENDPOINT_BASE: &str = "https://opencode.ai/zen/go/v1";
const NO_CACHED_FAMILY: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenCodeApiFamily {
    Responses,
    ChatCompletions,
    Messages,
    GoogleGenerateContent,
}

const GO_FAMILIES: &[OpenCodeApiFamily] = &[
    // OpenCode Go publishes models across different endpoint families. Probe
    // dynamically and cache the first family that also satisfies our schema.
    // Source: https://opencode.ai/docs/go/#endpoints
    OpenCodeApiFamily::ChatCompletions,
    OpenCodeApiFamily::Messages,
    OpenCodeApiFamily::Responses,
    OpenCodeApiFamily::GoogleGenerateContent,
];

pub struct OpenCodeProvider {
    client: Client,
    api_key: String,
    model: String,
    endpoint_base: String,
    disable_chat_thinking: AtomicBool,
    structured_family: AtomicU8,
}

impl OpenCodeProvider {
    pub(crate) const fn max_batch_size() -> usize {
        MAX_BATCH_SIZE
    }

    pub(crate) const fn response_token_budget() -> usize {
        OPENCODE_RESPONSE_TOKEN_BUDGET
    }

    pub(crate) fn estimate_response_tokens(requests: &[TranslationRequest]) -> usize {
        const PER_ENTRY_JSON_TOKENS: usize = 8;
        const ENVELOPE_JSON_TOKENS: usize = 16;

        let source_tokens = requests
            .iter()
            .map(|request| approximate_token_count(&request.text).saturating_sub(8))
            .sum::<usize>();
        let translated_text_tokens = source_tokens.saturating_mul(3).div_ceil(2);

        translated_text_tokens
            .saturating_add(requests.len().saturating_mul(PER_ENTRY_JSON_TOKENS))
            .saturating_add(ENVELOPE_JSON_TOKENS)
    }

    pub fn new(
        api_key: String,
        model: Option<String>,
        endpoint_base: Option<String>,
    ) -> Result<Self, AppError> {
        let model = model
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Api("Select an OpenCode model before translating".into()))?;
        let endpoint_base = normalize_opencode_endpoint(endpoint_base)?;

        Ok(Self {
            client: opencode_client()?,
            api_key,
            model,
            endpoint_base,
            disable_chat_thinking: AtomicBool::new(false),
            structured_family: AtomicU8::new(NO_CACHED_FAMILY),
        })
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
        .expect("serializing compact OpenCode prompt should not fail");
        approximate_token_count(&bundle.system_prompt)
            + approximate_token_count(&bundle.user_payload)
            + OPENCODE_WRAPPER_TOKEN_OVERHEAD
            + Self::estimate_response_tokens(requests)
    }

    pub async fn list_models(
        api_key: &str,
        endpoint_base: Option<String>,
    ) -> Result<Vec<ProviderModel>, AppError> {
        if api_key.trim().is_empty() {
            return Err(AppError::Api(
                "No API key configured for this provider".to_string(),
            ));
        }
        let client = opencode_client()?;
        let endpoint = normalize_opencode_endpoint(endpoint_base)?;
        ai_models::list_openai_compatible_models(&client, &endpoint, Some(api_key), &[]).await
    }

    fn structured_family_order(&self) -> Vec<OpenCodeApiFamily> {
        let cached = family_from_code(self.structured_family.load(Ordering::Relaxed));
        cached
            .into_iter()
            .chain(
                GO_FAMILIES
                    .iter()
                    .copied()
                    .filter(|family| Some(*family) != cached),
            )
            .collect()
    }

    fn remember_structured_family(&self, family: OpenCodeApiFamily) {
        self.structured_family
            .store(family_code(family), Ordering::Relaxed);
    }

    async fn translate_structured_bundle(
        &self,
        bundle: &compact_prompt::CompactPromptBundle,
        temperature: f32,
    ) -> Result<Vec<TranslationResponse>, AppError> {
        let mut attempt_errors = Vec::new();
        for family in self.structured_family_order() {
            match self
                .generate_text_with_family(
                    family,
                    &bundle.system_prompt,
                    &bundle.user_payload,
                    temperature,
                )
                .await
            {
                Ok(content) => {
                    match compact_prompt::parse_provider_response(&content, &bundle.entry_refs) {
                        Ok(parsed) => {
                            self.remember_structured_family(family);
                            return Ok(parsed);
                        }
                        Err(error) => attempt_errors.push(format!(
                            "{family:?} returned an invalid structured translation: {error}"
                        )),
                    }
                }
                Err(OpenCodeRequestError::Terminal(error)) => return Err(error),
                Err(OpenCodeRequestError::Retryable(error)) => {
                    attempt_errors.push(format!("{family:?}: {error}"));
                }
                Err(OpenCodeRequestError::RetryWithoutThinking(error)) => {
                    attempt_errors.push(format!("{family:?}: {error}"));
                }
            }
        }

        Err(AppError::Api(format!(
            "OpenCode did not return a valid structured translation from any supported API family. Attempts: {}",
            attempt_errors.join(" | ")
        )))
    }

    async fn generate_text_with_family(
        &self,
        family: OpenCodeApiFamily,
        system_prompt: &str,
        user_content: &str,
        temperature: f32,
    ) -> Result<String, OpenCodeRequestError> {
        let mut disable_chat_thinking = family == OpenCodeApiFamily::ChatCompletions
            && self.disable_chat_thinking.load(Ordering::Relaxed);
        let mut thinking_retry_reason = None;

        loop {
            match self
                .send_generation_request(
                    family,
                    system_prompt,
                    user_content,
                    temperature,
                    disable_chat_thinking,
                )
                .await
            {
                Ok(content) => return Ok(content),
                Err(OpenCodeRequestError::RetryWithoutThinking(error)) => {
                    if disable_chat_thinking {
                        return Err(OpenCodeRequestError::Terminal(error));
                    }
                    thinking_retry_reason = Some(error.to_string());
                    self.disable_chat_thinking.store(true, Ordering::Relaxed);
                    disable_chat_thinking = true;
                }
                Err(OpenCodeRequestError::Retryable(error)) => {
                    if let Some(reason) = thinking_retry_reason.take() {
                        return Err(OpenCodeRequestError::Retryable(AppError::Api(format!(
                            "{reason}; retry with thinking disabled also failed: {error}"
                        ))));
                    }
                    return Err(OpenCodeRequestError::Retryable(error));
                }
                Err(OpenCodeRequestError::Terminal(error)) => {
                    if let Some(reason) = thinking_retry_reason.take() {
                        return Err(OpenCodeRequestError::Terminal(AppError::Api(format!(
                            "{reason}; retry with thinking disabled also failed: {error}"
                        ))));
                    }
                    return Err(OpenCodeRequestError::Terminal(error));
                }
            }
        }
    }

    async fn send_generation_request(
        &self,
        family: OpenCodeApiFamily,
        system_prompt: &str,
        user_content: &str,
        temperature: f32,
        disable_chat_thinking: bool,
    ) -> Result<String, OpenCodeRequestError> {
        let url = endpoint_for_family(&self.endpoint_base, &self.model, family)
            .map_err(OpenCodeRequestError::Terminal)?;
        let temperature = normalized_temperature(temperature);
        let mut request = match family {
            OpenCodeApiFamily::Responses => serde_json::json!({
                "model": self.model,
                "input": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_content }
                ],
                "temperature": temperature,
                "max_output_tokens": OPENCODE_MAX_OUTPUT_TOKENS
            }),
            OpenCodeApiFamily::ChatCompletions => serde_json::json!({
                "model": self.model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_content }
                ],
                "temperature": temperature,
                "max_tokens": OPENCODE_MAX_OUTPUT_TOKENS
            }),
            OpenCodeApiFamily::Messages => serde_json::json!({
                "model": self.model,
                "system": system_prompt,
                "messages": [
                    { "role": "user", "content": user_content }
                ],
                "max_tokens": OPENCODE_MAX_OUTPUT_TOKENS,
                "temperature": temperature
            }),
            OpenCodeApiFamily::GoogleGenerateContent => serde_json::json!({
                "systemInstruction": {
                    "parts": [{ "text": system_prompt }]
                },
                "contents": [
                    {
                        "role": "user",
                        "parts": [{ "text": user_content }]
                    }
                ],
                "generationConfig": {
                    "temperature": temperature,
                    "maxOutputTokens": OPENCODE_MAX_OUTPUT_TOKENS
                }
            }),
        };
        if family == OpenCodeApiFamily::ChatCompletions && disable_chat_thinking {
            if let Some(request) = request.as_object_mut() {
                request.insert(
                    "thinking".to_string(),
                    serde_json::json!({ "type": "disabled" }),
                );
            }
        }

        for attempt in 0..=OPENCODE_TRANSIENT_RETRIES {
            let response = match self
                .client
                .post(url.clone())
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&request)
                .send()
                .await
            {
                Ok(response) => response,
                Err(error)
                    if is_transient_request_error(&error)
                        && attempt < OPENCODE_TRANSIENT_RETRIES =>
                {
                    tokio::time::sleep(transient_retry_delay(attempt)).await;
                    continue;
                }
                Err(error) => return Err(OpenCodeRequestError::Terminal(AppError::from(error))),
            };

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                let safe_body = sanitize_error_body(&body, &self.api_key);
                let error = AppError::Api(format!(
                    "OpenCode API error via {:?} ({}): {}",
                    family, status, safe_body
                ));
                if is_transient_response(status) && attempt < OPENCODE_TRANSIENT_RETRIES {
                    tokio::time::sleep(transient_retry_delay(attempt)).await;
                    continue;
                }
                if is_terminal_response(status, &body) {
                    return Err(OpenCodeRequestError::Terminal(error));
                }
                return Err(OpenCodeRequestError::Retryable(error));
            }

            let raw = response
                .text()
                .await
                .map_err(|error| OpenCodeRequestError::Terminal(AppError::from(error)))?;
            return parse_generation_text(family, &raw).map_err(|error| {
                if !disable_chat_thinking
                    && family == OpenCodeApiFamily::ChatCompletions
                    && chat_response_needs_thinking_disabled(&raw)
                {
                    OpenCodeRequestError::RetryWithoutThinking(error)
                } else {
                    OpenCodeRequestError::Terminal(error)
                }
            });
        }

        Err(OpenCodeRequestError::Retryable(AppError::Api(
            "OpenCode transient retry loop ended without a response".into(),
        )))
    }
}

fn opencode_client() -> Result<Client, AppError> {
    provider_http_client()
}

#[derive(Debug)]
enum OpenCodeRequestError {
    Terminal(AppError),
    Retryable(AppError),
    RetryWithoutThinking(AppError),
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ResponsesResponse {
    #[serde(default)]
    output_text: Option<String>,
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
}

#[derive(Deserialize)]
struct ResponsesOutputItem {
    #[serde(default)]
    content: Vec<ResponsesContentItem>,
}

#[derive(Deserialize)]
struct ResponsesContentItem {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentItem>,
}

#[derive(Deserialize)]
struct AnthropicContentItem {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct GoogleResponse {
    #[serde(default)]
    candidates: Vec<GoogleCandidate>,
}

#[derive(Deserialize)]
struct GoogleCandidate {
    content: GoogleContent,
}

#[derive(Deserialize)]
struct GoogleContent {
    #[serde(default)]
    parts: Vec<GooglePart>,
}

#[derive(Deserialize)]
struct GooglePart {
    #[serde(default)]
    text: Option<String>,
}

#[async_trait::async_trait]
impl TranslationProvider for OpenCodeProvider {
    fn name(&self) -> &'static str {
        "OpenCode"
    }

    async fn validate(&self) -> Result<(), AppError> {
        let request = TranslationRequest {
            entry_id: "fairtranslator-structured-readiness".into(),
            file_context: "Provider readiness check".into(),
            text: "Ready".into(),
            notes: String::new(),
        };
        let bundle = compact_prompt::build_provider_bundle(
            "English",
            "Spanish",
            std::slice::from_ref(&request),
            &[],
            &TranslationBatchContext {
                project_name: "FairTranslator readiness".into(),
                file_path: "provider-readiness".into(),
                file_kind: "Validation".into(),
                chunk_index: 1,
                total_chunks: 1,
                ..TranslationBatchContext::default()
            },
        )?;
        let responses = self.translate_structured_bundle(&bundle, 0.0).await?;
        if responses.len() != 1 || responses[0].translated_text.trim().is_empty() {
            return Err(AppError::Api(
                "OpenCode did not pass the structured translation readiness check".into(),
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
        context: &TranslationBatchContext,
    ) -> Result<Vec<TranslationResponse>, AppError> {
        let mut all_responses = Vec::with_capacity(requests.len());

        for chunk in requests.chunks(MAX_BATCH_SIZE) {
            let bundle = compact_prompt::build_provider_bundle(
                source_lang,
                target_lang,
                chunk,
                glossary,
                context,
            )?;
            let parsed = self.translate_structured_bundle(&bundle, 0.2).await?;
            all_responses.extend(parsed);
        }

        Ok(all_responses)
    }
}

const fn family_code(family: OpenCodeApiFamily) -> u8 {
    match family {
        OpenCodeApiFamily::Responses => 1,
        OpenCodeApiFamily::ChatCompletions => 2,
        OpenCodeApiFamily::Messages => 3,
        OpenCodeApiFamily::GoogleGenerateContent => 4,
    }
}

const fn family_from_code(code: u8) -> Option<OpenCodeApiFamily> {
    match code {
        1 => Some(OpenCodeApiFamily::Responses),
        2 => Some(OpenCodeApiFamily::ChatCompletions),
        3 => Some(OpenCodeApiFamily::Messages),
        4 => Some(OpenCodeApiFamily::GoogleGenerateContent),
        _ => None,
    }
}

fn is_terminal_response(status: StatusCode, body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    if status == StatusCode::UNAUTHORIZED
        && (normalized.contains("modelerror")
            || normalized.contains("invalid model")
            || normalized.contains("model not found")
            || normalized.contains("not supported"))
    {
        return false;
    }

    matches!(
        status,
        StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::PAYMENT_REQUIRED
    )
}

fn is_transient_response(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn is_transient_request_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn transient_retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(500 * (attempt as u64 + 1))
}

fn normalized_temperature(value: f32) -> f64 {
    ((value as f64) * 100.0).round() / 100.0
}

fn sanitize_error_body(body: &str, api_key: &str) -> String {
    let mut text = body.trim().to_string();
    if !api_key.is_empty() {
        text = text.replace(api_key, "[REDACTED]");
    }
    const WORKSPACE_URL: &str = "https://opencode.ai/workspace/";
    while let Some(start) = text.find(WORKSPACE_URL) {
        let end = text[start..]
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | '}' | ']'))
            .map(|offset| start + offset)
            .unwrap_or(text.len());
        text.replace_range(start..end, "OpenCode billing page");
    }
    if text.chars().count() > 1000 {
        text = text.chars().take(1000).collect::<String>();
        text.push_str("...");
    }
    text
}

fn normalize_opencode_endpoint(endpoint_base: Option<String>) -> Result<String, AppError> {
    let endpoint = normalize_https_endpoint_override(endpoint_base)?
        .unwrap_or_else(|| DEFAULT_ENDPOINT_BASE.to_string());
    ensure_go_endpoint(&endpoint)?;
    Ok(endpoint)
}

fn ensure_go_endpoint(endpoint_base: &str) -> Result<(), AppError> {
    let url = Url::parse(endpoint_base)
        .map_err(|_| AppError::Api("OpenCode endpoint must be a valid HTTPS URL".into()))?;
    if url.path().contains("/zen/go/") {
        Ok(())
    } else {
        Err(AppError::Api(
            "OpenCode is limited to Go endpoints for now. Use https://opencode.ai/zen/go/v1."
                .into(),
        ))
    }
}

fn endpoint_for_family(
    endpoint_base: &str,
    model: &str,
    family: OpenCodeApiFamily,
) -> Result<String, AppError> {
    let mut url = Url::parse(endpoint_base)
        .map_err(|_| AppError::Api("OpenCode endpoint must be a valid HTTPS URL".into()))?;
    let path = url.path().trim_end_matches('/');
    let base_path = if let Some(base) = path.strip_suffix("/chat/completions") {
        base
    } else if let Some(base) = path.strip_suffix("/responses") {
        base
    } else if let Some(base) = path.strip_suffix("/messages") {
        base
    } else if let Some(base) = path.strip_suffix("/models") {
        base
    } else {
        path
    };
    let next_path = match family {
        OpenCodeApiFamily::Responses => format!("{base_path}/responses"),
        OpenCodeApiFamily::ChatCompletions => format!("{base_path}/chat/completions"),
        OpenCodeApiFamily::Messages => format!("{base_path}/messages"),
        OpenCodeApiFamily::GoogleGenerateContent => {
            format!(
                "{base_path}/models/{}:generateContent",
                urlencoding_safe_path_segment(model)
            )
        }
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

fn urlencoding_safe_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect::<Vec<_>>(),
        })
        .collect()
}

fn parse_generation_text(family: OpenCodeApiFamily, raw: &str) -> Result<String, AppError> {
    match family {
        OpenCodeApiFamily::Responses => parse_responses_text(raw),
        OpenCodeApiFamily::ChatCompletions => parse_chat_text(raw),
        OpenCodeApiFamily::Messages => parse_anthropic_text(raw),
        OpenCodeApiFamily::GoogleGenerateContent => parse_google_text(raw),
    }
}

fn parse_chat_text(raw: &str) -> Result<String, AppError> {
    let parsed: ChatResponse = serde_json::from_str(raw)
        .map_err(|error| AppError::Api(format!("Could not parse chat response: {error}")))?;
    if let Some(content) = parsed
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
    {
        return Ok(content);
    }

    let finish_reason = parsed
        .choices
        .first()
        .and_then(|choice| choice.finish_reason.as_deref())
        .unwrap_or("unknown");
    Err(AppError::Api(format!(
        "Chat response did not include content (finish_reason: {finish_reason})"
    )))
}

fn chat_response_needs_thinking_disabled(raw: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<ChatResponse>(raw) else {
        return false;
    };

    parsed.choices.first().is_some_and(|choice| {
        let content_is_blank = choice
            .message
            .content
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty();
        content_is_blank
            && (json_value_has_content(choice.message.reasoning.as_ref())
                || json_value_has_content(choice.message.reasoning_content.as_ref())
                || choice
                    .finish_reason
                    .as_deref()
                    .is_some_and(|reason| reason.eq_ignore_ascii_case("length")))
    })
}

fn json_value_has_content(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::String(text)) => !text.trim().is_empty(),
        Some(serde_json::Value::Array(items)) => !items.is_empty(),
        Some(serde_json::Value::Object(fields)) => !fields.is_empty(),
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(_)) => true,
        _ => false,
    }
}

fn parse_responses_text(raw: &str) -> Result<String, AppError> {
    let parsed: ResponsesResponse = serde_json::from_str(raw)
        .map_err(|error| AppError::Api(format!("Could not parse responses response: {error}")))?;
    if let Some(output_text) = parsed
        .output_text
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Ok(output_text);
    }

    let text = parsed
        .output
        .into_iter()
        .flat_map(|item| item.content.into_iter())
        .filter_map(|content| content.text)
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        Err(AppError::Api(
            "Responses response did not include text".into(),
        ))
    } else {
        Ok(text.trim().to_string())
    }
}

fn parse_anthropic_text(raw: &str) -> Result<String, AppError> {
    let parsed: AnthropicResponse = serde_json::from_str(raw)
        .map_err(|error| AppError::Api(format!("Could not parse messages response: {error}")))?;
    let text = parsed
        .content
        .into_iter()
        .filter_map(|content| content.text)
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        Err(AppError::Api(
            "Messages response did not include text".into(),
        ))
    } else {
        Ok(text.trim().to_string())
    }
}

fn parse_google_text(raw: &str) -> Result<String, AppError> {
    let parsed: GoogleResponse = serde_json::from_str(raw)
        .map_err(|error| AppError::Api(format!("Could not parse Google response: {error}")))?;
    let text = parsed
        .candidates
        .into_iter()
        .flat_map(|candidate| candidate.content.parts.into_iter())
        .filter_map(|part| part.text)
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        Err(AppError::Api("Google response did not include text".into()))
    } else {
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_family_endpoints_from_go_base() {
        assert_eq!(
            endpoint_for_family(
                "https://opencode.ai/zen/go/v1",
                "glm-5",
                OpenCodeApiFamily::ChatCompletions
            )
            .unwrap(),
            "https://opencode.ai/zen/go/v1/chat/completions"
        );
        assert_eq!(
            endpoint_for_family(
                "https://opencode.ai/zen/go/v1",
                "minimax-m3",
                OpenCodeApiFamily::Messages
            )
            .unwrap(),
            "https://opencode.ai/zen/go/v1/messages"
        );
    }

    #[test]
    fn derives_google_endpoint_from_base() {
        assert_eq!(
            endpoint_for_family(
                "https://opencode.ai/zen/v1",
                "gemini-3.5-flash",
                OpenCodeApiFamily::GoogleGenerateContent
            )
            .unwrap(),
            "https://opencode.ai/zen/v1/models/gemini-3.5-flash:generateContent"
        );
    }

    #[test]
    fn caches_the_first_family_that_returns_a_valid_structured_translation() {
        let provider = OpenCodeProvider::new(
            "key".into(),
            Some("dynamic-model".into()),
            Some("https://opencode.ai/zen/go/v1".into()),
        )
        .unwrap();

        assert_eq!(provider.structured_family_order(), GO_FAMILIES);

        provider.remember_structured_family(OpenCodeApiFamily::Messages);
        let reordered = provider.structured_family_order();

        assert_eq!(reordered[0], OpenCodeApiFamily::Messages);
        assert_eq!(reordered.len(), GO_FAMILIES.len());
        assert!(GO_FAMILIES.iter().all(|family| reordered.contains(family)));
    }

    #[test]
    fn estimates_response_growth_and_keeps_short_batches_within_budget() {
        let short_requests = (0..128)
            .map(|index| TranslationRequest {
                entry_id: format!("entry-{index}"),
                file_context: "dialogue".into(),
                text: "Short dialogue line".into(),
                notes: String::new(),
            })
            .collect::<Vec<_>>();
        let long_requests = (0..128)
            .map(|index| TranslationRequest {
                entry_id: format!("entry-{index}"),
                file_context: "dialogue".into(),
                text: "x".repeat(1_000),
                notes: String::new(),
            })
            .collect::<Vec<_>>();

        assert!(
            OpenCodeProvider::estimate_response_tokens(&short_requests)
                <= OpenCodeProvider::response_token_budget()
        );
        assert!(
            OpenCodeProvider::estimate_response_tokens(&long_requests)
                > OpenCodeProvider::response_token_budget()
        );
    }

    #[test]
    fn accepts_only_go_endpoints_for_now() {
        assert!(ensure_go_endpoint("https://opencode.ai/zen/go/v1").is_ok());
        assert!(ensure_go_endpoint("https://opencode.ai/zen/v1").is_err());
        assert!(OpenCodeProvider::new(
            "key".into(),
            Some("kimi-k2.6".into()),
            Some("https://opencode.ai/zen/v1".into())
        )
        .is_err());
    }

    #[test]
    fn rejects_http_go_endpoints_before_sending_api_keys() {
        let error = OpenCodeProvider::new(
            "key".into(),
            Some("kimi-k2.6".into()),
            Some("http://opencode.ai/zen/go/v1".into()),
        )
        .err()
        .expect("HTTP endpoints must be rejected");

        assert!(error.to_string().contains("HTTPS"));
    }

    #[test]
    fn parses_supported_response_shapes() {
        assert_eq!(
            parse_generation_text(
                OpenCodeApiFamily::ChatCompletions,
                r#"{"choices":[{"message":{"content":"hello"}}]}"#
            )
            .unwrap(),
            "hello"
        );
        assert_eq!(
            parse_generation_text(OpenCodeApiFamily::Responses, r#"{"output_text":"hello"}"#)
                .unwrap(),
            "hello"
        );
        assert_eq!(
            parse_generation_text(
                OpenCodeApiFamily::Messages,
                r#"{"content":[{"type":"text","text":"hello"}]}"#
            )
            .unwrap(),
            "hello"
        );
        assert_eq!(
            parse_generation_text(
                OpenCodeApiFamily::GoogleGenerateContent,
                r#"{"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}"#
            )
            .unwrap(),
            "hello"
        );
    }

    #[test]
    fn detects_reasoning_only_chat_responses() {
        assert!(chat_response_needs_thinking_disabled(
            r#"{"choices":[{"finish_reason":"length","message":{"content":null,"reasoning":"internal reasoning"}}]}"#
        ));
        assert!(chat_response_needs_thinking_disabled(
            r#"{"choices":[{"finish_reason":"length","message":{"content":"","reasoning_content":[{"type":"text","text":"thinking"}]}}]}"#
        ));
        assert!(chat_response_needs_thinking_disabled(
            r#"{"choices":[{"finish_reason":"length","message":{"content":null}}]}"#
        ));
        assert!(!chat_response_needs_thinking_disabled(
            r#"{"choices":[{"finish_reason":"stop","message":{"content":"ok","reasoning":"internal reasoning"}}]}"#
        ));
        assert!(!chat_response_needs_thinking_disabled(
            r#"{"choices":[{"finish_reason":"stop","message":{"content":null}}]}"#
        ));
    }

    #[test]
    fn distinguishes_route_errors_from_account_errors() {
        assert!(!is_terminal_response(
            StatusCode::UNAUTHORIZED,
            r#"{"type":"error","error":{"type":"ModelError","message":"Invalid model for this endpoint"}}"#
        ));
        assert!(is_terminal_response(
            StatusCode::UNAUTHORIZED,
            r#"{"type":"error","error":{"type":"CreditsError","message":"Insufficient balance"}}"#
        ));
    }

    #[test]
    fn marks_server_errors_as_transient() {
        assert!(is_transient_response(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_transient_response(StatusCode::BAD_GATEWAY));
        assert!(is_transient_response(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_transient_response(StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_transient_response(StatusCode::UNAUTHORIZED));
        assert!(!is_transient_response(StatusCode::PAYMENT_REQUIRED));
    }

    #[test]
    fn normalizes_temperature_to_two_decimals() {
        assert_eq!(normalized_temperature(0.3), 0.3);
        assert_eq!(normalized_temperature(0.126), 0.13);
    }

    #[test]
    fn sanitizes_workspace_urls_in_error_bodies() {
        let sanitized = sanitize_error_body(
            r#"{"type":"error","error":{"message":"Insufficient balance. Manage your billing here: https://opencode.ai/workspace/wrk_123/billing"}}"#,
            "secret-value",
        );

        assert!(sanitized.contains("OpenCode billing page"));
        assert!(!sanitized.contains("wrk_123"));
    }

    #[test]
    fn sanitizes_api_keys_in_error_bodies() {
        let sanitized = sanitize_error_body(
            r#"{"error":"API key secret-value was rejected"}"#,
            "secret-value",
        );

        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("secret-value"));
    }
}

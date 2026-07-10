/// Trait defining the interface for translation API providers.
///
/// Each provider implements this trait so the batch
/// translation system can work with any backend transparently.
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::{GlossaryTerm, ProviderType};

/// Provider settings exposed over IPC. API keys are always redacted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyInfo {
    pub provider: ProviderType,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub command_path: Option<String>,
    pub profile: Option<String>,
    pub auth_mode: Option<String>,
    pub is_configured: bool,
}

pub(crate) struct ProviderInfo {
    pub(crate) provider: ProviderType,
    pub(crate) api_key: String,
    pub(crate) model: Option<String>,
    pub(crate) endpoint: Option<String>,
    pub(crate) organization: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) command_path: Option<String>,
    pub(crate) profile: Option<String>,
    pub(crate) auth_mode: Option<String>,
    pub(crate) is_configured: bool,
}

impl From<ProviderInfo> for ApiKeyInfo {
    fn from(info: ProviderInfo) -> Self {
        Self {
            provider: info.provider,
            model: info.model,
            endpoint: info.endpoint,
            organization: info.organization,
            project: info.project,
            command_path: info.command_path,
            profile: info.profile,
            auth_mode: info.auth_mode,
            is_configured: info.is_configured,
        }
    }
}

pub(crate) fn secret_update(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// A single text to translate with its entry ID for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    pub entry_id: String,
    pub file_context: String,
    pub text: String,
    pub notes: String,
}

/// Result of translating a single entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResponse {
    pub entry_id: String,
    pub translated_text: String,
}

/// A protected or established term that should stay consistent across chunks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationTermHint {
    pub source: String,
    pub target: Option<String>,
    pub note: String,
}

/// A prior translation example used as lightweight translation memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationExample {
    pub file_path: String,
    pub file_context: String,
    pub original: String,
    pub translation: String,
    pub notes: String,
}

/// Extra context passed to LLM-capable providers for chunk-aware translation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationBatchContext {
    pub project_name: String,
    pub file_path: String,
    pub file_kind: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub approximate_char_count: usize,
    pub style_guide: Vec<String>,
    pub term_hints: Vec<TranslationTermHint>,
    pub translation_examples: Vec<TranslationExample>,
    pub file_context_hints: Vec<String>,
    pub previous_chunk_hints: Vec<String>,
    pub next_chunk_hints: Vec<String>,
}

/// Stable approximate token estimator for prompt-budget planning.
///
/// This intentionally overestimates mixed-language/game text a bit so chunking
/// stays conservative when prompts contain JSON, control codes, and project
/// metadata.
pub fn approximate_token_count(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }

    let total_chars = text.chars().count();
    let non_ascii_chars = text.chars().filter(|ch| !ch.is_ascii()).count();
    let ascii_chars = total_chars.saturating_sub(non_ascii_chars);
    let line_count = text.lines().count();
    let structural_chars = text
        .chars()
        .filter(|ch| {
            matches!(
                ch,
                '{' | '}' | '[' | ']' | '"' | ':' | ',' | '\\' | '-' | '>' | '<'
            )
        })
        .count();

    ascii_chars.div_ceil(4)
        + non_ascii_chars.div_ceil(2)
        + line_count
        + structural_chars.div_ceil(12)
        + 8
}

/// All translation providers implement this async trait.
///
/// `translate_batch` receives a slice of requests and returns responses.
/// Providers should handle their own rate limiting internally.
#[async_trait::async_trait]
pub trait TranslationProvider: Send + Sync {
    /// Translate a batch of texts from `source_lang` to `target_lang`.
    ///
    /// The `glossary` slice provides term pairs that should be respected
    /// during translation (e.g., character names, item names).
    ///
    /// Providers may chunk the batch internally to respect API limits.
    async fn translate_batch(
        &self,
        requests: &[TranslationRequest],
        source_lang: &str,
        target_lang: &str,
        glossary: &[GlossaryTerm],
        context: &TranslationBatchContext,
    ) -> Result<Vec<TranslationResponse>, AppError>;

    /// Human-readable name of this provider (for logs/UI)
    fn name(&self) -> &'static str;

    /// Validate that the provider's API key and config are working.
    async fn validate(&self) -> Result<(), AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_info(api_key: &str) -> ProviderInfo {
        ProviderInfo {
            provider: ProviderType::Opencode,
            api_key: api_key.to_string(),
            model: Some("test-model".into()),
            endpoint: None,
            organization: None,
            project: None,
            command_path: None,
            profile: None,
            auth_mode: Some("apiKey".into()),
            is_configured: true,
        }
    }

    #[test]
    fn public_provider_info_never_contains_the_api_key() {
        let internal = provider_info("secret-value");
        assert!(!internal.api_key.is_empty());
        let public: ApiKeyInfo = internal.into();

        let serialized = serde_json::to_value(&public).unwrap();
        assert!(serialized.get("apiKey").is_none());
        assert!(public.is_configured);
    }

    #[test]
    fn empty_api_key_updates_preserve_the_saved_secret() {
        assert_eq!(secret_update(""), None);
        assert_eq!(secret_update("   "), None);
        assert_eq!(secret_update("  replacement  "), Some("replacement"));
    }
}

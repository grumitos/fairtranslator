/// Trait defining the interface for translation API providers.
///
/// Each provider (OpenAI, DeepL, Google) implements this trait so the batch
/// translation system can work with any backend transparently.
use crate::error::AppError;
use crate::models::GlossaryTerm;

/// A single text to translate with its entry ID for tracking
#[derive(Debug, Clone)]
pub struct TranslationRequest {
    pub entry_id: String,
    pub text: String,
}

/// Result of translating a single entry
#[derive(Debug, Clone)]
pub struct TranslationResponse {
    pub entry_id: String,
    pub translated_text: String,
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
    ) -> Result<Vec<TranslationResponse>, AppError>;

    /// Human-readable name of this provider (for logs/UI)
    fn name(&self) -> &'static str;

    /// Validate that the provider's API key and config are working.
    async fn validate(&self) -> Result<(), AppError>;
}

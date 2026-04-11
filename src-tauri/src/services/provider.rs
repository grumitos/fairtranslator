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
    pub file_context: String,
    pub text: String,
    pub notes: String,
}

/// Result of translating a single entry
#[derive(Debug, Clone)]
pub struct TranslationResponse {
    pub entry_id: String,
    pub translated_text: String,
}

/// A protected or established term that should stay consistent across chunks.
#[derive(Debug, Clone)]
pub struct TranslationTermHint {
    pub source: String,
    pub target: Option<String>,
    pub note: String,
}

/// A prior translation example used as lightweight translation memory.
#[derive(Debug, Clone)]
pub struct TranslationExample {
    pub file_path: String,
    pub file_context: String,
    pub original: String,
    pub translation: String,
    pub notes: String,
}

/// Extra context passed to LLM-capable providers for chunk-aware translation.
#[derive(Debug, Clone, Default)]
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

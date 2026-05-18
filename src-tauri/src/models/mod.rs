use serde::{Deserialize, Serialize};

/// Supported RPG Maker engine types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Mv,
    Mz,
    VxAce,
    Vx,
    Xp,
}

/// Translation status for individual entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum EntryStatus {
    Untranslated,
    Draft,
    Reviewed,
    Final,
}

/// Supported translation API providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ProviderType {
    Openai,
    Deepl,
    Google,
    Codex,
}

/// Root project model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine: Engine,
    pub game_dir: String,
    pub files: Vec<TranslationFile>,
    pub glossary: Vec<GlossaryTerm>,
    #[serde(default)]
    pub style_guide: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A single game file and its translatable entries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TranslationFile {
    pub relative_path: String,
    pub entries: Vec<TranslationEntry>,
}

/// A single translatable text entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TranslationEntry {
    pub id: String,
    pub file_context: String,
    pub original: String,
    pub translation: String,
    pub status: EntryStatus,
    pub notes: String,
}

/// A glossary term for translation consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GlossaryTerm {
    pub source: String,
    pub target: String,
    pub case_sensitive: bool,
}

/// Progress event payload emitted during batch translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub files_processed: usize,
    pub total_files: usize,
    pub chunks_processed: usize,
    pub total_chunks: usize,
    pub entries_translated: usize,
    pub total_entries: usize,
    pub current_file: String,
    pub current_file_chunks_processed: usize,
    pub current_file_total_chunks: usize,
    pub failed_entries: usize,
    pub failed_batches: usize,
    pub warnings: Vec<String>,
}

/// A single entry exported for manual or AI-assisted translation outside the app UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBatchEntry {
    pub file_path: String,
    pub entry_id: String,
    pub file_context: String,
    pub original: String,
    pub current_translation: String,
    pub instruction_candidate: bool,
    pub instruction_reason: Option<String>,
}

/// Structured payload returned when preparing a translation batch for an AI model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBatchExport {
    pub project_id: String,
    pub project_name: String,
    pub batch_id: String,
    pub engine: Engine,
    pub source_lang: String,
    pub target_lang: String,
    pub total_selected_entries: usize,
    pub exported_entries: usize,
    pub entries: Vec<AiBatchEntry>,
    #[serde(default)]
    pub response_map: Vec<AiBatchResponseRef>,
    pub prompt: String,
}

/// Mapping used by compact external-AI prompts to recover real project IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBatchResponseRef {
    pub local_id: u16,
    pub file_path: String,
    pub entry_id: String,
}

/// A single translated entry returned by an external AI workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBatchTranslation {
    pub file_path: String,
    pub entry_id: String,
    pub translation: String,
    pub notes: Option<String>,
    pub skip: bool,
}

/// Result returned after applying an AI-generated batch back into the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AiBatchApplyResult {
    pub applied_entries: usize,
    pub skipped_entries: usize,
    pub missing_entries: Vec<String>,
    pub duplicate_entries: Vec<String>,
    pub invalid_entries: Vec<String>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod typescript_bindings {
    use std::{fs, path::Path};

    use super::*;
    use ts_rs::TS;

    const GENERATED_DOMAIN_HEADER: &str = "\
/* eslint-disable */
// @generated by `npm run types:generate`. Do not edit by hand.

";

    fn append_type<T: TS>(output: &mut String) {
        let declaration = T::decl();
        if declaration.starts_with("type ") || declaration.starts_with("interface ") {
            output.push_str("export ");
        }
        output.push_str(&declaration);
        output.push_str("\n\n");
    }

    fn generated_domain_types() -> String {
        let mut output = GENERATED_DOMAIN_HEADER.to_string();

        append_type::<Engine>(&mut output);
        append_type::<EntryStatus>(&mut output);
        append_type::<ProviderType>(&mut output);
        append_type::<Project>(&mut output);
        append_type::<TranslationFile>(&mut output);
        append_type::<TranslationEntry>(&mut output);
        append_type::<GlossaryTerm>(&mut output);
        append_type::<BatchProgress>(&mut output);
        append_type::<AiBatchEntry>(&mut output);
        append_type::<AiBatchExport>(&mut output);
        append_type::<AiBatchResponseRef>(&mut output);
        append_type::<AiBatchTranslation>(&mut output);
        append_type::<AiBatchApplyResult>(&mut output);

        if output.ends_with("\n\n") {
            output.pop();
        }

        output
    }

    fn generated_domain_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("src")
            .join("types")
            .join("generated")
            .join("domain.ts")
    }

    #[test]
    fn generated_typescript_domain_bindings_are_current() {
        let path = generated_domain_path();
        let expected = generated_domain_types();
        let current = fs::read_to_string(&path).unwrap_or_default();
        let current = current.replace("\r\n", "\n");

        assert_eq!(
            current, expected,
            "generated TypeScript domain bindings are stale; run `npm run types:generate`"
        );
    }

    #[test]
    #[ignore = "regenerates frontend TypeScript bindings"]
    fn export_typescript_bindings() {
        let path = generated_domain_path();
        let parent = path
            .parent()
            .expect("generated bindings path should have a parent");
        fs::create_dir_all(parent).expect("create generated bindings directory");
        fs::write(path, generated_domain_types()).expect("write generated TypeScript bindings");
    }
}

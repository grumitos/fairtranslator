use std::collections::{HashMap, HashSet};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(not(test))]
use tauri::path::BaseDirectory;
#[cfg(not(test))]
use tauri::{AppHandle, Manager};
use tokio::task;

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::provider::{TranslationProvider, TranslationRequest, TranslationResponse};

const DEFAULT_COMMAND_PATH: &str = "codex";
const SIDECAR_BASENAME: &str = "codex";

#[cfg(not(test))]
type RuntimeAppHandle = AppHandle;
#[cfg(test)]
type RuntimeAppHandle = ();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeStatus {
    pub state: String,
    pub effective_command_path: String,
    pub resolved_command_path: Option<String>,
    pub command_source: String,
    pub version: Option<String>,
    pub is_installed: bool,
    pub is_authenticated: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeSource {
    Sidecar,
    Configured,
    Path,
}

impl RuntimeSource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Sidecar => "sidecar",
            Self::Configured => "configured",
            Self::Path => "path",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeCandidate {
    source: RuntimeSource,
    effective_command_path: String,
    resolved_command_path: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedRuntime {
    candidate: RuntimeCandidate,
    version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeFailureState {
    NotInstalled,
    Error,
}

#[derive(Debug, Clone)]
struct RuntimeResolutionFailure {
    candidate: RuntimeCandidate,
    state: RuntimeFailureState,
    message: String,
    version: Option<String>,
}

pub struct CodexProvider {
    app_handle: Option<RuntimeAppHandle>,
    configured_command_path: Option<String>,
    model: Option<String>,
    profile: Option<String>,
}

impl CodexProvider {
    pub fn new(
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
    ) -> Self {
        Self::from_parts(None, command_path, model, profile)
    }

    pub fn with_app_handle(
        app_handle: RuntimeAppHandle,
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
    ) -> Self {
        Self::from_parts(Some(app_handle), command_path, model, profile)
    }

    fn from_parts(
        app_handle: Option<RuntimeAppHandle>,
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
    ) -> Self {
        Self {
            app_handle,
            configured_command_path: command_path.and_then(normalize_optional),
            model: model.and_then(normalize_optional),
            profile: profile.and_then(normalize_optional),
        }
    }

    fn build_prompt(
        source_lang: &str,
        target_lang: &str,
        requests: &[TranslationRequest],
        glossary: &[GlossaryTerm],
    ) -> Result<String, AppError> {
        let entries = requests
            .iter()
            .map(|request| {
                json!({
                    "entryId": request.entry_id,
                    "text": request.text,
                })
            })
            .collect::<Vec<_>>();
        let entries_json = serde_json::to_string_pretty(&entries)?;

        let mut prompt = format!(
            concat!(
                "You are a professional translator for RPG Maker video games.\n",
                "Translate every entry from {source_lang} to {target_lang}.\n",
                "Preserve all RPG Maker control codes, placeholders, escape sequences and line breaks.\n",
                "Return every requested entry exactly once.\n",
                "Do not invent entry IDs.\n",
                "Do not add explanations or markdown.\n",
                "Required output shape:\n",
                "{{\"translations\": [{{\"entryId\": \"...\", \"translatedText\": \"...\"}}]}}\n"
            ),
            source_lang = source_lang,
            target_lang = target_lang,
        );

        if !glossary.is_empty() {
            prompt.push_str("\nGlossary (use these translations consistently):\n");
            for term in glossary {
                prompt.push_str(&format!("- \"{}\" -> \"{}\"\n", term.source, term.target));
            }
        }

        prompt.push_str("\nEntries:\n");
        prompt.push_str(&entries_json);
        Ok(prompt)
    }

    fn response_schema() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "translations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "entryId": { "type": "string" },
                            "translatedText": { "type": "string" }
                        },
                        "required": ["entryId", "translatedText"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["translations"],
            "additionalProperties": false
        })
    }

    pub async fn runtime_status(&self) -> Result<CodexRuntimeStatus, AppError> {
        let runtime = match self.resolve_runtime().await {
            Ok(runtime) => runtime,
            Err(failure) => return Ok(status_from_failure(failure)),
        };

        let login_output = self
            .run_command(
                &runtime.candidate,
                vec!["login".to_string(), "status".to_string()],
                None,
                None,
            )
            .await?;
        let combined_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&login_output.stdout),
            String::from_utf8_lossy(&login_output.stderr)
        );
        let is_authenticated = combined_output.to_ascii_lowercase().contains("logged in");

        let (state, message) = if is_authenticated {
            (
                "ready".to_string(),
                if runtime.candidate.source == RuntimeSource::Sidecar {
                    "Bundled Codex runtime is installed and authenticated".to_string()
                } else {
                    "Codex CLI is installed and authenticated".to_string()
                },
            )
        } else if login_requires_auth(&combined_output) || login_output.status.success() {
            (
                "loginRequired".to_string(),
                "Codex CLI is installed but not authenticated. Run `codex login` first"
                    .to_string(),
            )
        } else {
            (
                "error".to_string(),
                format!(
                    "Codex CLI is installed but login status could not be confirmed: {}",
                    command_output_detail(&login_output)
                ),
            )
        };

        Ok(CodexRuntimeStatus {
            state,
            effective_command_path: runtime.candidate.effective_command_path,
            resolved_command_path: runtime.candidate.resolved_command_path,
            command_source: runtime.candidate.source.as_str().to_string(),
            version: runtime.version,
            is_installed: true,
            is_authenticated,
            message,
        })
    }

    async fn resolve_runtime(&self) -> Result<ResolvedRuntime, RuntimeResolutionFailure> {
        if let Some(sidecar_candidate) = self.sidecar_candidate() {
            if let Ok(runtime) = self.probe_runtime(sidecar_candidate).await {
                return Ok(runtime);
            }
        }

        if let Some(configured_path) = &self.configured_command_path {
            let resolved = resolve_command_path(configured_path);
            let candidate = RuntimeCandidate {
                source: RuntimeSource::Configured,
                effective_command_path: configured_path.clone(),
                resolved_command_path: resolved
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            };
            if resolved.is_none() {
                return Err(RuntimeResolutionFailure {
                    candidate,
                    state: RuntimeFailureState::NotInstalled,
                    message: "Configured Codex executable was not found".to_string(),
                    version: None,
                });
            }
            return self.probe_runtime(candidate).await;
        }

        let resolved = resolve_command_path(DEFAULT_COMMAND_PATH);
        let candidate = RuntimeCandidate {
            source: RuntimeSource::Path,
            effective_command_path: DEFAULT_COMMAND_PATH.to_string(),
            resolved_command_path: resolved
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        };
        if resolved.is_none() {
            return Err(RuntimeResolutionFailure {
                candidate,
                state: RuntimeFailureState::NotInstalled,
                message: "Codex CLI was not found on PATH".to_string(),
                version: None,
            });
        }

        self.probe_runtime(candidate).await
    }

    fn sidecar_candidate(&self) -> Option<RuntimeCandidate> {
        resolve_sidecar_path(self.app_handle.as_ref()).map(|path| RuntimeCandidate {
            source: RuntimeSource::Sidecar,
            effective_command_path: path.to_string_lossy().into_owned(),
            resolved_command_path: Some(path.to_string_lossy().into_owned()),
        })
    }

    async fn probe_runtime(
        &self,
        candidate: RuntimeCandidate,
    ) -> Result<ResolvedRuntime, RuntimeResolutionFailure> {
        let output = self
            .run_command(
                &candidate,
                vec!["--version".to_string()],
                None,
                None,
            )
            .await
            .map_err(|error| RuntimeResolutionFailure {
                candidate: candidate.clone(),
                state: RuntimeFailureState::Error,
                message: error.to_string(),
                version: None,
            })?;

        let version = extract_first_nonempty_line(&String::from_utf8_lossy(&output.stdout))
            .or_else(|| extract_first_nonempty_line(&String::from_utf8_lossy(&output.stderr)));

        if !output.status.success() {
            return Err(RuntimeResolutionFailure {
                candidate,
                state: RuntimeFailureState::Error,
                message: format!(
                    "Codex CLI was found but `codex --version` failed: {}",
                    command_output_detail(&output)
                ),
                version,
            });
        }

        Ok(ResolvedRuntime { candidate, version })
    }

    async fn run_exec(
        &self,
        runtime: &RuntimeCandidate,
        prompt: String,
        schema_path: PathBuf,
        output_path: PathBuf,
        workdir: PathBuf,
    ) -> Result<std::process::Output, AppError> {
        let mut args = vec![
            "exec".to_string(),
            "--sandbox".to_string(),
            "read-only".to_string(),
            "--skip-git-repo-check".to_string(),
            "--ephemeral".to_string(),
            "--output-schema".to_string(),
            schema_path.to_string_lossy().into_owned(),
            "--output-last-message".to_string(),
            output_path.to_string_lossy().into_owned(),
            "-C".to_string(),
            workdir.to_string_lossy().into_owned(),
        ];
        if let Some(model) = &self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }
        if let Some(profile) = &self.profile {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }
        args.push("--".to_string());
        args.push("-".to_string());
        self.run_command(runtime, args, Some(&workdir), Some(prompt))
            .await
    }

    async fn run_command(
        &self,
        runtime: &RuntimeCandidate,
        args: Vec<String>,
        current_dir: Option<&Path>,
        stdin_text: Option<String>,
    ) -> Result<std::process::Output, AppError> {
        let current_dir = current_dir.map(Path::to_path_buf);
        let mut command = self.build_command(runtime, &args, current_dir.as_deref())?;

        task::spawn_blocking(move || {
            if stdin_text.is_some() {
                command.stdin(Stdio::piped());
            }
            command.stdout(Stdio::piped()).stderr(Stdio::piped());

            let mut child = command.spawn().map_err(AppError::from)?;
            if let Some(stdin_text) = stdin_text {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin
                        .write_all(stdin_text.as_bytes())
                        .map_err(AppError::from)?;
                }
            }
            child.wait_with_output().map_err(AppError::from)
        })
        .await
        .map_err(|error| AppError::Project(format!("Failed to join Codex task: {error}")))?
    }

    fn build_command(
        &self,
        runtime: &RuntimeCandidate,
        args: &[String],
        current_dir: Option<&Path>,
    ) -> Result<Command, AppError> {
        match runtime.source {
            RuntimeSource::Sidecar | RuntimeSource::Configured | RuntimeSource::Path => {
                let mut command = Command::new(&runtime.effective_command_path);
                if let Some(current_dir) = current_dir {
                    command.current_dir(current_dir);
                }
                command.args(args);
                Ok(command)
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecResponse {
    translations: Vec<CodexExecTranslation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexExecTranslation {
    entry_id: String,
    translated_text: String,
}

fn normalize_optional(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn temp_codex_dir() -> PathBuf {
    std::env::temp_dir().join(format!("fairtranslator-codex-{}", uuid::Uuid::new_v4()))
}

fn status_from_failure(failure: RuntimeResolutionFailure) -> CodexRuntimeStatus {
    let is_installed = failure.state == RuntimeFailureState::Error;
    CodexRuntimeStatus {
        state: match failure.state {
            RuntimeFailureState::NotInstalled => "notInstalled".to_string(),
            RuntimeFailureState::Error => "error".to_string(),
        },
        effective_command_path: failure.candidate.effective_command_path,
        resolved_command_path: failure.candidate.resolved_command_path,
        command_source: failure.candidate.source.as_str().to_string(),
        version: failure.version,
        is_installed,
        is_authenticated: false,
        message: failure.message,
    }
}

fn command_output_detail(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else if !stdout.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        format!("exit status {}", output.status)
    }
}

fn command_failure(output: &std::process::Output, context: &str) -> AppError {
    let detail = command_output_detail(output);
    AppError::Api(format!("{context}: {detail}"))
}

fn strip_markdown_fences(raw_response: &str) -> &str {
    let trimmed = raw_response.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }

    let without_open = trimmed
        .split_once('\n')
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);

    without_open
        .rsplit_once("```")
        .map(|(body, _)| body.trim())
        .unwrap_or(without_open.trim())
}

fn extract_first_nonempty_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn login_requires_auth(output: &str) -> bool {
    let normalized = output.to_ascii_lowercase();
    normalized.contains("not logged in")
        || normalized.contains("run `codex login`")
        || normalized.contains("run codex login")
        || normalized.contains("authentication required")
        || normalized.contains("login required")
}

fn resolve_command_path(command_path: &str) -> Option<PathBuf> {
    let raw_path = Path::new(command_path);
    if raw_path.is_absolute() || raw_path.parent().is_some() {
        return raw_path.is_file().then(|| raw_path.to_path_buf());
    }

    let path_var = env::var_os("PATH")?;
    #[cfg(windows)]
    let pathexts = windows_pathexts();

    for dir in env::split_paths(&path_var) {
        #[cfg(windows)]
        let candidates = executable_candidates(&dir, command_path, &pathexts);
        #[cfg(not(windows))]
        let candidates = executable_candidates(&dir, command_path);

        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(windows)]
fn sidecar_filename() -> String {
    format!(
        "{SIDECAR_BASENAME}-{}.exe",
        env!("FAIRTRANSLATOR_TARGET_TRIPLE")
    )
}

#[cfg(not(windows))]
fn sidecar_filename() -> String {
    format!(
        "{SIDECAR_BASENAME}-{}",
        env!("FAIRTRANSLATOR_TARGET_TRIPLE")
    )
}

fn sidecar_relative_path() -> PathBuf {
    PathBuf::from("binaries").join(sidecar_filename())
}

#[cfg(not(test))]
fn resolve_sidecar_path(app_handle: Option<&RuntimeAppHandle>) -> Option<PathBuf> {
    let sidecar_name = sidecar_filename();
    let relative_path = sidecar_relative_path();
    let mut candidates = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative_path),
    ];

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join(&sidecar_name));
            candidates.push(parent.join("binaries").join(&sidecar_name));
        }
    }

    if let Some(app_handle) = app_handle {
        if let Ok(resource_path) = app_handle
            .path()
            .resolve(&relative_path, BaseDirectory::Resource)
        {
            candidates.push(resource_path);
        }
        if let Ok(resource_path) = app_handle
            .path()
            .resolve(&sidecar_name, BaseDirectory::Resource)
        {
            candidates.push(resource_path);
        }
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(test)]
fn resolve_sidecar_path(_app_handle: Option<&RuntimeAppHandle>) -> Option<PathBuf> {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(sidecar_relative_path());
    candidate.is_file().then_some(candidate)
}

#[cfg(windows)]
fn windows_pathexts() -> Vec<String> {
    env::var("PATHEXT")
        .ok()
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|ext| !ext.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|exts| !exts.is_empty())
        .unwrap_or_else(|| {
            vec![
                ".EXE".to_string(),
                ".CMD".to_string(),
                ".BAT".to_string(),
                ".COM".to_string(),
            ]
        })
}

#[cfg(windows)]
fn executable_candidates(dir: &Path, command_name: &str, pathexts: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let base = dir.join(command_name);
    candidates.push(base.clone());

    if base.extension().is_none() {
        for ext in pathexts {
            candidates.push(dir.join(format!("{command_name}{ext}")));
        }
    }

    candidates
}

#[cfg(not(windows))]
fn executable_candidates(dir: &Path, command_name: &str) -> Vec<PathBuf> {
    vec![dir.join(command_name)]
}

fn parse_exec_response(
    raw_response: &str,
    requests: &[TranslationRequest],
) -> Result<Vec<TranslationResponse>, AppError> {
    let normalized = strip_markdown_fences(raw_response);
    let parsed: CodexExecResponse = serde_json::from_str(normalized)
        .map_err(|error| AppError::Api(format!("Failed to parse Codex response: {error}")))?;

    if parsed.translations.len() != requests.len() {
        return Err(AppError::Api(format!(
            "Codex returned {} translations but expected {}",
            parsed.translations.len(),
            requests.len()
        )));
    }

    let expected_ids = requests
        .iter()
        .map(|request| request.entry_id.as_str())
        .collect::<HashSet<_>>();
    let mut seen_ids = HashSet::new();
    let mut by_id = HashMap::with_capacity(parsed.translations.len());

    for translation in parsed.translations {
        if !expected_ids.contains(translation.entry_id.as_str()) {
            return Err(AppError::Api(format!(
                "Codex returned an unexpected entry ID: {}",
                translation.entry_id
            )));
        }

        if !seen_ids.insert(translation.entry_id.clone()) {
            return Err(AppError::Api(format!(
                "Codex returned a duplicate entry ID: {}",
                translation.entry_id
            )));
        }

        by_id.insert(translation.entry_id, translation.translated_text);
    }

    requests
        .iter()
        .map(|request| {
            let translated_text = by_id.remove(&request.entry_id).ok_or_else(|| {
                AppError::Api(format!(
                    "Codex omitted a translation for entry ID {}",
                    request.entry_id
                ))
            })?;
            Ok(TranslationResponse {
                entry_id: request.entry_id.clone(),
                translated_text,
            })
        })
        .collect()
}

#[async_trait::async_trait]
impl TranslationProvider for CodexProvider {
    fn name(&self) -> &'static str {
        "Codex"
    }

    async fn validate(&self) -> Result<(), AppError> {
        let status = self.runtime_status().await?;
        if !status.is_installed || !status.is_authenticated {
            return Err(AppError::Api(status.message));
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
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let runtime = self
            .resolve_runtime()
            .await
            .map_err(|failure| AppError::Api(failure.message))?;

        let prompt = Self::build_prompt(source_lang, target_lang, requests, glossary)?;
        let temp_dir = temp_codex_dir();
        std::fs::create_dir_all(&temp_dir)?;

        let schema_path = temp_dir.join("schema.json");
        let output_path = temp_dir.join("result.json");
        std::fs::write(
            &schema_path,
            serde_json::to_vec_pretty(&Self::response_schema())?,
        )?;

        let output = self
            .run_exec(
                &runtime.candidate,
                prompt,
                schema_path.clone(),
                output_path.clone(),
                temp_dir.clone(),
            )
            .await?;

        if !output.status.success() {
            let _ = std::fs::remove_file(schema_path);
            let _ = std::fs::remove_file(output_path);
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(command_failure(&output, "Codex translation command failed"));
        }

        let _ = std::fs::remove_file(schema_path);
        let raw_response = std::fs::read_to_string(&output_path)?;
        let _ = std::fs::remove_file(output_path);
        let _ = std::fs::remove_dir_all(&temp_dir);

        parse_exec_response(&raw_response, requests)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_command_path_and_model_normalization() {
        let provider = CodexProvider::new(
            Some("   ".to_string()),
            Some("  gpt-5.4  ".to_string()),
            Some("  codex-default  ".to_string()),
        );
        assert!(provider.configured_command_path.is_none());
        assert_eq!(provider.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(provider.profile.as_deref(), Some("codex-default"));
    }

    #[test]
    fn test_build_prompt_includes_glossary_and_entry_ids() {
        let prompt = CodexProvider::build_prompt(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".to_string(),
                text: "勇者".to_string(),
            }],
            &[GlossaryTerm {
                source: "勇者".to_string(),
                target: "Heroe".to_string(),
                case_sensitive: true,
            }],
        )
        .unwrap();

        assert!(prompt.contains("entry-1"));
        assert!(prompt.contains("\"勇者\" -> \"Heroe\""));
        assert!(prompt.contains("Required output shape"));
    }

    #[test]
    fn test_parse_exec_response_rejects_missing_entries() {
        let error = parse_exec_response(
            r#"{"translations":[{"entryId":"entry-1","translatedText":"Hola"}]}"#,
            &[
                TranslationRequest {
                    entry_id: "entry-1".to_string(),
                    text: "Hello".to_string(),
                },
                TranslationRequest {
                    entry_id: "entry-2".to_string(),
                    text: "World".to_string(),
                },
            ],
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("returned 1 translations but expected 2"));
    }

    #[test]
    fn test_parse_exec_response_preserves_request_order() {
        let parsed = parse_exec_response(
            r#"{
              "translations": [
                {"entryId":"entry-2","translatedText":"Mundo"},
                {"entryId":"entry-1","translatedText":"Hola"}
              ]
            }"#,
            &[
                TranslationRequest {
                    entry_id: "entry-1".to_string(),
                    text: "Hello".to_string(),
                },
                TranslationRequest {
                    entry_id: "entry-2".to_string(),
                    text: "World".to_string(),
                },
            ],
        )
        .unwrap();

        assert_eq!(parsed[0].entry_id, "entry-1");
        assert_eq!(parsed[0].translated_text, "Hola");
        assert_eq!(parsed[1].entry_id, "entry-2");
        assert_eq!(parsed[1].translated_text, "Mundo");
    }

    #[test]
    fn test_parse_exec_response_accepts_wrapped_json() {
        let parsed = parse_exec_response(
            "```json\n{\"translations\":[{\"entryId\":\"entry-1\",\"translatedText\":\"Hola\"}]}\n```",
            &[TranslationRequest {
                entry_id: "entry-1".to_string(),
                text: "Hello".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].translated_text, "Hola");
    }

    #[test]
    fn test_login_requires_auth_detects_expected_prompts() {
        assert!(login_requires_auth("Not logged in. Run `codex login` first."));
        assert!(login_requires_auth("Authentication required"));
        assert!(!login_requires_auth("Logged in using ChatGPT"));
    }

    #[test]
    fn test_status_from_failure_marks_not_installed() {
        let status = status_from_failure(RuntimeResolutionFailure {
            candidate: RuntimeCandidate {
                source: RuntimeSource::Path,
                effective_command_path: DEFAULT_COMMAND_PATH.to_string(),
                resolved_command_path: None,
            },
            state: RuntimeFailureState::NotInstalled,
            message: "Codex CLI was not found on PATH".to_string(),
            version: None,
        });

        assert_eq!(status.state, "notInstalled");
        assert_eq!(status.command_source, "path");
        assert!(!status.is_installed);
    }
}

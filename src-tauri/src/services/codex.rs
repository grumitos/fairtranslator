use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use tauri::path::BaseDirectory;
#[cfg(not(test))]
use tauri::{AppHandle, Manager};
use tokio::task;

use crate::error::AppError;
use crate::models::GlossaryTerm;
use crate::services::ai_models::ProviderModel;
use crate::services::compact_prompt::{self, CompactPromptBundle};
use crate::services::provider::{
    approximate_token_count, TranslationBatchContext, TranslationProvider, TranslationRequest,
    TranslationResponse,
};

const DEFAULT_COMMAND_PATH: &str = "codex";
const SIDECAR_BASENAME: &str = "codex";
const CODEX_RUNTIME_WRAPPER_TOKEN_OVERHEAD: usize = 160;
const CODEX_RESPONSE_HEADROOM_TOKENS: usize = 2048;
const CODEX_STATUS_TIMEOUT: Duration = Duration::from_secs(30);
const CODEX_EXEC_TIMEOUT: Duration = Duration::from_secs(300);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
    reasoning_effort: Option<String>,
}

impl CodexProvider {
    pub fn new(
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
        reasoning_effort: Option<String>,
    ) -> Self {
        Self::from_parts(None, command_path, model, profile, reasoning_effort)
    }

    pub fn with_app_handle(
        app_handle: RuntimeAppHandle,
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
        reasoning_effort: Option<String>,
    ) -> Self {
        Self::from_parts(
            Some(app_handle),
            command_path,
            model,
            profile,
            reasoning_effort,
        )
    }

    fn from_parts(
        app_handle: Option<RuntimeAppHandle>,
        command_path: Option<String>,
        model: Option<String>,
        profile: Option<String>,
        reasoning_effort: Option<String>,
    ) -> Self {
        Self {
            app_handle,
            configured_command_path: command_path.and_then(normalize_optional),
            model: model.and_then(normalize_optional),
            profile: profile.and_then(normalize_optional),
            reasoning_effort: reasoning_effort.and_then(normalize_optional),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn build_prompt(
        source_lang: &str,
        target_lang: &str,
        requests: &[TranslationRequest],
        glossary: &[GlossaryTerm],
        context: &TranslationBatchContext,
    ) -> Result<String, AppError> {
        Ok(compact_prompt::build_provider_bundle(
            source_lang,
            target_lang,
            requests,
            glossary,
            context,
        )?
        .render_single_prompt())
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
        .expect("serializing compact Codex prompt should not fail");
        let prompt = bundle.render_single_prompt();
        let schema = compact_prompt::provider_response_schema().to_string();

        approximate_token_count(&prompt)
            + approximate_token_count(&schema)
            + CODEX_RUNTIME_WRAPPER_TOKEN_OVERHEAD
            + CODEX_RESPONSE_HEADROOM_TOKENS
    }

    fn response_schema() -> serde_json::Value {
        compact_prompt::provider_response_schema()
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
                Some(CODEX_STATUS_TIMEOUT),
            )
            .await?;
        let combined_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&login_output.stdout),
            String::from_utf8_lossy(&login_output.stderr)
        );
        let is_authenticated =
            login_is_authenticated(&combined_output, login_output.status.success());

        let (state, message) = if is_authenticated {
            (
                "ready".to_string(),
                if runtime.candidate.source == RuntimeSource::Sidecar {
                    "Bundled Codex runtime is installed and authenticated".to_string()
                } else {
                    "Codex CLI is installed and authenticated".to_string()
                },
            )
        } else if login_requires_auth(&combined_output) {
            (
                "loginRequired".to_string(),
                "Codex CLI is installed but not authenticated. Run `codex login` first".to_string(),
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

    pub async fn login(&self, device_auth: bool) -> Result<CodexRuntimeStatus, AppError> {
        let runtime = self
            .resolve_runtime()
            .await
            .map_err(|failure| AppError::Api(failure.message))?;

        let mut args = vec!["login".to_string()];
        if device_auth {
            args.push("--device-auth".to_string());
        }

        let output = self.run_login_command(&runtime.candidate, &args).await?;

        if !output.status.success() {
            return Err(command_failure(&output, "Codex login failed"));
        }

        self.runtime_status().await
    }

    pub async fn list_models(&self) -> Result<Vec<ProviderModel>, AppError> {
        let runtime = self
            .resolve_runtime()
            .await
            .map_err(|failure| AppError::Api(failure.message))?;
        let output = self
            .run_command(
                &runtime.candidate,
                vec!["models".to_string()],
                None,
                None,
                Some(CODEX_STATUS_TIMEOUT),
            )
            .await?;

        let mut models = Vec::new();
        if output.status.success() {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            for line in combined
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                for token in line.split_whitespace() {
                    let token = token.trim_matches(|ch: char| {
                        matches!(ch, ',' | ';' | '*' | '[' | ']' | '(' | ')' | '"' | '\'')
                    });
                    if looks_like_model_id(token)
                        && !models.iter().any(|model: &ProviderModel| model.id == token)
                    {
                        models.push(ProviderModel {
                            id: token.to_string(),
                            owned_by: Some("codex-cli".to_string()),
                            created: None,
                        });
                    }
                }
            }
        }

        if let Some(configured_model) = &self.model {
            if !models.iter().any(|model| model.id == *configured_model) {
                models.insert(
                    0,
                    ProviderModel {
                        id: configured_model.clone(),
                        owned_by: Some("configured".to_string()),
                        created: None,
                    },
                );
            }
        }

        Ok(models)
    }

    async fn run_login_command(
        &self,
        runtime: &RuntimeCandidate,
        args: &[String],
    ) -> Result<std::process::Output, AppError> {
        #[cfg(windows)]
        {
            return self
                .run_login_command_in_windows_terminal(runtime, args)
                .await;
        }

        #[cfg(not(windows))]
        {
            self.run_command(runtime, args.to_vec(), None, None, None)
                .await
        }
    }

    #[cfg(windows)]
    async fn run_login_command_in_windows_terminal(
        &self,
        runtime: &RuntimeCandidate,
        args: &[String],
    ) -> Result<std::process::Output, AppError> {
        let executable = runtime.effective_command_path.clone();
        let args = args.to_vec();

        task::spawn_blocking(move || {
            let mut command = Command::new("cmd");
            command.arg("/C").arg("start").arg("").arg("/WAIT");
            command.arg(executable);
            command.args(args);
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
            command.output().map_err(AppError::from)
        })
        .await
        .map_err(|error| AppError::Project(format!("Failed to join Codex login task: {error}")))?
    }

    async fn resolve_runtime(&self) -> Result<ResolvedRuntime, RuntimeResolutionFailure> {
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

        if let Some(sidecar_candidate) = self.sidecar_candidate() {
            if let Ok(runtime) = self.probe_runtime(sidecar_candidate).await {
                return Ok(runtime);
            }
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
                Some(CODEX_STATUS_TIMEOUT),
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
        if let Some(reasoning_effort) = &self.reasoning_effort {
            args.push("-c".to_string());
            args.push(reasoning_effort_config_arg(reasoning_effort));
        }
        args.push("--".to_string());
        args.push("-".to_string());
        self.run_command(
            runtime,
            args,
            Some(&workdir),
            Some(prompt),
            Some(CODEX_EXEC_TIMEOUT),
        )
        .await
    }

    async fn run_command(
        &self,
        runtime: &RuntimeCandidate,
        args: Vec<String>,
        current_dir: Option<&Path>,
        stdin_text: Option<String>,
        timeout: Option<Duration>,
    ) -> Result<std::process::Output, AppError> {
        let current_dir = current_dir.map(Path::to_path_buf);
        let mut command = self.build_command(runtime, &args, current_dir.as_deref())?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let (finished_tx, finished_rx) = mpsc::channel();

        let task = task::spawn_blocking(move || {
            let _completion = CommandCompletion(finished_tx);
            if stdin_text.is_some() {
                command.stdin(Stdio::piped());
            }
            command.stdout(Stdio::piped()).stderr(Stdio::piped());

            let child = command.spawn().map_err(AppError::from)?;
            wait_with_optional_timeout(child, stdin_text, timeout, &worker_cancelled)
        });
        let mut cancellation = CommandCancellationGuard::new(cancelled, finished_rx);
        let result = task
            .await
            .map_err(|error| AppError::Project(format!("Failed to join Codex task: {error}")))?;
        cancellation.disarm();
        result
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

struct CommandCompletion(mpsc::Sender<()>);

impl Drop for CommandCompletion {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

struct CommandCancellationGuard {
    cancelled: Arc<AtomicBool>,
    finished: Option<mpsc::Receiver<()>>,
}

impl CommandCancellationGuard {
    fn new(cancelled: Arc<AtomicBool>, finished: mpsc::Receiver<()>) -> Self {
        Self {
            cancelled,
            finished: Some(finished),
        }
    }

    fn disarm(&mut self) {
        self.finished = None;
    }
}

impl Drop for CommandCancellationGuard {
    fn drop(&mut self) {
        let Some(finished) = self.finished.take() else {
            return;
        };
        self.cancelled.store(true, Ordering::Release);
        // Dropping the provider future must not release the batch run while its process still runs.
        let _ = finished.recv();
    }
}

enum ChildStop {
    Completed(ExitStatus),
    Cancelled(ExitStatus),
    TimedOut(ExitStatus),
}

fn wait_with_optional_timeout(
    mut child: Child,
    stdin_text: Option<String>,
    timeout: Option<Duration>,
    cancelled: &AtomicBool,
) -> Result<std::process::Output, AppError> {
    let stdin_writer = child.stdin.take().map(|mut stdin| {
        thread::spawn(move || {
            if let Some(stdin_text) = stdin_text {
                stdin.write_all(stdin_text.as_bytes())?;
            }
            Ok::<(), std::io::Error>(())
        })
    });
    let stdout_reader = child.stdout.take().map(|mut stdout| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes)?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        })
    });
    let stderr_reader = child.stderr.take().map(|mut stderr| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes)?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        })
    });

    let started_at = Instant::now();
    let stop = loop {
        if let Some(status) = child.try_wait().map_err(AppError::from)? {
            break ChildStop::Completed(status);
        }
        if cancelled.load(Ordering::Acquire) {
            break ChildStop::Cancelled(terminate_child_tree(&mut child)?);
        }
        if timeout.is_some_and(|timeout| started_at.elapsed() >= timeout) {
            break ChildStop::TimedOut(terminate_child_tree(&mut child)?);
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    };

    let _stdin_result = join_io_thread(stdin_writer, "stdin")?;
    let stdout = join_io_thread(stdout_reader, "stdout")?.unwrap_or_default();
    let stderr = join_io_thread(stderr_reader, "stderr")?.unwrap_or_default();
    let status = match stop {
        ChildStop::Completed(status)
        | ChildStop::Cancelled(status)
        | ChildStop::TimedOut(status) => status,
    };
    let output = Output {
        status,
        stdout,
        stderr,
    };

    match stop {
        ChildStop::Completed(_) => Ok(output),
        ChildStop::Cancelled(_) => Err(AppError::Api("Codex command cancelled".to_string())),
        ChildStop::TimedOut(_) => {
            let timeout = timeout.expect("timed out command must have a timeout");
            Err(AppError::Api(format!(
                "Codex command timed out after {} seconds: {}",
                timeout.as_secs(),
                command_output_detail(&output)
            )))
        }
    }
}

fn join_io_thread<T>(
    handle: Option<thread::JoinHandle<std::io::Result<T>>>,
    stream: &str,
) -> Result<Option<T>, AppError> {
    let Some(handle) = handle else {
        return Ok(None);
    };
    handle
        .join()
        .map_err(|_| AppError::Project(format!("Codex {stream} worker panicked")))?
        .map(Some)
        .map_err(AppError::from)
}

fn terminate_child_tree(child: &mut Child) -> Result<ExitStatus, AppError> {
    #[cfg(windows)]
    {
        let taskkill = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !taskkill.is_ok_and(|status| status.success()) {
            let _ = child.kill();
        }
    }

    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }

    child.wait().map_err(AppError::from)
}

fn normalize_optional(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

struct CodexTempDir(PathBuf);

impl CodexTempDir {
    fn create() -> Result<Self, AppError> {
        let path =
            std::env::temp_dir().join(format!("fairtranslator-codex-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for CodexTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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

fn login_is_authenticated(output: &str, command_succeeded: bool) -> bool {
    !login_requires_auth(output)
        && (command_succeeded || output.to_ascii_lowercase().contains("logged in"))
}

fn looks_like_model_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':'))
        && (value.contains('-') || value.contains('/') || value.starts_with("gpt"))
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "models" | "model" | "provider" | "available"
        )
}

fn reasoning_effort_config_arg(reasoning_effort: &str) -> String {
    format!(
        "model_reasoning_effort={}",
        serde_json::to_string(reasoning_effort).unwrap_or_else(|_| "\"low\"".to_string())
    )
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
    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&relative_path)];

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
    bundle: &CompactPromptBundle,
) -> Result<Vec<TranslationResponse>, AppError> {
    compact_prompt::parse_provider_response(raw_response, &bundle.entry_refs)
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
        context: &TranslationBatchContext,
    ) -> Result<Vec<TranslationResponse>, AppError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let runtime = self
            .resolve_runtime()
            .await
            .map_err(|failure| AppError::Api(failure.message))?;

        let bundle = compact_prompt::build_provider_bundle(
            source_lang,
            target_lang,
            requests,
            glossary,
            context,
        )?;
        let prompt = bundle.render_single_prompt();
        let temp_dir = CodexTempDir::create()?;

        let result = async {
            let schema_path = temp_dir.path().join("schema.json");
            let output_path = temp_dir.path().join("result.json");
            std::fs::write(
                &schema_path,
                serde_json::to_vec_pretty(
                    &bundle
                        .response_schema
                        .clone()
                        .unwrap_or_else(Self::response_schema),
                )?,
            )?;

            let output = self
                .run_exec(
                    &runtime.candidate,
                    prompt,
                    schema_path,
                    output_path.clone(),
                    temp_dir.path().to_path_buf(),
                )
                .await?;

            if !output.status.success() {
                return Err(command_failure(&output, "Codex translation command failed"));
            }

            let raw_response = std::fs::read_to_string(output_path)?;
            parse_exec_response(&raw_response, &bundle)
        }
        .await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn process_is_running(pid: u32) -> bool {
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}"
                ),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(windows)]
    fn kill_test_process(pid: u32) {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cancelling_codex_kills_waits_cleans_up_and_allows_the_next_run() {
        use crate::services::batch_lifecycle::{BatchRunControl, BatchRunLifecycle};
        use std::sync::Arc;

        let root = std::env::temp_dir().join(format!(
            "fairtranslator-codex-cancel-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let ready_path = root.join("ready.txt");
        let temp_path_marker = root.join("temp-path.txt");
        let success_path = root.join("succeed");
        let script_path = root.join("codex-helper.cmd");
        let ps_ready_path = ready_path.to_string_lossy().replace('\'', "''");
        std::fs::write(
            &script_path,
            format!(
                r#"@echo off
if "%~1"=="--version" (
  echo codex-test 1.0
  exit /b 0
)
if "%~1"=="login" (
  echo Logged in using test helper
  exit /b 0
)
if not "%~1"=="exec" exit /b 1
set "OUT="
:args
if "%~1"=="" goto run
if "%~1"=="--output-last-message" (
  set "OUT=%~2"
  shift
)
shift
goto args
:run
if exist "{success_path}" (
  >"%OUT%" echo {{"translations":[{{"k":1,"t":"Hola"}}]}}
  exit /b 0
)
>"{temp_path_marker}" echo %CD%
powershell.exe -NoProfile -NonInteractive -Command "$stream = [IO.File]::Open((Join-Path (Get-Location) 'lock'), [IO.FileMode]::Create, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None); Set-Content -LiteralPath '{ps_ready_path}' -Value $PID; while ($true) {{ Start-Sleep -Milliseconds 100 }}"
"#,
                success_path = success_path.display(),
                temp_path_marker = temp_path_marker.display(),
            ),
        )
        .unwrap();

        let provider = CodexProvider::new(
            Some(script_path.to_string_lossy().into_owned()),
            None,
            None,
            None,
        );
        let requests = [TranslationRequest {
            entry_id: "entry-1".to_string(),
            file_context: String::new(),
            text: "Hello".to_string(),
            notes: String::new(),
        }];
        let context = TranslationBatchContext::default();
        let control = Arc::new(BatchRunControl::new());
        let first_guard = control.try_acquire().unwrap();
        let first_run_id = first_guard.run_id();
        let cancel_control = Arc::clone(&control);
        let cancel_ready_path = ready_path.clone();
        let cancel_task = tokio::spawn(async move {
            for _ in 0..200 {
                if cancel_ready_path.is_file() {
                    assert!(cancel_control.cancel(first_run_id));
                    return;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            panic!("controlled Codex helper did not start");
        });
        let mut first_lifecycle =
            BatchRunLifecycle::new(&control, first_run_id, &["Map001.json".to_string()]);
        assert!(first_lifecycle
            .wait_or_cancel(provider.translate_batch(&requests, "en", "es", &[], &context))
            .await
            .is_none());
        cancel_task.await.unwrap();
        drop(first_lifecycle);
        drop(first_guard);

        let helper_pid: u32 = std::fs::read_to_string(&ready_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let first_temp_dir =
            PathBuf::from(std::fs::read_to_string(&temp_path_marker).unwrap().trim());
        let helper_survived_cancel = process_is_running(helper_pid);
        let temp_survived_cancel = first_temp_dir.exists();

        std::fs::write(&success_path, "").unwrap();
        let second_guard = control.try_acquire().unwrap();
        let mut second_lifecycle = BatchRunLifecycle::new(
            &control,
            second_guard.run_id(),
            &["Map001.json".to_string()],
        );
        let second = second_lifecycle
            .wait_or_cancel(provider.translate_batch(&requests, "en", "es", &[], &context))
            .await
            .expect("a stale cancellation must not target the next run")
            .unwrap();
        assert_eq!(second[0].translated_text, "Hola");

        if helper_survived_cancel {
            kill_test_process(helper_pid);
        }
        for _ in 0..20 {
            if !first_temp_dir.exists() && !process_is_running(helper_pid) {
                break;
            }
            let _ = std::fs::remove_dir_all(&first_temp_dir);
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = std::fs::remove_dir_all(&root);

        assert!(!helper_survived_cancel, "cancel left the helper running");
        assert!(!temp_survived_cancel, "cancel left Codex temp files behind");
    }

    #[test]
    fn test_default_command_path_and_model_normalization() {
        let provider = CodexProvider::new(
            Some("   ".to_string()),
            Some("  gpt-5.4  ".to_string()),
            Some("  codex-default  ".to_string()),
            None,
        );
        assert!(provider.configured_command_path.is_none());
        assert_eq!(provider.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(provider.profile.as_deref(), Some("codex-default"));
    }

    #[test]
    fn test_reasoning_effort_uses_current_codex_config_key() {
        assert_eq!(
            reasoning_effort_config_arg("low"),
            "model_reasoning_effort=\"low\""
        );
    }

    #[test]
    fn test_codex_command_timeouts_are_bounded() {
        assert_eq!(CODEX_STATUS_TIMEOUT.as_secs(), 30);
        assert_eq!(CODEX_EXEC_TIMEOUT.as_secs(), 300);
    }

    #[test]
    fn test_build_prompt_uses_compact_payload_and_masks_tokens() {
        let prompt = CodexProvider::build_prompt(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".to_string(),
                file_context: "Map001 > Event 1 > Show Text".to_string(),
                text: "\\N[1]\n勇者".to_string(),
                notes: String::new(),
            }],
            &[GlossaryTerm {
                source: "勇者".to_string(),
                target: "Heroe".to_string(),
                case_sensitive: true,
            }],
            &TranslationBatchContext::default(),
        )
        .unwrap();

        assert!(prompt.contains("\"gl\":[{\"s\":\"勇者\",\"t\":\"Heroe\"}]"));
        assert!(prompt.contains("\"k\":1"));
        assert!(prompt.contains("__K1_CC0__"));
        assert!(!prompt.contains("entry-1"));
    }

    #[test]
    fn test_parse_exec_response_rejects_missing_entries() {
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[
                TranslationRequest {
                    entry_id: "entry-1".to_string(),
                    file_context: String::new(),
                    text: "Hello".to_string(),
                    notes: String::new(),
                },
                TranslationRequest {
                    entry_id: "entry-2".to_string(),
                    file_context: String::new(),
                    text: "World".to_string(),
                    notes: String::new(),
                },
            ],
            &[],
            &TranslationBatchContext::default(),
        )
        .unwrap();
        let error =
            parse_exec_response(r#"{"translations":[{"k":1,"t":"Hola"}]}"#, &bundle).unwrap_err();

        assert!(error
            .to_string()
            .contains("devolvió 1 traducciones, pero se esperaban 2"));
    }

    #[test]
    fn test_parse_exec_response_preserves_request_order() {
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[
                TranslationRequest {
                    entry_id: "entry-1".to_string(),
                    file_context: String::new(),
                    text: "Hello".to_string(),
                    notes: String::new(),
                },
                TranslationRequest {
                    entry_id: "entry-2".to_string(),
                    file_context: String::new(),
                    text: "World".to_string(),
                    notes: String::new(),
                },
            ],
            &[],
            &TranslationBatchContext::default(),
        )
        .unwrap();
        let parsed = parse_exec_response(
            r#"{"translations":[{"k":2,"t":"Mundo"},{"k":1,"t":"Hola"}]}"#,
            &bundle,
        )
        .unwrap();

        assert_eq!(parsed[0].entry_id, "entry-1");
        assert_eq!(parsed[0].translated_text, "Hola");
        assert_eq!(parsed[1].entry_id, "entry-2");
        assert_eq!(parsed[1].translated_text, "Mundo");
    }

    #[test]
    fn test_parse_exec_response_accepts_wrapped_json() {
        let bundle = compact_prompt::build_provider_bundle(
            "ja",
            "es",
            &[TranslationRequest {
                entry_id: "entry-1".to_string(),
                file_context: String::new(),
                text: "\\N[1]\nHello".to_string(),
                notes: String::new(),
            }],
            &[],
            &TranslationBatchContext::default(),
        )
        .unwrap();
        let parsed = parse_exec_response(
            "```json\n{\"translations\":[{\"k\":1,\"t\":\"__K1_CC0__\\\\nHola\"}]}\n```",
            &bundle,
        )
        .unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].translated_text, "\\N[1]\nHola");
    }

    #[test]
    fn test_login_requires_auth_detects_expected_prompts() {
        assert!(login_requires_auth(
            "Not logged in. Run `codex login` first."
        ));
        assert!(login_requires_auth("Authentication required"));
        assert!(!login_requires_auth("Logged in using ChatGPT"));
    }

    #[test]
    fn test_not_logged_in_is_never_authenticated() {
        assert!(!login_is_authenticated(
            "Not logged in. Run `codex login` first.",
            true,
        ));
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

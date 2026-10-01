//! Keeps the external CLIs FairTranslator drives up to date (setting
//! `autoUpdateClis`, on by default). It runs in the background at startup and
//! never delays the window; a translation run that needs a CLI waits only
//! while that CLI is actually being updated. Offline or failed updates never
//! block anything: the installed version keeps working.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::error::AppError;

/// How long the latest-version lookup may take before counting as offline.
pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for the CLI's own updater.
pub const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);
/// Longest a run waits for an update in progress before giving up.
pub const RUN_WAIT_TIMEOUT: Duration = Duration::from_secs(360);
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);
/// How long to keep reading output after the command exits.
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
pub const NPM_REGISTRY: &str = "https://registry.npmjs.org";

/// A CLI FairTranslator can update through the CLI's own updater.
#[derive(Debug, Clone, Copy)]
pub struct CliSpec {
    pub id: &'static str,
    /// npm package whose `latest` dist-tag is the newest release.
    pub npm_package: &'static str,
    pub update_args: &'static [&'static str],
    pub update_env: &'static [(&'static str, &'static str)],
}

pub const CODEX: CliSpec = CliSpec {
    id: "codex",
    npm_package: "@openai/codex",
    update_args: &["update"],
    update_env: &[("CODEX_NON_INTERACTIVE", "1")],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliUpdateState {
    Checking,
    UpToDate,
    Updating,
    Updated,
    Failed,
    Skipped,
    Offline,
}

/// Why a CLI was not checked (only with `CliUpdateState::Skipped`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliSkipReason {
    /// `autoUpdateClis` is off.
    Disabled,
    /// The runtime bundled with FairTranslator.
    Bundled,
    /// A custom path or another installer's copy.
    Custom,
    NotInstalled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliUpdateStatus {
    pub cli: String,
    pub state: CliUpdateState,
    pub skip_reason: Option<CliSkipReason>,
    /// Installed version when known.
    pub from_version: Option<String>,
    /// Latest version (target of an update) when known.
    pub to_version: Option<String>,
    pub message: String,
}

impl CliUpdateStatus {
    fn new(cli: &str, state: CliUpdateState, message: impl Into<String>) -> Self {
        Self {
            cli: cli.to_string(),
            state,
            skip_reason: None,
            from_version: None,
            to_version: None,
            message: message.into(),
        }
    }

    fn skipped(cli: &str, reason: CliSkipReason, message: &str) -> Self {
        Self {
            skip_reason: Some(reason),
            ..Self::new(cli, CliUpdateState::Skipped, message)
        }
    }

    fn versions(mut self, from: Option<&Version>, to: Option<&Version>) -> Self {
        self.from_version = from.map(Version::to_string);
        self.to_version = to.map(Version::to_string);
        self
    }
}

/// Which install of a CLI would run, and whether FairTranslator may update it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliInstall {
    /// The CLI's standard installer layout: safe to update with its updater.
    Standard(PathBuf),
    /// The runtime bundled with FairTranslator: updated with the app.
    Sidecar(PathBuf),
    /// A path the user chose outside the standard layout: left alone.
    Custom(PathBuf),
    Missing,
}

/// Standard layout of the Codex standalone installer: the shim in
/// `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin` and the payload under
/// `%USERPROFILE%\.codex\packages\standalone` (`current` is a junction to the
/// active release).
pub fn codex_standard_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(
            PathBuf::from(local)
                .join("Programs")
                .join("OpenAI")
                .join("Codex"),
        );
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        roots.push(
            PathBuf::from(profile)
                .join(".codex")
                .join("packages")
                .join("standalone"),
        );
    }
    roots
}

fn comparable(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Classifies the executable that would run, given where it came from.
pub fn classify_install(
    resolved: Option<&Path>,
    is_sidecar: bool,
    standard_roots: &[PathBuf],
) -> CliInstall {
    let Some(path) = resolved else {
        return CliInstall::Missing;
    };
    if is_sidecar {
        return CliInstall::Sidecar(path.to_path_buf());
    }
    let candidate = comparable(path);
    let standard = standard_roots
        .iter()
        .map(|root| comparable(root))
        .any(|root| candidate.starts_with(&root));
    if standard {
        CliInstall::Standard(path.to_path_buf())
    } else {
        CliInstall::Custom(path.to_path_buf())
    }
}

/// Current update status per CLI, observable by commands and waiting runs.
pub struct CliUpdates {
    statuses: Mutex<HashMap<String, CliUpdateStatus>>,
    changed: Notify,
}

impl Default for CliUpdates {
    fn default() -> Self {
        Self {
            statuses: Mutex::new(HashMap::new()),
            changed: Notify::new(),
        }
    }
}

static REGISTRY: LazyLock<CliUpdates> = LazyLock::new(CliUpdates::default);

/// The process-wide status registry used by the app.
pub fn registry() -> &'static CliUpdates {
    &REGISTRY
}

impl CliUpdates {
    pub fn set(&self, status: CliUpdateStatus) {
        if let Ok(mut statuses) = self.statuses.lock() {
            statuses.insert(status.cli.clone(), status);
        }
        self.changed.notify_waiters();
    }

    pub fn snapshot(&self) -> Vec<CliUpdateStatus> {
        let mut statuses = self
            .statuses
            .lock()
            .map(|statuses| statuses.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        statuses.sort_by(|left, right| left.cli.cmp(&right.cli));
        statuses
    }

    /// Marks `cli` as being checked. The app calls this before spawning the
    /// background check, so no run can start in the gap before it begins.
    pub fn mark_checking(&self, cli: &str) {
        self.set(CliUpdateStatus::new(
            cli,
            CliUpdateState::Checking,
            "Checking for updates",
        ));
    }

    /// Records that automatic updates are off for `cli`.
    pub fn mark_disabled(&self, cli: &str) {
        self.set(CliUpdateStatus::skipped(
            cli,
            CliSkipReason::Disabled,
            "Automatic updates are off",
        ));
    }

    /// Checking counts as busy because an update only ever starts from it.
    fn is_busy(&self, cli: &str) -> bool {
        self.statuses
            .lock()
            .map(|statuses| {
                statuses.get(cli).is_some_and(|status| {
                    matches!(
                        status.state,
                        CliUpdateState::Checking | CliUpdateState::Updating
                    )
                })
            })
            .unwrap_or(false)
    }

    /// Waits while `cli` is being checked or updated so a run never spawns it
    /// mid-update. Returns at once otherwise (including offline/failed).
    pub async fn wait_until_idle(&self, cli: &str, timeout: Duration) -> Result<(), AppError> {
        let deadline = Instant::now() + timeout;
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.is_busy(cli) {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AppError::Api(format!(
                    "La CLI de {cli} se sigue actualizando; vuelve a intentarlo en un momento."
                )));
            }
            let _ = tokio::time::timeout(remaining, notified).await;
        }
    }
}

/// Parsed `major.minor.patch[-pre]` version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    parts: [u64; 3],
    pre: Option<String>,
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [major, minor, patch] = self.parts;
        write!(formatter, "{major}.{minor}.{patch}")?;
        if let Some(pre) = &self.pre {
            write!(formatter, "-{pre}")?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.parts
            .cmp(&other.parts)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(left), Some(right)) => left.cmp(right),
            })
    }
}

/// First `x.y.z[-pre]` token in text such as `codex-cli 0.159.2`.
pub fn parse_version(text: &str) -> Option<Version> {
    text.split(|ch: char| ch.is_whitespace() || ch == '"' || ch == ',')
        .map(|token| token.trim_start_matches('v'))
        .find_map(|token| {
            let (core, pre) = match token.split_once('-') {
                Some((core, pre)) => (core, Some(pre.to_string())),
                None => (token, None),
            };
            let numbers = core
                .split('.')
                .map(|part| part.parse::<u64>().ok())
                .collect::<Option<Vec<_>>>()?;
            let [major, minor, patch] = numbers.as_slice() else {
                return None;
            };
            Some(Version {
                parts: [*major, *minor, *patch],
                pre,
            })
        })
}

/// Runs a command with a hard timeout, reading its pipes on helper threads.
/// A timed-out command is killed with its whole process tree. Output reading
/// is bounded too: a background process the command leaves behind (Codex can
/// start a shared daemon) inherits the pipes and would otherwise keep them
/// open long after the command itself exits.
fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            let _ = sender.send(bytes);
        });
        receiver
    };
    let stdout = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Ok(None) => {
                kill_tree(child.id());
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {} s", timeout.as_secs()));
            }
            Err(error) => return Err(error.to_string()),
        }
    };
    let collect = |receiver: std::sync::mpsc::Receiver<Vec<u8>>| {
        receiver
            .recv_timeout(OUTPUT_DRAIN_TIMEOUT)
            .unwrap_or_default()
    };
    Ok(Output {
        status,
        stdout: collect(stdout),
        stderr: collect(stderr),
    })
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new(system32().join("taskkill.exe"))
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    let _ = pid;
}

#[cfg(windows)]
fn system32() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
        .join("System32")
}

/// Machine-level `PSModulePath` (registry value, variables expanded). A child
/// of PowerShell 7 inherits PS7 module paths, which break the Windows
/// PowerShell 5.1 installers some updaters launch (e.g. `Get-FileHash` not
/// found).
#[cfg(windows)]
fn machine_ps_module_path() -> String {
    let fallback = || {
        let program_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        format!(
            "{program_files}\\WindowsPowerShell\\Modules;{system_root}\\system32\\WindowsPowerShell\\v1.0\\Modules"
        )
    };
    let output = Command::new(system32().join("reg.exe"))
        .args([
            "query",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
            "/v",
            "PSModulePath",
        ])
        .stdin(Stdio::null())
        .output();
    let Ok(output) = output else {
        return fallback();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(raw) = text
        .lines()
        .find(|line| line.trim_start().starts_with("PSModulePath"))
        .and_then(|line| line.split("REG_").nth(1))
        .and_then(|rest| {
            rest.split_once(char::is_whitespace)
                .map(|(_, value)| value.trim())
        })
    else {
        return fallback();
    };
    expand_environment(raw)
}

#[cfg(windows)]
fn expand_environment(value: &str) -> String {
    let mut output = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                output.push_str(&std::env::var(name).unwrap_or_else(|_| format!("%{name}%")));
                rest = &after[end + 1..];
            }
            None => {
                output.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    output.push_str(rest);
    output
}

/// The updater child: non-interactive, System32 first in `PATH` (GNU tar from
/// Git for Windows fails on drive-letter paths: "Cannot connect to C:"), and
/// the machine-level `PSModulePath`.
fn update_command(executable: &Path, spec: &CliSpec) -> Command {
    let mut command = Command::new(executable);
    command.args(spec.update_args);
    for (key, value) in spec.update_env {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        let mut paths = vec![system32()];
        if let Some(current) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&current));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env("PATH", joined);
        }
        command.env("PSModulePath", machine_ps_module_path());
    }
    command
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(work).await.ok()
}

async fn installed_version(executable: &Path) -> Option<Version> {
    let executable = executable.to_path_buf();
    let output = blocking(move || {
        let mut command = Command::new(&executable);
        command.arg("--version");
        run_with_timeout(command, VERSION_TIMEOUT)
    })
    .await?
    .ok()?;
    parse_version(&String::from_utf8_lossy(&output.stdout))
        .or_else(|| parse_version(&String::from_utf8_lossy(&output.stderr)))
}

/// `latest` dist-tag of an npm package; `None` when offline or unreadable.
async fn latest_version(registry_base: &str, package: &str, timeout: Duration) -> Option<Version> {
    let client = reqwest::Client::builder().timeout(timeout).build().ok()?;
    let url = format!("{}/{}/latest", registry_base.trim_end_matches('/'), package);
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let body = response.json::<serde_json::Value>().await.ok()?;
    parse_version(body.get("version")?.as_str()?)
}

/// Last non-empty line of the updater output, shortened for the status line.
fn short_detail(output: &Output) -> String {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let line = text
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no output");
    line.chars().take(160).collect()
}

/// Checks one CLI and updates it through its own updater when the registry
/// has a newer release. Every state change is stored in `registry` and passed
/// to `emit` (the app forwards it as the `cli-update:status` event).
pub async fn check_and_update(
    spec: &CliSpec,
    install: CliInstall,
    registry_base: &str,
    lookup_timeout: Duration,
    update_timeout: Duration,
    registry: &CliUpdates,
    emit: &(dyn Fn(&CliUpdateStatus) + Send + Sync),
) -> CliUpdateStatus {
    let publish = |status: CliUpdateStatus| {
        registry.set(status.clone());
        emit(&status);
        log::info!(
            "CLI update {}: {:?} ({})",
            status.cli,
            status.state,
            status.message
        );
        status
    };

    let executable = match install {
        CliInstall::Standard(path) => path,
        CliInstall::Sidecar(_) => {
            return publish(CliUpdateStatus::skipped(
                spec.id,
                CliSkipReason::Bundled,
                "Bundled runtime; it updates with FairTranslator",
            ))
        }
        CliInstall::Custom(_) => {
            return publish(CliUpdateStatus::skipped(
                spec.id,
                CliSkipReason::Custom,
                "Not the standard install (custom path or another installer); left as is",
            ))
        }
        CliInstall::Missing => {
            return publish(CliUpdateStatus::skipped(
                spec.id,
                CliSkipReason::NotInstalled,
                "Not installed",
            ))
        }
    };

    publish(CliUpdateStatus::new(
        spec.id,
        CliUpdateState::Checking,
        "Checking for updates",
    ));
    let Some(installed) = installed_version(&executable).await else {
        return publish(CliUpdateStatus::new(
            spec.id,
            CliUpdateState::Failed,
            "Could not read the installed version",
        ));
    };
    let Some(latest) = latest_version(registry_base, spec.npm_package, lookup_timeout).await else {
        return publish(
            CliUpdateStatus::new(
                spec.id,
                CliUpdateState::Offline,
                "Could not reach the update server; using the installed version",
            )
            .versions(Some(&installed), None),
        );
    };
    if latest <= installed {
        return publish(
            CliUpdateStatus::new(spec.id, CliUpdateState::UpToDate, "Up to date")
                .versions(Some(&installed), Some(&latest)),
        );
    }

    publish(
        CliUpdateStatus::new(spec.id, CliUpdateState::Updating, "Updating")
            .versions(Some(&installed), Some(&latest)),
    );
    let command = update_command(&executable, spec);
    let outcome = blocking(move || run_with_timeout(command, update_timeout)).await;
    let failure = |message: String| {
        CliUpdateStatus::new(spec.id, CliUpdateState::Failed, message)
            .versions(Some(&installed), Some(&latest))
    };
    match outcome {
        Some(Ok(output)) if output.status.success() => match installed_version(&executable).await {
            Some(now) if now > installed => publish(
                CliUpdateStatus::new(spec.id, CliUpdateState::Updated, "Updated")
                    .versions(Some(&installed), Some(&now)),
            ),
            _ => publish(failure(format!(
                "The updater finished but the version did not change: {}",
                short_detail(&output)
            ))),
        },
        Some(Ok(output)) => publish(failure(format!("Update failed: {}", short_detail(&output)))),
        Some(Err(error)) => publish(failure(format!("Update failed: {error}"))),
        None => publish(failure("Update task stopped unexpectedly".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::Arc;

    use crate::services::codex::CodexProvider;
    use crate::services::provider::TranslationProvider;

    /// Local stand-in for the npm registry: answers every request with the
    /// current `latest` version and records the request lines.
    fn fake_registry(latest: Arc<Mutex<String>>, requests: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).to_string();
                if let Some(line) = request.lines().next() {
                    requests.lock().unwrap().push(line.to_string());
                }
                let body = format!(
                    r#"{{"name":"@openai/codex","version":"{}"}}"#,
                    latest.lock().unwrap()
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    async fn wait_for_state(gate: &CliUpdates, state: CliUpdateState) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !gate
            .snapshot()
            .iter()
            .any(|status| status.cli == CODEX.id && status.state == state)
        {
            assert!(Instant::now() < deadline, "never reached {state:?}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn log_lines(path: &Path) -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    /// Fake Codex install plus fake registry: a newer release is installed
    /// through the CLI's own updater with the child environment the real
    /// installer needs, and a run started meanwhile waits for it; an equal
    /// version, the bundled sidecar, custom paths and an offline registry
    /// never trigger an update. The updater leaves a background process
    /// holding its output pipes, which must not stall the update.
    #[cfg(windows)]
    #[tokio::test]
    async fn codex_update_flow_with_fake_cli_and_registry() {
        let root = std::env::temp_dir().join(format!(
            "fairtranslator-cli-update-{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let standard_root = root.join("standard");
        let bin = standard_root.join("bin");
        let custom_dir = root.join("custom");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&custom_dir).unwrap();
        let log = root.join("calls.log");
        let env_log = root.join("env.log");
        let version_file = root.join("version.txt");
        std::fs::write(&version_file, "codex-cli 1.0.0\r\n").unwrap();
        // Outlives the updater while holding its output pipes, like a daemon,
        // until the test tells it to stop (or a minute passes).
        let linger_stop = root.join("linger.stop");
        let linger_done = root.join("linger.done");
        let linger = root.join("linger.cmd");
        std::fs::write(
            &linger,
            format!(
                "@echo off\r\nfor /l %%i in (1,1,60) do (\r\n  if exist \"{stop}\" goto end\r\n  ping -n 2 127.0.0.1 >nul\r\n)\r\n:end\r\n>\"{done}\" echo done\r\n",
                stop = linger_stop.display(),
                done = linger_done.display()
            ),
        )
        .unwrap();

        let script = bin.join("codex.cmd");
        std::fs::write(
            &script,
            format!(
                r#"@echo off
if "%~1"=="--version" goto version
if "%~1"=="update" goto update
>>"{log}" echo other %*
exit /b 0
:version
>>"{log}" echo version
type "{version_file}"
exit /b 0
:update
>>"{log}" echo update-start
>"{env_log}" echo NONINTERACTIVE=%CODEX_NON_INTERACTIVE%
>>"{env_log}" echo PATH=%PATH%
>>"{env_log}" echo PSMODULEPATH=%PSModulePath%
start "" /b "{linger}"
ping -n 3 127.0.0.1 >nul
>"{version_file}" echo codex-cli 1.1.0
>>"{log}" echo update-end
exit /b 0
"#,
                log = log.display(),
                env_log = env_log.display(),
                version_file = version_file.display(),
                linger = linger.display(),
            ),
        )
        .unwrap();
        // What a run spawns (as `translate_batch_start` does through
        // `validate`), logged apart so the order is unambiguous.
        let run_script = custom_dir.join("codex.cmd");
        std::fs::write(
            &run_script,
            format!(
                "@echo off\r\n>>\"{log}\" echo run %*\r\nif \"%~1\"==\"--version\" echo codex-cli 1.1.0\r\nexit /b 0\r\n",
                log = log.display(),
            ),
        )
        .unwrap();

        let roots = vec![standard_root.clone()];
        let configured = |path: &Path| {
            CodexProvider::new(Some(path.to_string_lossy().into_owned()), None, None, None)
        };
        assert_eq!(
            configured(&script).update_target(&roots).await,
            CliInstall::Standard(script.clone())
        );
        assert_eq!(
            configured(&run_script).update_target(&roots).await,
            CliInstall::Custom(run_script.clone())
        );
        assert_eq!(
            classify_install(Some(&script), true, &roots),
            CliInstall::Sidecar(script.clone())
        );
        assert_eq!(classify_install(None, false, &roots), CliInstall::Missing);

        let latest = Arc::new(Mutex::new("1.1.0".to_string()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let base = fake_registry(latest.clone(), requests.clone());
        let gate: &'static CliUpdates = Box::leak(Box::default());
        let lookup = Duration::from_secs(5);
        let update_timeout = Duration::from_secs(60);

        // Newer release: updated, and a run started meanwhile waits for it.
        let seen = Arc::new(Mutex::new(Vec::new()));
        let update = {
            let (seen, base, script) = (seen.clone(), base.clone(), script.clone());
            tokio::spawn(async move {
                let emit = move |status: &CliUpdateStatus| seen.lock().unwrap().push(status.state);
                check_and_update(
                    &CODEX,
                    CliInstall::Standard(script),
                    &base,
                    lookup,
                    update_timeout,
                    gate,
                    &emit,
                )
                .await
            })
        };
        wait_for_state(gate, CliUpdateState::Updating).await;
        let provider = CodexProvider::new(
            Some(run_script.to_string_lossy().into_owned()),
            None,
            None,
            None,
        )
        .with_update_gate_for_test(gate);
        let _ = provider.validate().await;
        let status = update.await.unwrap();
        assert!(
            !linger_done.exists(),
            "the update waited for the background process"
        );
        assert_eq!(status.state, CliUpdateState::Updated, "{status:?}");
        assert_eq!(status.skip_reason, None);
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "cli": "codex", "state": "updated", "skipReason": null,
                "fromVersion": "1.0.0", "toVersion": "1.1.0", "message": "Updated"
            })
        );
        assert_eq!(status.from_version.as_deref(), Some("1.0.0"));
        assert_eq!(status.to_version.as_deref(), Some("1.1.0"));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                CliUpdateState::Checking,
                CliUpdateState::Updating,
                CliUpdateState::Updated
            ]
        );
        assert!(requests.lock().unwrap()[0].starts_with("GET /@openai/codex/latest "));

        let calls = log_lines(&log);
        let update_start = calls
            .iter()
            .position(|line| line == "update-start")
            .unwrap();
        let update_end = calls.iter().position(|line| line == "update-end").unwrap();
        assert_eq!(
            update_end,
            update_start + 1,
            "something ran mid-update: {calls:?}"
        );
        let first_run = calls
            .iter()
            .position(|line| line.starts_with("run "))
            .unwrap();
        assert!(first_run > update_end, "the run did not wait: {calls:?}");

        let env = log_lines(&env_log);
        assert!(env.contains(&"NONINTERACTIVE=1".to_string()), "{env:?}");
        let path_first = env
            .iter()
            .find_map(|line| line.strip_prefix("PATH="))
            .and_then(|path| path.split(';').next())
            .unwrap()
            .to_ascii_lowercase();
        assert_eq!(
            path_first,
            system32().to_string_lossy().to_ascii_lowercase()
        );
        let module_path = env
            .iter()
            .find_map(|line| line.strip_prefix("PSMODULEPATH="))
            .unwrap()
            .to_ascii_lowercase();
        assert!(module_path.contains("windowspowershell"), "{module_path}");
        assert!(!module_path.contains("powershell\\7"), "{module_path}");

        let emit = |_: &CliUpdateStatus| {};
        let updates =
            |lines: &[String]| lines.iter().filter(|line| *line == "update-start").count();

        // Same version: nothing to do.
        let status = check_and_update(
            &CODEX,
            CliInstall::Standard(script.clone()),
            &base,
            lookup,
            update_timeout,
            gate,
            &emit,
        )
        .await;
        assert_eq!(status.state, CliUpdateState::UpToDate, "{status:?}");
        assert_eq!(updates(&log_lines(&log)), 1);

        // The bundled sidecar, custom paths and missing installs are left
        // alone without even being run.
        let calls_before = log_lines(&log).len();
        for (install, reason) in [
            (CliInstall::Sidecar(script.clone()), CliSkipReason::Bundled),
            (
                CliInstall::Custom(run_script.clone()),
                CliSkipReason::Custom,
            ),
            (CliInstall::Missing, CliSkipReason::NotInstalled),
        ] {
            let status =
                check_and_update(&CODEX, install, &base, lookup, update_timeout, gate, &emit).await;
            assert_eq!(status.state, CliUpdateState::Skipped, "{status:?}");
            assert_eq!(status.skip_reason, Some(reason));
        }
        assert_eq!(log_lines(&log).len(), calls_before);

        // Registry unreachable: offline, the installed version keeps working.
        *latest.lock().unwrap() = "9.9.9".into();
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let offline_base = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let status = check_and_update(
            &CODEX,
            CliInstall::Standard(script.clone()),
            &offline_base,
            lookup,
            update_timeout,
            gate,
            &emit,
        )
        .await;
        assert_eq!(status.state, CliUpdateState::Offline, "{status:?}");
        assert_eq!(status.from_version.as_deref(), Some("1.1.0"));
        assert_eq!(updates(&log_lines(&log)), 1);
        gate.wait_until_idle(CODEX.id, Duration::from_millis(10))
            .await
            .unwrap();

        // Let the background process end before cleaning up.
        std::fs::write(&linger_stop, "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !linger_done.exists() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

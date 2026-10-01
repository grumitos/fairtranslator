//! Background updates of the external CLIs FairTranslator drives
//! (`autoUpdateClis`, on by default). See `services::cli_updates`.

use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

use crate::error::AppError;
use crate::models::ProviderType;
use crate::services::cli_updates::{self, CliUpdateStatus};
use crate::services::codex::CodexProvider;

/// Store file for app-wide, non-secret settings.
const SETTINGS_STORE: &str = "app-settings.json";
const AUTO_UPDATE_KEY: &str = "autoUpdateClis";
/// Event carrying every `CliUpdateStatus` change.
pub const CLI_UPDATE_EVENT: &str = "cli-update:status";

fn auto_update_enabled(app: &AppHandle) -> bool {
    app.store(SETTINGS_STORE)
        .ok()
        .and_then(|store| store.get(AUTO_UPDATE_KEY))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

/// Checks the CLIs in the background without delaying the window. Codex is
/// marked busy before the task starts, so a run that needs it waits (bounded)
/// instead of spawning it while it is being replaced.
pub fn start_background_updates(app: &AppHandle) {
    let registry = cli_updates::registry();
    if !auto_update_enabled(app) {
        registry.mark_disabled(cli_updates::CODEX.id);
        return;
    }
    registry.mark_checking(cli_updates::CODEX.id);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let configured = super::translation::configured_command_path(&app, &ProviderType::Codex);
        let provider = CodexProvider::with_app_handle(app.clone(), configured, None, None, None);
        let install = provider
            .update_target(&cli_updates::codex_standard_roots())
            .await;
        let emit = |status: &CliUpdateStatus| {
            let _ = app.emit(CLI_UPDATE_EVENT, status);
        };
        cli_updates::check_and_update(
            &cli_updates::CODEX,
            install,
            cli_updates::NPM_REGISTRY,
            cli_updates::LOOKUP_TIMEOUT,
            cli_updates::UPDATE_TIMEOUT,
            registry,
            &emit,
        )
        .await;
    });
}

/// Latest update status of each CLI (empty until the first check starts).
#[tauri::command]
pub fn cli_update_status() -> Vec<CliUpdateStatus> {
    cli_updates::registry().snapshot()
}

#[tauri::command]
pub fn cli_auto_update_get(app: AppHandle) -> bool {
    auto_update_enabled(&app)
}

/// Takes effect on the next start.
#[tauri::command]
pub fn cli_auto_update_set(app: AppHandle, enabled: bool) -> Result<(), AppError> {
    let store = app
        .store(SETTINGS_STORE)
        .map_err(|e| AppError::Project(format!("Failed to open settings store: {e}")))?;
    store.set(AUTO_UPDATE_KEY, serde_json::Value::Bool(enabled));
    store
        .save()
        .map_err(|e| AppError::Project(format!("Failed to save settings store: {e}")))
}

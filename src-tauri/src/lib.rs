#[cfg(not(test))]
mod commands;
pub mod error;
pub mod models;
pub mod parser;
pub mod services;

#[cfg(not(test))]
use commands::project::AppState;

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::project::project_create,
            commands::project::project_open,
            commands::project::project_save,
            commands::project::project_get,
            commands::project::project_update_entry,
            commands::project::project_update_entries,
            commands::export::export_project,
            commands::translation::apikey_save,
            commands::translation::apikey_load,
            commands::translation::apikey_delete,
            commands::translation::apikey_validate,
            commands::translation::codex_status,
            commands::translation::codex_login,
            commands::translation::translate_ai_batch_export,
            commands::translation::translate_ai_batch_apply,
            commands::translation::translate_batch_start,
            commands::glossary::glossary_add,
            commands::glossary::glossary_update,
            commands::glossary::glossary_remove,
            commands::glossary::glossary_list,
            commands::style_guide::style_guide_replace,
            commands::style_guide::style_guide_list,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
pub fn run() {}

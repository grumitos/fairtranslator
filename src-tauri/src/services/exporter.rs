use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::AppError;
use crate::models::{Engine, EntryStatus, Project};
use crate::parser::rpgmaker::{
    classify_file, classify_relative_path, parse_plugins_js, render_plugins_js, FileKind,
    CODE_CHANGE_NAME, CODE_CHANGE_NICKNAME, CODE_CHANGE_PROFILE, CODE_SCROLL_TEXT,
    CODE_SHOW_CHOICES, CODE_SHOW_TEXT, DB_TEXT_FIELDS,
};
use crate::services::translation_integrity;

const LEGACY_DB_TEXT_FIELDS: &[(&str, &str)] = &[
    ("name", "@name"),
    ("description", "@description"),
    ("nickname", "@nickname"),
    ("message1", "@message1"),
    ("message2", "@message2"),
    ("message3", "@message3"),
    ("message4", "@message4"),
];

const SAFE_WEB_FONT_FALLBACKS: &[&str] = &[
    "\"Segoe UI\"",
    "Meiryo",
    "\"Hiragino Sans\"",
    "\"Noto Sans\"",
    "Verdana",
    "Arial",
    "sans-serif",
];

// ---------------------------------------------------------------------------
// ExportResult
// ---------------------------------------------------------------------------

/// Result returned after exporting a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// Number of game files written to the output directory.
    pub files_exported: usize,
    /// Number of translation entries successfully injected.
    pub entries_applied: usize,
    /// Total number of entries in the project (for context).
    pub total_entries: usize,
    /// Non-blocking integrity warnings discovered during export preflight.
    pub warnings: Vec<String>,
}

trait EngineExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError>;
}

struct MvMzExporter;
struct VxAceExporter;
struct VxExporter;
struct XpExporter;

impl EngineExporter for MvMzExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError> {
        export_mv_mz_project(project, output_dir)
    }
}

impl EngineExporter for VxAceExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError> {
        export_vx_ace_project(project, output_dir)
    }
}

impl EngineExporter for VxExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError> {
        export_vx_project(project, output_dir)
    }
}

impl EngineExporter for XpExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError> {
        export_xp_project(project, output_dir)
    }
}

static MV_MZ_EXPORTER: MvMzExporter = MvMzExporter;
static VXACE_EXPORTER: VxAceExporter = VxAceExporter;
static VX_EXPORTER: VxExporter = VxExporter;
static XP_EXPORTER: XpExporter = XpExporter;

fn exporter_for_engine(engine: &Engine) -> Result<&'static dyn EngineExporter, AppError> {
    match engine {
        Engine::Mv | Engine::Mz => Ok(&MV_MZ_EXPORTER),
        Engine::VxAce => Ok(&VXACE_EXPORTER),
        Engine::Vx => Ok(&VX_EXPORTER),
        Engine::Xp => Ok(&XP_EXPORTER),
    }
}

// ---------------------------------------------------------------------------
// Main export function
// ---------------------------------------------------------------------------

/// Export all translated entries back into the original game JSON files.
///
/// Copies the original game into `output_dir`, injects translations into the
/// copied files, and never writes back into the source game directory.
///
/// Only entries with a non-empty translation and a status other than
/// `Untranslated` are applied. Original files are never modified.
pub fn export_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let exporter = exporter_for_engine(&project.engine)?;
    exporter.export_project(project, output_dir)
}

fn export_mv_mz_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let game_dir = validate_game_dir(project)?;
    prepare_export_destination(&game_dir, output_dir)?;
    validate_project_source_files(project, &game_dir)?;
    let validation = translation_integrity::validate_project_for_export(project);
    if validation.has_errors() {
        return Err(AppError::Project(
            translation_integrity::summarize_export_errors(&validation.errors),
        ));
    }
    if let Err(error) = copy_dir_recursive(&game_dir, output_dir) {
        let _ = std::fs::remove_dir_all(output_dir);
        return Err(error);
    }

    let (translations, total_entries) = collect_translations(project);

    let prefer_system_fonts = project_requires_safe_web_fonts(project);
    let mut export_warnings = validation.warnings.clone();

    let export_result = (|| {
        let mut files_exported: usize = 0;
        let mut entries_applied: usize = 0;
        let mut font_fallback_applied = false;

        for file in &project.files {
            let copied_path = output_dir.join(&file.relative_path);
            let content = std::fs::read_to_string(&copied_path)?;
            let stem = Path::new(&file.relative_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let kind = classify_relative_path(&file.relative_path);
            let mut json = match kind {
                FileKind::Plugins => parse_plugins_js(&content)?,
                _ => serde_json::from_str(&content)?,
            };

            let applied = apply_translations(&stem, &kind, &mut json, &translations)?;
            entries_applied += applied;

            if prefer_system_fonts && stem == "System" {
                font_fallback_applied |= patch_mv_mz_system_font_preferences(&mut json);
            }

            let output_content = match kind {
                FileKind::Plugins => render_plugins_js(&json)?,
                _ => serde_json::to_string_pretty(&json)?,
            };
            std::fs::write(&copied_path, output_content)?;

            files_exported += 1;
        }

        if prefer_system_fonts {
            font_fallback_applied |= patch_exported_mv_mz_system_font_file(output_dir)?;
        }

        if font_fallback_applied {
            export_warnings.push(
                "Export forced safe system fonts for translated Latin text; the original webfont stayed in the copy, but dialogue/menu rendering now prefers a glyph-complete fallback stack.".into(),
            );
        }

        Ok(ExportResult {
            files_exported,
            entries_applied,
            total_entries,
            warnings: export_warnings.clone(),
        })
    })();

    if export_result.is_err() {
        let _ = std::fs::remove_dir_all(output_dir);
    }

    export_result
}

fn export_vx_ace_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    export_legacy_project(project, output_dir)
}

fn export_vx_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    export_legacy_project(project, output_dir)
}

fn export_xp_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    export_legacy_project(project, output_dir)
}

fn export_legacy_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let game_dir = validate_game_dir(project)?;
    prepare_export_destination(&game_dir, output_dir)?;
    validate_project_source_files(project, &game_dir)?;
    let validation = translation_integrity::validate_project_for_export(project);
    if validation.has_errors() {
        return Err(AppError::Project(
            translation_integrity::summarize_export_errors(&validation.errors),
        ));
    }
    if let Err(error) = copy_dir_recursive(&game_dir, output_dir) {
        let _ = std::fs::remove_dir_all(output_dir);
        return Err(error);
    }

    let (translations, total_entries) = collect_translations(project);
    let export_result = (|| {
        let mut files_exported: usize = 0;
        let mut entries_applied: usize = 0;

        for file in &project.files {
            let output_path = output_dir.join(&file.relative_path);
            let stem = Path::new(&file.relative_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let mut value = read_marshal_value(&output_path)?;
            let applied = apply_legacy_translations(&stem, &mut value, &translations)?;
            entries_applied += applied;

            write_marshal_value(&output_path, &value)?;
            files_exported += 1;
        }

        Ok(ExportResult {
            files_exported,
            entries_applied,
            total_entries,
            warnings: validation.warnings.clone(),
        })
    })();

    if export_result.is_err() {
        let _ = std::fs::remove_dir_all(output_dir);
    }

    export_result
}

fn validate_game_dir(project: &Project) -> Result<PathBuf, AppError> {
    let game_dir = PathBuf::from(&project.game_dir);
    if !game_dir.is_dir() {
        return Err(AppError::NotFound(format!(
            "Game directory not found: {}",
            game_dir.display()
        )));
    }

    Ok(game_dir)
}

fn prepare_export_destination(game_dir: &Path, output_dir: &Path) -> Result<(), AppError> {
    if is_same_or_nested(game_dir, output_dir)? {
        return Err(AppError::Project(format!(
            "Export destination '{}' must be outside the original game directory '{}'.",
            output_dir.display(),
            game_dir.display()
        )));
    }

    if output_dir.exists() {
        if !output_dir.is_dir() {
            return Err(AppError::Project(format!(
                "Export destination is not a directory: {}",
                output_dir.display()
            )));
        }

        if std::fs::read_dir(output_dir)?.next().transpose()?.is_some() {
            return Err(AppError::Project(format!(
                "Export destination '{}' is not empty. Choose an empty folder.",
                output_dir.display()
            )));
        }
    } else {
        std::fs::create_dir_all(output_dir)?;
    }

    Ok(())
}

fn validate_project_source_files(project: &Project, game_dir: &Path) -> Result<(), AppError> {
    let mut missing_files: Vec<String> = project
        .files
        .iter()
        .filter_map(|file| {
            let path = game_dir.join(&file.relative_path);
            if path.is_file() {
                None
            } else {
                Some(file.relative_path.clone())
            }
        })
        .collect();

    if missing_files.is_empty() {
        return Ok(());
    }

    missing_files.sort();
    Err(AppError::NotFound(format!(
        "Cannot export because source game files are missing: {}",
        missing_files.join(", ")
    )))
}

fn copy_dir_recursive(source_dir: &Path, target_dir: &Path) -> Result<(), AppError> {
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());

        if source_path.is_dir() {
            std::fs::create_dir_all(&target_path)?;
            copy_dir_recursive(&source_path, &target_path)?;
        } else if source_path.is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &target_path)?;
        }
    }

    Ok(())
}

fn is_same_or_nested(base_dir: &Path, candidate_dir: &Path) -> Result<bool, AppError> {
    let normalized_base = normalize_path(base_dir)?;
    let normalized_candidate = normalize_path(candidate_dir)?;

    Ok(normalized_candidate == normalized_base
        || normalized_candidate.starts_with(&normalized_base))
}

fn normalize_path(path: &Path) -> Result<PathBuf, AppError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    Ok(normalize_lexically(&absolute))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

fn collect_translations(project: &Project) -> (HashMap<String, String>, usize) {
    let mut translations: HashMap<String, String> = HashMap::new();
    let mut total_entries: usize = 0;

    for file in &project.files {
        for entry in &file.entries {
            total_entries += 1;
            if entry.status != EntryStatus::Untranslated && !entry.translation.trim().is_empty() {
                translations.insert(entry.id.clone(), entry.translation.clone());
            }
        }
    }

    (translations, total_entries)
}

fn project_requires_safe_web_fonts(project: &Project) -> bool {
    project
        .files
        .iter()
        .flat_map(|file| file.entries.iter())
        .any(|entry| {
            entry.status != EntryStatus::Untranslated
                && !entry.translation.trim().is_empty()
                && contains_extended_latin_chars(&entry.translation)
        })
}

fn contains_extended_latin_chars(text: &str) -> bool {
    text.chars()
        .any(|ch| ch == '¡' || ch == '¿' || matches!(ch, '\u{00C0}'..='\u{024F}'))
}

fn build_safe_web_font_stack(existing: &str) -> String {
    let mut fonts: Vec<String> = Vec::new();

    for font in SAFE_WEB_FONT_FALLBACKS {
        if !fonts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(font))
        {
            fonts.push((*font).to_string());
        }
    }

    for font in existing
        .split(',')
        .map(str::trim)
        .filter(|font| !font.is_empty())
    {
        if !fonts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(font))
        {
            fonts.push(font.to_string());
        }
    }

    fonts.join(", ")
}

fn patch_mv_mz_system_font_preferences(system_json: &mut Value) -> bool {
    let Some(advanced) = system_json
        .get_mut("advanced")
        .and_then(Value::as_object_mut)
    else {
        return false;
    };

    let current_fallbacks = advanced
        .get("fallbackFonts")
        .and_then(Value::as_str)
        .unwrap_or("sans-serif")
        .to_string();
    let desired_fallbacks = build_safe_web_font_stack(&current_fallbacks);
    let current_main_font = advanced
        .get("mainFontFilename")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut changed = false;

    if !current_main_font.is_empty() {
        advanced.insert("mainFontFilename".into(), Value::String(String::new()));
        changed = true;
    }

    if current_fallbacks != desired_fallbacks {
        advanced.insert("fallbackFonts".into(), Value::String(desired_fallbacks));
        changed = true;
    }

    changed
}

fn patch_exported_mv_mz_system_font_file(output_dir: &Path) -> Result<bool, AppError> {
    let system_path = output_dir.join("data").join("System.json");
    if !system_path.is_file() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&system_path)?;
    let mut json: Value = serde_json::from_str(&content)?;
    let changed = patch_mv_mz_system_font_preferences(&mut json);
    if changed {
        let output_content = serde_json::to_string_pretty(&json)?;
        std::fs::write(&system_path, output_content)?;
    }

    Ok(changed)
}

fn read_marshal_value(path: &Path) -> Result<Value, AppError> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let value: rpgtool_common::Value =
        rpgtool_common::conv_read(rpgtool_common::Format::Marshal, &mut reader).map_err(|e| {
            AppError::Parse(format!(
                "Failed to read marshal file '{}': {e}",
                path.display()
            ))
        })?;

    serde_json::to_value(&value).map_err(|e| {
        AppError::Parse(format!(
            "Failed to convert marshal value to JSON for '{}': {e}",
            path.display()
        ))
    })
}

fn write_marshal_value(path: &Path, value: &Value) -> Result<(), AppError> {
    let marshal_value: rpgtool_common::Value =
        serde_json::from_value(value.clone()).map_err(|e| {
            AppError::Parse(format!(
                "Failed to convert JSON to marshal value for '{}': {e}",
                path.display()
            ))
        })?;
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    rpgtool_common::conv_write(marshal_value, rpgtool_common::Format::Marshal, &mut writer)
        .map_err(|e| {
            AppError::Parse(format!(
                "Failed to write marshal file '{}': {e}",
                path.display()
            ))
        })?;
    Ok(())
}

fn apply_legacy_translations(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    match classify_file(stem) {
        FileKind::Database => apply_vxace_database(stem, json, translations),
        FileKind::Map => apply_vxace_map(stem, json, translations),
        FileKind::CommonEvents => apply_vxace_common_events(json, translations),
        FileKind::Troops => apply_vxace_troops(json, translations),
        FileKind::System => apply_vxace_system(json, translations),
        FileKind::Plugins => Ok(0),
        FileKind::Ignored => Ok(0),
    }
}

fn apply_vxace_database(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected array for VX Ace database")))?;

    let mut applied = 0;

    for (idx, item) in arr.iter_mut().enumerate() {
        let Some(fields) = object_fields_mut(item) else {
            continue;
        };

        let item_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);

        for (field_name, legacy_key) in LEGACY_DB_TEXT_FIELDS {
            let entry_id = format!("{stem}:{field_name}:{item_id}");
            if let Some(translation) = translations.get(&entry_id) {
                if let Some(value) = fields.get_mut(*legacy_key) {
                    if set_string_value(value, translation) {
                        applied += 1;
                    }
                }
            }
        }
    }

    Ok(applied)
}

fn apply_vxace_map(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let fields = object_fields_mut(json)
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected RPG::Map object")))?;

    let mut applied = 0;

    if let Some(translation) = translations.get(&format!("{stem}:displayName")) {
        if let Some(display_name) = fields.get_mut("@display_name") {
            if set_string_value(display_name, translation) {
                applied += 1;
            }
        }
    }

    if let Some(events_hash) = fields.get_mut("@events") {
        if let Some(hash_entries) = hash_entries_mut(events_hash) {
            for hash_entry in hash_entries.iter_mut() {
                let Some(event) = hash_entry.get_mut("value") else {
                    continue;
                };
                let Some(event_fields) = object_fields_mut(event) else {
                    continue;
                };
                let ev_id = event_fields
                    .get("@id")
                    .and_then(Value::as_i64)
                    .map(|v| v as u64)
                    .unwrap_or(0);

                if let Some(pages) = event_fields.get_mut("@pages").and_then(Value::as_array_mut) {
                    for (page_idx, page) in pages.iter_mut().enumerate() {
                        let Some(page_fields) = object_fields_mut(page) else {
                            continue;
                        };
                        if let Some(list) =
                            page_fields.get_mut("@list").and_then(Value::as_array_mut)
                        {
                            let id_prefix = format!("{stem}:{ev_id}:{page_idx}");
                            applied += apply_vxace_event_commands(
                                list.as_mut_slice(),
                                &id_prefix,
                                translations,
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(applied)
}

fn apply_vxace_common_events(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse("CommonEvents: expected array".into()))?;

    let mut applied = 0;
    for (idx, item) in arr.iter_mut().enumerate() {
        let Some(fields) = object_fields_mut(item) else {
            continue;
        };
        let ce_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);
        if let Some(list) = fields.get_mut("@list").and_then(Value::as_array_mut) {
            applied += apply_vxace_event_commands(
                list.as_mut_slice(),
                &format!("CommonEvents:{ce_id}"),
                translations,
            );
        }
    }

    Ok(applied)
}

fn apply_vxace_troops(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse("Troops: expected array".into()))?;

    let mut applied = 0;
    for (idx, item) in arr.iter_mut().enumerate() {
        let Some(fields) = object_fields_mut(item) else {
            continue;
        };
        let troop_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);

        if let Some(translation) = translations.get(&format!("Troops:name:{troop_id}")) {
            if let Some(name) = fields.get_mut("@name") {
                if set_string_value(name, translation) {
                    applied += 1;
                }
            }
        }

        if let Some(pages) = fields.get_mut("@pages").and_then(Value::as_array_mut) {
            for (page_idx, page) in pages.iter_mut().enumerate() {
                let Some(page_fields) = object_fields_mut(page) else {
                    continue;
                };
                if let Some(list) = page_fields.get_mut("@list").and_then(Value::as_array_mut) {
                    applied += apply_vxace_event_commands(
                        list.as_mut_slice(),
                        &format!("Troops:{troop_id}:{page_idx}"),
                        translations,
                    );
                }
            }
        }
    }

    Ok(applied)
}

fn apply_vxace_system(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let fields = object_fields_mut(json)
        .ok_or_else(|| AppError::Parse("System: expected RPG::System object".into()))?;

    let mut applied = 0;

    if let Some(translation) = translations.get("System:gameTitle") {
        if let Some(title) = fields.get_mut("@game_title") {
            if set_string_value(title, translation) {
                applied += 1;
            }
        }
    }

    if let Some(terms) = fields.get_mut("@terms") {
        applied += apply_vxace_terms_recursive(terms, "System:terms", translations);
    }

    Ok(applied)
}

fn apply_vxace_terms_recursive(
    value: &mut Value,
    id_prefix: &str,
    translations: &HashMap<String, String>,
) -> usize {
    if let Some(translation) = translations.get(id_prefix) {
        if set_string_value(value, translation) {
            return 1;
        }
    }

    if string_from_legacy_value(value).is_some() {
        return 0;
    }

    if let Some(arr) = value.as_array_mut() {
        let mut applied = 0;
        for (i, val) in arr.iter_mut().enumerate() {
            applied += apply_vxace_terms_recursive(val, &format!("{id_prefix}.{i}"), translations);
        }
        return applied;
    }

    let Some(fields) = object_fields_mut(value) else {
        return 0;
    };
    let keys: Vec<String> = fields.keys().cloned().collect();
    let mut applied = 0;
    for key in keys {
        if let Some(val) = fields.get_mut(&key) {
            let clean_key = key.trim_start_matches('@');
            applied +=
                apply_vxace_terms_recursive(val, &format!("{id_prefix}.{clean_key}"), translations);
        }
    }

    applied
}

fn apply_vxace_event_commands(
    list: &mut [Value],
    id_prefix: &str,
    translations: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    let mut i = 0;

    while i < list.len() {
        let code = command_code_legacy(&list[i]).unwrap_or(0);

        match code {
            CODE_SHOW_TEXT => {
                let start = i;
                let mut line_indices: Vec<usize> = vec![i];
                i += 1;
                while i < list.len() {
                    if command_code_legacy(&list[i]).unwrap_or(0) == CODE_SHOW_TEXT {
                        line_indices.push(i);
                        i += 1;
                    } else {
                        break;
                    }
                }

                let entry_id = format!("{id_prefix}:text:{start}");
                if let Some(translation) = translations.get(&entry_id) {
                    apply_vxace_multiline_text(list, &line_indices, translation);
                    applied += 1;
                }
                continue;
            }
            CODE_SCROLL_TEXT => {
                let start = i;
                let mut line_indices: Vec<usize> = vec![i];
                i += 1;
                while i < list.len() {
                    if command_code_legacy(&list[i]).unwrap_or(0) == CODE_SCROLL_TEXT {
                        line_indices.push(i);
                        i += 1;
                    } else {
                        break;
                    }
                }

                let entry_id = format!("{id_prefix}:scroll:{start}");
                if let Some(translation) = translations.get(&entry_id) {
                    apply_vxace_multiline_text(list, &line_indices, translation);
                    applied += 1;
                }
                continue;
            }
            CODE_SHOW_CHOICES => {
                if let Some(params) = command_parameters_legacy_mut(&mut list[i]) {
                    if let Some(choices) = params.get_mut(0).and_then(Value::as_array_mut) {
                        for (ci, choice) in choices.iter_mut().enumerate() {
                            let entry_id = format!("{id_prefix}:choice:{i}:{ci}");
                            if let Some(translation) = translations.get(&entry_id) {
                                if set_string_value(choice, translation) {
                                    applied += 1;
                                }
                            }
                        }
                    }
                }
            }
            CODE_CHANGE_NAME => {
                let entry_id = format!("{id_prefix}:changeName:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    if set_vxace_parameter(list, i, 1, translation) {
                        applied += 1;
                    }
                }
            }
            CODE_CHANGE_NICKNAME => {
                let entry_id = format!("{id_prefix}:changeNickname:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    if set_vxace_parameter(list, i, 1, translation) {
                        applied += 1;
                    }
                }
            }
            CODE_CHANGE_PROFILE => {
                let entry_id = format!("{id_prefix}:changeProfile:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    if set_vxace_parameter(list, i, 1, translation) {
                        applied += 1;
                    }
                }
            }
            _ => {}
        }

        i += 1;
    }

    applied
}

fn apply_vxace_multiline_text(list: &mut [Value], line_indices: &[usize], translation: &str) {
    let lines: Vec<&str> = translation.split('\n').collect();
    let num_commands = line_indices.len();

    for (j, &cmd_idx) in line_indices.iter().enumerate() {
        let text = if j < lines.len() {
            if j == num_commands - 1 && lines.len() > num_commands {
                lines[j..].join("\n")
            } else {
                lines[j].to_string()
            }
        } else {
            String::new()
        };
        let _ = set_vxace_parameter(list, cmd_idx, 0, &text);
    }
}

fn set_vxace_parameter(list: &mut [Value], cmd_idx: usize, param_idx: usize, value: &str) -> bool {
    let Some(params) = command_parameters_legacy_mut(&mut list[cmd_idx]) else {
        return false;
    };
    let Some(parameter) = params.get_mut(param_idx) else {
        return false;
    };
    set_string_value(parameter, value)
}

fn object_fields(value: &Value) -> Option<&Map<String, Value>> {
    value
        .get("$object")
        .and_then(|o| o.get("fields"))
        .and_then(Value::as_object)
}

fn object_fields_mut(value: &mut Value) -> Option<&mut Map<String, Value>> {
    value
        .get_mut("$object")
        .and_then(|o| o.get_mut("fields"))
        .and_then(Value::as_object_mut)
}

fn hash_entries_mut(value: &mut Value) -> Option<&mut Vec<Value>> {
    value.get_mut("$hash").and_then(Value::as_array_mut)
}

fn command_code_legacy(cmd: &Value) -> Option<i64> {
    object_fields(cmd)?.get("@code").and_then(Value::as_i64)
}

fn command_parameters_legacy_mut(cmd: &mut Value) -> Option<&mut Vec<Value>> {
    object_fields_mut(cmd)?
        .get_mut("@parameters")
        .and_then(Value::as_array_mut)
}

fn string_from_legacy_value(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }

    if let Some(instance_value) = value.get("$instance").and_then(|v| v.get("value")) {
        return string_from_legacy_value(instance_value);
    }

    if let Some(bytes) = value.get("$string").and_then(Value::as_array) {
        let decoded = bytes
            .iter()
            .filter_map(|b| b.as_u64().map(|n| n as u8))
            .collect::<Vec<u8>>();
        return Some(String::from_utf8_lossy(&decoded).to_string());
    }

    None
}

fn set_string_value(target: &mut Value, translation: &str) -> bool {
    if target.is_string() {
        *target = Value::String(translation.to_string());
        return true;
    }

    if let Some(instance_value) = target.get_mut("$instance").and_then(|v| v.get_mut("value")) {
        return set_string_value(instance_value, translation);
    }

    if let Some(bytes) = target.get_mut("$string").and_then(Value::as_array_mut) {
        *bytes = translation
            .as_bytes()
            .iter()
            .map(|b| Value::from(*b as u64))
            .collect();
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Translation application — dispatcher
// ---------------------------------------------------------------------------

fn apply_translations(
    stem: &str,
    kind: &FileKind,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    match kind {
        FileKind::Database => apply_database(stem, json, translations),
        FileKind::Map => apply_map(stem, json, translations),
        FileKind::CommonEvents => apply_common_events(json, translations),
        FileKind::Troops => apply_troops(json, translations),
        FileKind::System => apply_system(json, translations),
        FileKind::Plugins => apply_plugins(json, translations),
        FileKind::Ignored => Ok(0),
    }
}

fn apply_plugins(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let plugins = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse("Plugins: expected JSON array".into()))?;

    let mut applied = 0;

    for (plugin_index, plugin) in plugins.iter_mut().enumerate() {
        let Some(plugin_obj) = plugin.as_object_mut() else {
            continue;
        };

        if let Some(description) = plugin_obj.get_mut("description") {
            applied += apply_plugin_value(
                description,
                &format!("Plugins:{plugin_index}:description"),
                translations,
            )?;
        }

        if let Some(parameters) = plugin_obj
            .get_mut("parameters")
            .and_then(Value::as_object_mut)
        {
            for (key, value) in parameters.iter_mut() {
                applied += apply_plugin_value(
                    value,
                    &format!("Plugins:{plugin_index}:parameters:{key}"),
                    translations,
                )?;
            }
        }
    }

    Ok(applied)
}

fn apply_plugin_value(
    value: &mut Value,
    id_prefix: &str,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    match value {
        Value::String(text) => {
            if let Some(mut parsed) = parse_embedded_plugin_json(text) {
                let applied =
                    apply_plugin_value(&mut parsed, &format!("{id_prefix}:json"), translations)?;
                if applied > 0 {
                    *text = serde_json::to_string(&parsed)?;
                }
                return Ok(applied);
            }

            if let Some(translation) = translations.get(id_prefix) {
                if text != translation {
                    *text = translation.clone();
                    return Ok(1);
                }
            }

            Ok(0)
        }
        Value::Object(map) => {
            let mut applied = 0;
            for (key, child) in map.iter_mut() {
                applied += apply_plugin_value(child, &format!("{id_prefix}:{key}"), translations)?;
            }
            Ok(applied)
        }
        Value::Array(items) => {
            let mut applied = 0;
            for (index, child) in items.iter_mut().enumerate() {
                applied +=
                    apply_plugin_value(child, &format!("{id_prefix}:{index}"), translations)?;
            }
            Ok(applied)
        }
        _ => Ok(0),
    }
}

fn parse_embedded_plugin_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

// ---------------------------------------------------------------------------
// Database (Actors, Items, Skills, Weapons, Armors, etc.)
// ---------------------------------------------------------------------------

fn apply_database(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected JSON array")))?;

    let mut applied = 0;

    for (idx, item) in arr.iter_mut().enumerate() {
        if item.is_null() {
            continue;
        }
        let obj = match item.as_object_mut() {
            Some(o) => o,
            None => continue,
        };

        let item_id = obj.get("id").and_then(|v| v.as_u64()).unwrap_or(idx as u64);

        for &field in DB_TEXT_FIELDS {
            let entry_id = format!("{stem}:{field}:{item_id}");
            if let Some(translation) = translations.get(&entry_id) {
                if let Some(val) = obj.get_mut(field) {
                    if val.is_string() {
                        *val = Value::String(translation.clone());
                        applied += 1;
                    }
                }
            }
        }
    }

    Ok(applied)
}

// ---------------------------------------------------------------------------
// Map (displayName + event commands)
// ---------------------------------------------------------------------------

fn apply_map(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let obj = json
        .as_object_mut()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected JSON object")))?;

    let mut applied = 0;

    // displayName
    let dn_id = format!("{stem}:displayName");
    if let Some(translation) = translations.get(&dn_id) {
        if let Some(val) = obj.get_mut("displayName") {
            if val.is_string() {
                *val = Value::String(translation.clone());
                applied += 1;
            }
        }
    }

    // Events
    if let Some(Value::Array(events)) = obj.get_mut("events") {
        for (ev_idx, event) in events.iter_mut().enumerate() {
            if event.is_null() {
                continue;
            }
            let ev_id = event
                .get("id")
                .and_then(|v| v.as_u64())
                .unwrap_or(ev_idx as u64);

            if let Some(Value::Array(pages)) = event.get_mut("pages") {
                for (page_idx, page) in pages.iter_mut().enumerate() {
                    if let Some(Value::Array(list)) = page.get_mut("list") {
                        let id_prefix = format!("{stem}:{ev_id}:{page_idx}");
                        applied += apply_event_commands(list, &id_prefix, translations);
                    }
                }
            }
        }
    }

    Ok(applied)
}

// ---------------------------------------------------------------------------
// CommonEvents
// ---------------------------------------------------------------------------

fn apply_common_events(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse("CommonEvents: expected JSON array".into()))?;

    let mut applied = 0;

    for (idx, item) in arr.iter_mut().enumerate() {
        if item.is_null() {
            continue;
        }
        let ce_id = item
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or(idx as u64);

        if let Some(Value::Array(list)) = item.get_mut("list") {
            let id_prefix = format!("CommonEvents:{ce_id}");
            applied += apply_event_commands(list, &id_prefix, translations);
        }
    }

    Ok(applied)
}

// ---------------------------------------------------------------------------
// Troops (name + battle event commands)
// ---------------------------------------------------------------------------

fn apply_troops(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let arr = json
        .as_array_mut()
        .ok_or_else(|| AppError::Parse("Troops: expected JSON array".into()))?;

    let mut applied = 0;

    for (idx, item) in arr.iter_mut().enumerate() {
        if item.is_null() {
            continue;
        }
        let troop_id = item
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or(idx as u64);

        // Troop name
        let name_id = format!("Troops:name:{troop_id}");
        if let Some(translation) = translations.get(&name_id) {
            if let Some(obj) = item.as_object_mut() {
                if let Some(val) = obj.get_mut("name") {
                    if val.is_string() {
                        *val = Value::String(translation.clone());
                        applied += 1;
                    }
                }
            }
        }

        // Battle event pages
        if let Some(Value::Array(pages)) = item.get_mut("pages") {
            for (page_idx, page) in pages.iter_mut().enumerate() {
                if let Some(Value::Array(list)) = page.get_mut("list") {
                    let id_prefix = format!("Troops:{troop_id}:{page_idx}");
                    applied += apply_event_commands(list, &id_prefix, translations);
                }
            }
        }
    }

    Ok(applied)
}

// ---------------------------------------------------------------------------
// System (gameTitle + terms)
// ---------------------------------------------------------------------------

fn apply_system(
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let obj = json
        .as_object_mut()
        .ok_or_else(|| AppError::Parse("System: expected JSON object".into()))?;

    let mut applied = 0;

    // gameTitle
    if let Some(translation) = translations.get("System:gameTitle") {
        if let Some(val) = obj.get_mut("gameTitle") {
            if val.is_string() {
                *val = Value::String(translation.clone());
                applied += 1;
            }
        }
    }

    // terms — recursive injection
    if let Some(terms) = obj.get_mut("terms") {
        applied += apply_terms_recursive(terms, "System:terms", translations);
    }

    Ok(applied)
}

/// Recursively apply translations to the terms object.
/// Mirrors `parser::rpgmaker::extract_terms_recursive`.
fn apply_terms_recursive(
    value: &mut Value,
    id_prefix: &str,
    translations: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;

    match value {
        Value::String(_) => {
            if let Some(translation) = translations.get(id_prefix) {
                *value = Value::String(translation.clone());
                applied += 1;
            }
        }
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if let Some(val) = map.get_mut(&key) {
                    applied +=
                        apply_terms_recursive(val, &format!("{id_prefix}.{key}"), translations);
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter_mut().enumerate() {
                if val.is_string() {
                    applied +=
                        apply_terms_recursive(val, &format!("{id_prefix}.{i}"), translations);
                }
            }
        }
        _ => {}
    }

    applied
}

// ---------------------------------------------------------------------------
// Event commands (shared by Maps, CommonEvents, Troops)
// ---------------------------------------------------------------------------

/// Apply translations to event commands in a list.
/// Mirrors `parser::rpgmaker::extract_event_commands`.
fn apply_event_commands(
    list: &mut Vec<Value>,
    id_prefix: &str,
    translations: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    let mut i = 0;

    while i < list.len() {
        let code = list[i].get("code").and_then(|v| v.as_i64()).unwrap_or(0);

        match code {
            CODE_SHOW_TEXT => {
                // Collect consecutive 401 commands (multi-line text)
                let start = i;
                let mut line_indices: Vec<usize> = vec![i];
                i += 1;
                while i < list.len() {
                    let next_code = list[i].get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                    if next_code == CODE_SHOW_TEXT {
                        line_indices.push(i);
                        i += 1;
                    } else {
                        break;
                    }
                }

                let entry_id = format!("{id_prefix}:text:{start}");
                if let Some(translation) = translations.get(&entry_id) {
                    apply_multiline_text(list, &line_indices, translation);
                    applied += 1;
                }
                continue; // i already advanced past the group
            }
            CODE_SCROLL_TEXT => {
                // Collect consecutive 405 commands (scrolling text)
                let start = i;
                let mut line_indices: Vec<usize> = vec![i];
                i += 1;
                while i < list.len() {
                    let next_code = list[i].get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                    if next_code == CODE_SCROLL_TEXT {
                        line_indices.push(i);
                        i += 1;
                    } else {
                        break;
                    }
                }

                let entry_id = format!("{id_prefix}:scroll:{start}");
                if let Some(translation) = translations.get(&entry_id) {
                    apply_multiline_text(list, &line_indices, translation);
                    applied += 1;
                }
                continue;
            }
            CODE_SHOW_CHOICES => {
                // Replace individual choice strings
                if let Some(params) = list[i].get_mut("parameters") {
                    if let Some(arr) = params.as_array_mut() {
                        if let Some(choices) = arr.first_mut().and_then(|v| v.as_array_mut()) {
                            for (ci, choice) in choices.iter_mut().enumerate() {
                                let entry_id = format!("{id_prefix}:choice:{i}:{ci}");
                                if let Some(translation) = translations.get(&entry_id) {
                                    *choice = Value::String(translation.clone());
                                    applied += 1;
                                }
                            }
                        }
                    }
                }
            }
            CODE_CHANGE_NAME => {
                let entry_id = format!("{id_prefix}:changeName:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    set_parameter(list, i, 1, translation);
                    applied += 1;
                }
            }
            CODE_CHANGE_NICKNAME => {
                let entry_id = format!("{id_prefix}:changeNickname:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    set_parameter(list, i, 1, translation);
                    applied += 1;
                }
            }
            CODE_CHANGE_PROFILE => {
                let entry_id = format!("{id_prefix}:changeProfile:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    set_parameter(list, i, 1, translation);
                    applied += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    applied
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply a multi-line translation to consecutive 401/405 commands.
///
/// - Lines are distributed 1:1 across the commands.
/// - If the translation has fewer lines, excess commands get empty strings.
/// - If the translation has more lines, excess lines are joined into the
///   last command (a limitation for MVP — Phase 6 may insert new commands).
fn apply_multiline_text(list: &mut [Value], line_indices: &[usize], translation: &str) {
    let lines: Vec<&str> = translation.split('\n').collect();
    let num_commands = line_indices.len();

    for (j, &cmd_idx) in line_indices.iter().enumerate() {
        let text = if j < lines.len() {
            if j == num_commands - 1 && lines.len() > num_commands {
                // Last command receives all remaining lines
                lines[j..].join("\n")
            } else {
                lines[j].to_string()
            }
        } else {
            // Translation has fewer lines — empty this command
            String::new()
        };

        if let Some(params) = list[cmd_idx].get_mut("parameters") {
            if let Some(arr) = params.as_array_mut() {
                if !arr.is_empty() {
                    arr[0] = Value::String(text);
                }
            }
        }
    }
}

/// Set a string parameter at a specific index in a command's parameters array.
fn set_parameter(list: &mut [Value], cmd_idx: usize, param_idx: usize, value: &str) {
    if let Some(params) = list[cmd_idx].get_mut("parameters") {
        if let Some(arr) = params.as_array_mut() {
            if arr.len() > param_idx {
                arr[param_idx] = Value::String(value.to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Engine, TranslationEntry, TranslationFile};
    use rpgtool_common::{conv_read, conv_write, Format};
    use serde_json::json;
    use std::fs;
    use std::io::{BufReader, BufWriter};

    fn make_entry(
        id: &str,
        original: &str,
        translation: &str,
        status: EntryStatus,
    ) -> TranslationEntry {
        TranslationEntry {
            id: id.to_string(),
            file_context: String::new(),
            original: original.to_string(),
            translation: translation.to_string(),
            status,
            notes: String::new(),
        }
    }

    fn write_marshal(path: &Path, json_value: Value) {
        let value: rpgtool_common::Value = serde_json::from_value(json_value).unwrap();
        let file = std::fs::File::create(path).unwrap();
        let mut writer = BufWriter::new(file);
        conv_write(value, Format::Marshal, &mut writer).unwrap();
    }

    fn read_marshal(path: &Path) -> Value {
        let file = std::fs::File::open(path).unwrap();
        let mut reader = BufReader::new(file);
        let value: rpgtool_common::Value = conv_read(Format::Marshal, &mut reader).unwrap();
        serde_json::to_value(value).unwrap()
    }

    #[test]
    fn test_collect_translations_skips_whitespace_only_entries() {
        let project = Project {
            id: "test".into(),
            name: "Whitespace".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\Whitespace".into(),
            files: vec![TranslationFile {
                relative_path: "data/System.json".into(),
                entries: vec![
                    make_entry("System:gameTitle", "Test", "   ", EntryStatus::Draft),
                    make_entry("System:terms.basic.0", "Level", "Nivel", EntryStatus::Draft),
                ],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let (translations, total_entries) = collect_translations(&project);
        assert_eq!(total_entries, 2);
        assert_eq!(translations.len(), 1);
        assert_eq!(translations.get("System:terms.basic.0").unwrap(), "Nivel");
    }

    #[test]
    fn test_export_project_blocks_multiline_drift() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_block_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(
            data_dir.join("Map001.json"),
            r#"{"displayName":"","events":[null,{"id":1,"name":"EV001","pages":[{"list":[{"code":401,"parameters":["Line 1"]},{"code":401,"parameters":["Line 2"]},{"code":0,"parameters":[]}]}]}]}"#,
        )
        .unwrap();
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Block Drift".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![TranslationFile {
                relative_path: "data/Map001.json".into(),
                entries: vec![make_entry(
                    "Map001:1:0:text:0",
                    "Line 1\nLine 2",
                    "Linea 1",
                    EntryStatus::Draft,
                )],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_block_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let error = export_project(&project, &output_dir).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("Export blocked by translation integrity checks"));
        assert!(message.contains("changed line count"));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    fn write_minimal_mz_game(temp_dir: &Path) {
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rmmz_core.js"), "").unwrap();
        fs::write(
            data_dir.join("System.json"),
            r#"{"advanced":{"mainFontFilename":"mplus-1m-regular.woff","numberFontFilename":"mplus-2p-bold-sub.woff","fallbackFonts":"Verdana, sans-serif"}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("Map001.json"),
            r#"{"displayName":"","events":[null,{"id":1,"name":"EV001","pages":[{"list":[{"code":401,"parameters":["Original line."]},{"code":0,"parameters":[]}]}]}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_export_project_prefers_safe_system_fonts_for_extended_latin_text() {
        let temp_dir =
            std::env::temp_dir().join(format!("fairtl_export_font_{}", uuid::Uuid::new_v4()));
        write_minimal_mz_game(&temp_dir);

        let project = Project {
            id: "test".into(),
            name: "Font Safe".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mz,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![make_entry(
                        "Map001:1:0:text:0",
                        "Original line.",
                        "Las demás estarán aquí.",
                        EntryStatus::Draft,
                    )],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_font_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let system_json: Value =
            serde_json::from_str(&fs::read_to_string(output_dir.join("data/System.json")).unwrap())
                .unwrap();

        assert_eq!(system_json["advanced"]["mainFontFilename"], "");
        assert!(system_json["advanced"]["fallbackFonts"]
            .as_str()
            .unwrap()
            .contains("Meiryo"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("safe system fonts")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_keeps_webfont_for_ascii_only_translations() {
        let temp_dir =
            std::env::temp_dir().join(format!("fairtl_export_font_ascii_{}", uuid::Uuid::new_v4()));
        write_minimal_mz_game(&temp_dir);

        let project = Project {
            id: "test".into(),
            name: "Font Keep".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mz,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![make_entry(
                        "Map001:1:0:text:0",
                        "Original line.",
                        "Llegamos a casa.",
                        EntryStatus::Draft,
                    )],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_font_ascii_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let system_json: Value =
            serde_json::from_str(&fs::read_to_string(output_dir.join("data/System.json")).unwrap())
                .unwrap();

        assert_eq!(
            system_json["advanced"]["mainFontFilename"],
            "mplus-1m-regular.woff"
        );
        assert_eq!(
            system_json["advanced"]["fallbackFonts"],
            "Verdana, sans-serif"
        );
        assert!(!result
            .warnings
            .iter()
            .any(|warning| warning.contains("safe system fonts")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_apply_database() {
        let mut json: Value = serde_json::from_str(
            r#"[
                null,
                {"id": 1, "name": "Harold", "description": "A brave warrior."},
                {"id": 2, "name": "Therese", "description": "A healer."}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("Actors:name:1".into(), "Haroldo".into());
        translations.insert("Actors:description:2".into(), "Una sanadora.".into());

        let applied = apply_database("Actors", &mut json, &translations).unwrap();
        assert_eq!(applied, 2);
        assert_eq!(json[1]["name"], "Haroldo");
        assert_eq!(json[1]["description"], "A brave warrior."); // unchanged
        assert_eq!(json[2]["name"], "Therese"); // unchanged
        assert_eq!(json[2]["description"], "Una sanadora.");
    }

    #[test]
    fn test_apply_map_display_name() {
        let mut json: Value = serde_json::from_str(
            r#"{
                "displayName": "Forest of Trials",
                "events": []
            }"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("Map001:displayName".into(), "Bosque de Pruebas".into());

        let applied = apply_map("Map001", &mut json, &translations).unwrap();
        assert_eq!(applied, 1);
        assert_eq!(json["displayName"], "Bosque de Pruebas");
    }

    #[test]
    fn test_apply_event_commands_text() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 401, "parameters": ["Hello, traveler!"]},
                {"code": 401, "parameters": ["Welcome to our village."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "Map001:1:0:text:0".into(),
            "¡Hola, viajero!\nBienvenido a nuestro pueblo.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][0], "¡Hola, viajero!");
        assert_eq!(list[1]["parameters"][0], "Bienvenido a nuestro pueblo.");
    }

    #[test]
    fn test_apply_event_commands_choices() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 102, "parameters": [["Yes", "No"]]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("Map001:2:0:choice:0:0".into(), "Sí".into());
        translations.insert("Map001:2:0:choice:0:1".into(), "No".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:2:0", &translations);
        assert_eq!(applied, 2);
        assert_eq!(list[0]["parameters"][0][0], "Sí");
        assert_eq!(list[0]["parameters"][0][1], "No");
    }

    #[test]
    fn test_apply_system() {
        let mut json: Value = serde_json::from_str(
            r#"{
                "gameTitle": "My RPG Adventure",
                "terms": {
                    "basic": ["Level", "Lv"],
                    "messages": {
                        "actionFailure": "%1 has no effect!"
                    }
                }
            }"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("System:gameTitle".into(), "Mi Aventura RPG".into());
        translations.insert("System:terms.basic.0".into(), "Nivel".into());
        translations.insert(
            "System:terms.messages.actionFailure".into(),
            "¡%1 no tiene efecto!".into(),
        );

        let applied = apply_system(&mut json, &translations).unwrap();
        assert_eq!(applied, 3);
        assert_eq!(json["gameTitle"], "Mi Aventura RPG");
        assert_eq!(json["terms"]["basic"][0], "Nivel");
        assert_eq!(json["terms"]["basic"][1], "Lv"); // unchanged
        assert_eq!(
            json["terms"]["messages"]["actionFailure"],
            "¡%1 no tiene efecto!"
        );
    }

    #[test]
    fn test_apply_multiline_fewer_lines() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 401, "parameters": ["Line 1"]},
                {"code": 401, "parameters": ["Line 2"]},
                {"code": 401, "parameters": ["Line 3"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("test:text:0".into(), "Línea 1".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][0], "Línea 1");
        assert_eq!(list[1]["parameters"][0], ""); // emptied
        assert_eq!(list[2]["parameters"][0], ""); // emptied
    }

    #[test]
    fn test_apply_multiline_more_lines() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 401, "parameters": ["Line 1"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("test:text:0".into(), "Línea 1\nLínea 2\nLínea 3".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        // All excess lines joined into the single command
        assert_eq!(list[0]["parameters"][0], "Línea 1\nLínea 2\nLínea 3");
    }

    #[test]
    fn test_apply_change_name() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 320, "parameters": [1, "Harold"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("test:changeName:0".into(), "Haroldo".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][1], "Haroldo");
    }

    #[test]
    fn test_apply_change_nickname() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 324, "parameters": [1, "The Brave"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("test:changeNickname:0".into(), "El Valiente".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][1], "El Valiente");
    }

    #[test]
    fn test_apply_change_profile() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 325, "parameters": [1, "A brave warrior from the north."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "test:changeProfile:0".into(),
            "Un guerrero valiente del norte.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][1], "Un guerrero valiente del norte.");
    }

    #[test]
    fn test_apply_plugins_rewrites_stringified_menu_subcommands() {
        let mut json = parse_plugins_js(
            r#"var $plugins =
[
  {
    "name": "MenuSubCommand",
    "status": true,
    "description": "Menu plugin",
    "parameters": {
      "subCommands": "[\"{\\\"Name\\\":\\\"Character Introduction\\\",\\\"ParentName\\\":\\\"Character Introduction\\\"}\"]"
    }
  }
];"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "Plugins:0:parameters:subCommands:json:0:json:Name".into(),
            "Introducción de personajes".into(),
        );
        translations.insert(
            "Plugins:0:parameters:subCommands:json:0:json:ParentName".into(),
            "Introducción de personajes".into(),
        );

        let applied = apply_plugins(&mut json, &translations).unwrap();
        assert_eq!(applied, 2);

        let subcommands = json[0]["parameters"]["subCommands"].as_str().unwrap();
        assert!(subcommands.contains("Introducción de personajes"));
        assert!(!subcommands.contains("Character Introduction"));
    }

    #[test]
    fn test_apply_scroll_text() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 405, "parameters": ["In a faraway kingdom..."]},
                {"code": 405, "parameters": ["A hero was born."]},
                {"code": 405, "parameters": ["His name was Harold."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "test:scroll:0".into(),
            "En un reino lejano...\nNació un héroe.\nSu nombre era Haroldo.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "test", &translations);
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][0], "En un reino lejano...");
        assert_eq!(list[1]["parameters"][0], "Nació un héroe.");
        assert_eq!(list[2]["parameters"][0], "Su nombre era Haroldo.");
    }

    #[test]
    fn test_export_project_missing_game_dir() {
        let project = Project {
            id: "test".into(),
            name: "Legacy".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Xp,
            game_dir: "C:\\missing".into(),
            files: vec![],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let out = Path::new("C:\\tmp\\out");
        let err = export_project(&project, out).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Game directory not found"));
    }

    #[test]
    fn test_export_project_rejects_nested_output_dir() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_nested_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("Actors.json"), "[null]").unwrap();
        fs::create_dir_all(temp_dir.join("js")).unwrap();
        fs::write(temp_dir.join("js").join("rpg_core.js"), "").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Nested".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let nested_output = temp_dir.join("output");
        let err = export_project(&project, &nested_output).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("must be outside"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_export_project_roundtrip_vxace() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_vxace_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("Data");
        fs::create_dir_all(&data_dir).unwrap();

        write_marshal(
            &data_dir.join("Actors.rvdata2"),
            json!([
              {
                "$object": {
                  "class": "RPG::Actor",
                  "fields": {
                    "@id": 1,
                    "@name": { "$instance": { "value": "Hero", "fields": { "E": true } } },
                    "@description": { "$instance": { "value": "Brave warrior", "fields": { "E": true } } }
                  }
                }
              }
            ]),
        );
        write_marshal(
            &data_dir.join("System.rvdata2"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "Legacy Title", "fields": { "E": true } } },
                  "@terms": {
                    "$object": {
                      "class": "RPG::System::Terms",
                      "fields": {
                        "@basic": [
                          { "$instance": { "value": "Level", "fields": { "E": true } } }
                        ]
                      }
                    }
                  }
                }
              }
            }),
        );

        let project = Project {
            id: "test".into(),
            name: "Legacy".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::VxAce,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "Data/Actors.rvdata2".into(),
                    entries: vec![
                        make_entry("Actors:name:1", "Hero", "Heroe", EntryStatus::Draft),
                        make_entry(
                            "Actors:description:1",
                            "Brave warrior",
                            "",
                            EntryStatus::Untranslated,
                        ),
                    ],
                },
                TranslationFile {
                    relative_path: "Data/System.rvdata2".into(),
                    entries: vec![
                        make_entry(
                            "System:gameTitle",
                            "Legacy Title",
                            "Titulo Legacy",
                            EntryStatus::Reviewed,
                        ),
                        make_entry("System:terms.basic.0", "Level", "Nivel", EntryStatus::Final),
                    ],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_vxace_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();

        assert_eq!(result.files_exported, 2);
        assert_eq!(result.entries_applied, 3);
        assert_eq!(result.total_entries, 4);

        let actors_output = read_marshal(&output_dir.join("Data").join("Actors.rvdata2"));
        let actor_fields = object_fields(&actors_output[0]).unwrap();
        assert_eq!(
            string_from_legacy_value(actor_fields.get("@name").unwrap()).unwrap(),
            "Heroe"
        );
        assert_eq!(
            string_from_legacy_value(actor_fields.get("@description").unwrap()).unwrap(),
            "Brave warrior"
        );

        let system_output = read_marshal(&output_dir.join("Data").join("System.rvdata2"));
        let system_fields = object_fields(&system_output).unwrap();
        assert_eq!(
            string_from_legacy_value(system_fields.get("@game_title").unwrap()).unwrap(),
            "Titulo Legacy"
        );
        let terms = system_fields.get("@terms").unwrap();
        let terms_fields = object_fields(terms).unwrap();
        let basic_terms = terms_fields
            .get("@basic")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(string_from_legacy_value(&basic_terms[0]).unwrap(), "Nivel");

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_roundtrip_vx() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_vx_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("Data");
        fs::create_dir_all(&data_dir).unwrap();

        write_marshal(
            &data_dir.join("Actors.rvdata"),
            json!([
              {
                "$object": {
                  "class": "RPG::Actor",
                  "fields": {
                    "@id": 1,
                    "@name": { "$instance": { "value": "Ralph", "fields": { "E": true } } }
                  }
                }
              }
            ]),
        );
        write_marshal(
            &data_dir.join("System.rvdata"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "VX Title", "fields": { "E": true } } }
                }
              }
            }),
        );

        let project = Project {
            id: "test".into(),
            name: "LegacyVX".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Vx,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "Data/Actors.rvdata".into(),
                    entries: vec![make_entry(
                        "Actors:name:1",
                        "Ralph",
                        "Ralf",
                        EntryStatus::Draft,
                    )],
                },
                TranslationFile {
                    relative_path: "Data/System.rvdata".into(),
                    entries: vec![make_entry(
                        "System:gameTitle",
                        "VX Title",
                        "Titulo VX",
                        EntryStatus::Reviewed,
                    )],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_vx_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();

        assert_eq!(result.files_exported, 2);
        assert_eq!(result.entries_applied, 2);
        assert_eq!(result.total_entries, 2);

        let actors_output = read_marshal(&output_dir.join("Data").join("Actors.rvdata"));
        let actor_fields = object_fields(&actors_output[0]).unwrap();
        assert_eq!(
            string_from_legacy_value(actor_fields.get("@name").unwrap()).unwrap(),
            "Ralf"
        );

        let system_output = read_marshal(&output_dir.join("Data").join("System.rvdata"));
        let system_fields = object_fields(&system_output).unwrap();
        assert_eq!(
            string_from_legacy_value(system_fields.get("@game_title").unwrap()).unwrap(),
            "Titulo VX"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_roundtrip_xp() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_xp_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("Data");
        fs::create_dir_all(&data_dir).unwrap();

        write_marshal(
            &data_dir.join("Actors.rxdata"),
            json!([
              {
                "$object": {
                  "class": "RPG::Actor",
                  "fields": {
                    "@id": 1,
                    "@name": { "$instance": { "value": "Aluxes", "fields": { "E": true } } }
                  }
                }
              }
            ]),
        );
        write_marshal(
            &data_dir.join("System.rxdata"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "XP Title", "fields": { "E": true } } }
                }
              }
            }),
        );

        let project = Project {
            id: "test".into(),
            name: "LegacyXP".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Xp,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "Data/Actors.rxdata".into(),
                    entries: vec![make_entry(
                        "Actors:name:1",
                        "Aluxes",
                        "Alex",
                        EntryStatus::Draft,
                    )],
                },
                TranslationFile {
                    relative_path: "Data/System.rxdata".into(),
                    entries: vec![make_entry(
                        "System:gameTitle",
                        "XP Title",
                        "Titulo XP",
                        EntryStatus::Reviewed,
                    )],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_xp_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();

        assert_eq!(result.files_exported, 2);
        assert_eq!(result.entries_applied, 2);
        assert_eq!(result.total_entries, 2);

        let actors_output = read_marshal(&output_dir.join("Data").join("Actors.rxdata"));
        let actor_fields = object_fields(&actors_output[0]).unwrap();
        assert_eq!(
            string_from_legacy_value(actor_fields.get("@name").unwrap()).unwrap(),
            "Alex"
        );

        let system_output = read_marshal(&output_dir.join("Data").join("System.rxdata"));
        let system_fields = object_fields(&system_output).unwrap();
        assert_eq!(
            string_from_legacy_value(system_fields.get("@game_title").unwrap()).unwrap(),
            "Titulo XP"
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_roundtrip() {
        // Create a temp game directory with a simple Actors.json
        let temp_dir =
            std::env::temp_dir().join(format!("fairtl_export_test_{}", uuid::Uuid::new_v4()));
        let data_dir = temp_dir.join("data");
        fs::create_dir_all(&data_dir).unwrap();

        // Write a minimal Actors.json
        let actors_json = r#"[null,{"id":1,"name":"Harold","description":"A brave warrior."}]"#;
        fs::write(data_dir.join("Actors.json"), actors_json).unwrap();

        // Engine detection file (MV)
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        fs::write(temp_dir.join("Game.exe"), "stub").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![TranslationFile {
                relative_path: "data/Actors.json".into(),
                entries: vec![
                    make_entry("Actors:name:1", "Harold", "Haroldo", EntryStatus::Draft),
                    make_entry(
                        "Actors:description:1",
                        "A brave warrior.",
                        "",
                        EntryStatus::Untranslated,
                    ),
                ],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();

        assert_eq!(result.files_exported, 1);
        assert_eq!(result.entries_applied, 1); // Only "Haroldo" (Draft status)
        assert_eq!(result.total_entries, 2);

        // Verify the output file
        let output_content = fs::read_to_string(output_dir.join("data/Actors.json")).unwrap();
        let output_json: Value = serde_json::from_str(&output_content).unwrap();
        assert_eq!(output_json[1]["name"], "Haroldo");
        assert_eq!(output_json[1]["description"], "A brave warrior."); // unchanged
        assert!(output_dir.join("js").join("rpg_core.js").exists());
        assert!(output_dir.join("Game.exe").exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }
}

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::AppError;
use crate::models::{Engine, EntryStatus, Project};
use crate::parser;
use crate::parser::rpgmaker::{
    classify_file, classify_relative_path, parse_mv_mz_game, parse_plugins_js, render_plugins_js,
    FileKind, CODE_CHANGE_NAME, CODE_CHANGE_NICKNAME, CODE_CHANGE_PROFILE, CODE_CONTROL_VARIABLES,
    CODE_SCROLL_TEXT, CODE_SHOW_CHOICES, CODE_SHOW_TEXT, CODE_SHOW_TEXT_HEADER, DB_TEXT_FIELDS,
    SYSTEM_TEXT_FIELDS,
};
use crate::services::message_wrapping::{
    join_overflow_message_lines, wrap_mv_mz_message_translation,
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
const CODE_SHOW_PICTURE: i64 = 231;
const CODE_CHANGE_ACTOR_IMAGES: i64 = 322;
const CODE_SCRIPT: i64 = 355;
const CODE_SCRIPT_CONTINUATION: i64 = 655;
const MOVEMENT_CODE_CHANGE_IMAGE: i64 = 41;
const CODE_CHANGE_BATTLE_BGM: i64 = 132;
const CODE_CHANGE_VICTORY_ME: i64 = 133;
const CODE_CHANGE_DEFEAT_ME: i64 = 139;
const CODE_PLAY_BGM: i64 = 241;
const CODE_PLAY_BGS: i64 = 245;
const CODE_PLAY_ME: i64 = 249;
const CODE_PLAY_SE: i64 = 250;
const MAX_SOURCE_DRIFT_EXAMPLES: usize = 5;

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
struct UnityExporter;

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

impl EngineExporter for UnityExporter {
    fn export_project(
        &self,
        project: &Project,
        output_dir: &Path,
    ) -> Result<ExportResult, AppError> {
        export_unity_project(project, output_dir)
    }
}

static MV_MZ_EXPORTER: MvMzExporter = MvMzExporter;
static VXACE_EXPORTER: VxAceExporter = VxAceExporter;
static VX_EXPORTER: VxExporter = VxExporter;
static XP_EXPORTER: XpExporter = XpExporter;
static UNITY_EXPORTER: UnityExporter = UnityExporter;

fn exporter_for_engine(engine: &Engine) -> Result<&'static dyn EngineExporter, AppError> {
    match engine {
        Engine::Mv | Engine::Mz => Ok(&MV_MZ_EXPORTER),
        Engine::VxAce => Ok(&VXACE_EXPORTER),
        Engine::Vx => Ok(&VX_EXPORTER),
        Engine::Xp => Ok(&XP_EXPORTER),
        Engine::Unity => Ok(&UNITY_EXPORTER),
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
    let game_dir = validate_game_dir(project)?;
    validate_project_source_files(project, &game_dir)?;
    validate_project_source_snapshot(project, &game_dir)?;
    let exporter = exporter_for_engine(&project.engine)?;
    exporter.export_project(project, output_dir)
}

fn export_mv_mz_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let game_dir = PathBuf::from(&project.game_dir);
    let validation = translation_integrity::validate_project_for_export(project);
    if validation.has_errors() {
        return Err(AppError::Project(
            translation_integrity::summarize_export_errors(&validation.errors),
        ));
    }
    prepare_export_destination(&game_dir, output_dir)?;
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
                FileKind::Html => Value::String(content),
                _ => serde_json::from_str(&content)?,
            };

            let applied =
                apply_translations(&stem, &kind, &mut json, &translations, &project.target_lang)?;
            entries_applied += applied;

            if prefer_system_fonts && stem == "System" {
                font_fallback_applied |= patch_mv_mz_system_font_preferences(&mut json);
            }

            let output_content = match kind {
                FileKind::Plugins => render_plugins_js(&json)?,
                FileKind::Html => json
                    .as_str()
                    .ok_or_else(|| AppError::Parse(format!("{stem}: expected HTML content")))?
                    .to_string(),
                _ => serde_json::to_string_pretty(&json)?,
            };
            std::fs::write(&copied_path, output_content)?;

            files_exported += 1;
        }

        if prefer_system_fonts {
            font_fallback_applied |= patch_exported_mv_mz_system_font_file(output_dir)?;
        }

        let normalized_asset_refs = normalize_exported_mv_mz_asset_references(output_dir)?;

        if font_fallback_applied {
            export_warnings.push(
                "Export forced safe system fonts for translated Latin text; the original webfont stayed in the copy, but dialogue/menu rendering now prefers a glyph-complete fallback stack.".into(),
            );
        }
        if normalized_asset_refs > 0 {
            export_warnings.push(format!(
                "Export restored {normalized_asset_refs} RPG Maker asset references to the exact copied file names."
            ));
        }

        if let Err(error) =
            append_visible_source_text_warnings(project, output_dir, &mut export_warnings)
        {
            export_warnings.push(format!(
                "Post-export visible text QA could not run: {error}"
            ));
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

fn export_unity_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let game_dir = PathBuf::from(&project.game_dir);
    let validation = translation_integrity::validate_project_for_export(project);
    if validation.has_errors() {
        return Err(AppError::Project(
            translation_integrity::summarize_export_errors(&validation.errors),
        ));
    }

    prepare_export_destination(&game_dir, output_dir)?;
    if let Err(error) = copy_dir_recursive(&game_dir, output_dir) {
        let _ = std::fs::remove_dir_all(output_dir);
        return Err(error);
    }

    let export_result = (|| {
        let inventory = parser::unity::inventory_unity_game(output_dir)?;
        if project.files.len() != 1
            || project.files[0].relative_path != inventory.localization_file_relative_path
        {
            return Err(AppError::Project(format!(
                "Unity Loose JSON v0 project must contain only '{}'. Re-import the game before exporting.",
                inventory.localization_file_relative_path
            )));
        }

        let project_file = &project.files[0];
        let copied_path = output_dir.join(&project_file.relative_path);
        let bytes = std::fs::read(&copied_path)?;
        let has_bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        let json_bytes = if has_bom { &bytes[3..] } else { &bytes[..] };
        let json_text = std::str::from_utf8(json_bytes).map_err(|error| {
            AppError::Parse(format!(
                "Unity localization JSON '{}' must be UTF-8: {error}",
                project_file.relative_path
            ))
        })?;
        let mut json: Value = serde_json::from_str(json_text)?;
        let object = json.as_object_mut().ok_or_else(|| {
            AppError::Parse(format!(
                "Unity localization JSON '{}' must be a flat string-to-string object.",
                project_file.relative_path
            ))
        })?;

        let mut entries_applied = 0usize;
        let mut expected_readback = HashMap::new();
        for entry in &project_file.entries {
            if entry.status == EntryStatus::Untranslated || entry.translation.trim().is_empty() {
                continue;
            }

            let key = decode_unity_json_id(&entry.id)?;
            let value = object.get_mut(&key).ok_or_else(|| {
                AppError::Project(format!(
                    "Unity source entry '{}' is missing from '{}'. Re-import the game before exporting.",
                    entry.id, project_file.relative_path
                ))
            })?;
            let current = value.as_str().ok_or_else(|| {
                AppError::Project(format!(
                    "Unity source entry '{}' is no longer a string. Re-import the game before exporting.",
                    entry.id
                ))
            })?;
            if current != entry.original {
                return Err(AppError::Project(format!(
                    "Unity source entry '{}' changed after export preflight. Re-import the game before exporting.",
                    entry.id
                )));
            }

            *value = Value::String(entry.translation.clone());
            expected_readback.insert(entry.id.clone(), entry.translation.clone());
            entries_applied += 1;
        }

        let rendered = serde_json::to_vec_pretty(&json)?;
        let mut output_bytes = Vec::with_capacity(rendered.len() + usize::from(has_bom) * 3);
        if has_bom {
            output_bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        output_bytes.extend_from_slice(&rendered);
        std::fs::write(&copied_path, output_bytes)?;

        let readback = parser::unity::parse_unity_game(output_dir)?;
        let readback_entries = readback
            .iter()
            .flat_map(|file| file.entries.iter())
            .map(|entry| (entry.id.as_str(), entry.original.as_str()))
            .collect::<HashMap<_, _>>();
        for (id, translation) in &expected_readback {
            if readback_entries.get(id.as_str()).copied() != Some(translation.as_str()) {
                return Err(AppError::Project(format!(
                    "Unity export verification failed for entry '{id}'. The incomplete output was removed."
                )));
            }
        }

        Ok(ExportResult {
            files_exported: 1,
            entries_applied,
            total_entries: project_file.entries.len(),
            warnings: validation.warnings.clone(),
        })
    })();

    if export_result.is_err() {
        let _ = std::fs::remove_dir_all(output_dir);
    }

    export_result
}

fn decode_unity_json_id(id: &str) -> Result<String, AppError> {
    let encoded = id.strip_prefix("UnityJson:/").ok_or_else(|| {
        AppError::Project(format!(
            "Unsupported Unity entry ID '{id}'. Re-import the game before exporting."
        ))
    })?;
    if encoded.is_empty() || encoded.contains('/') {
        return Err(AppError::Project(format!(
            "Unsupported Unity JSON Pointer '{id}'. Unity Loose JSON v0 accepts one flat key segment."
        )));
    }

    let mut decoded = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();
    while let Some(ch) = chars.next() {
        if ch != '~' {
            decoded.push(ch);
            continue;
        }
        match chars.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => {
                return Err(AppError::Project(format!(
                    "Invalid Unity JSON Pointer escape in entry ID '{id}'. Re-import the game before exporting."
                )))
            }
        }
    }

    Ok(decoded)
}

fn export_legacy_project(project: &Project, output_dir: &Path) -> Result<ExportResult, AppError> {
    let game_dir = PathBuf::from(&project.game_dir);
    let validation = translation_integrity::validate_project_for_export(project);
    if validation.has_errors() {
        return Err(AppError::Project(
            translation_integrity::summarize_export_errors(&validation.errors),
        ));
    }
    prepare_export_destination(&game_dir, output_dir)?;
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
        reject_symlink_or_reparse(output_dir)?;
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

fn validate_project_source_snapshot(project: &Project, game_dir: &Path) -> Result<(), AppError> {
    let current = parser::parse_game(game_dir)?;
    if current.engine != project.engine {
        return Err(AppError::Project(format!(
            "Game engine changed since import: project uses {:?}, current source is {:?}. Re-import the game before exporting.",
            project.engine, current.engine
        )));
    }

    let current_paths = current
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<HashSet<_>>();
    let current_entries = current
        .files
        .iter()
        .flat_map(|file| {
            file.entries.iter().map(|entry| {
                (
                    (file.relative_path.as_str(), entry.id.as_str()),
                    entry.original.as_str(),
                )
            })
        })
        .collect::<HashMap<_, _>>();

    let mut drift_count = 0usize;
    let mut examples = Vec::new();
    for file in &project.files {
        for entry in &file.entries {
            if entry.status == EntryStatus::Untranslated || entry.translation.trim().is_empty() {
                continue;
            }

            let reason = if !current_paths.contains(file.relative_path.as_str()) {
                Some("file or translatable content missing")
            } else {
                match current_entries.get(&(file.relative_path.as_str(), entry.id.as_str())) {
                    None => Some("entry missing"),
                    Some(original) if *original != entry.original => Some("original text changed"),
                    Some(_) => None,
                }
            };

            if let Some(reason) = reason {
                drift_count += 1;
                if examples.len() < MAX_SOURCE_DRIFT_EXAMPLES {
                    examples.push(format!("{} :: {} ({reason})", file.relative_path, entry.id));
                }
            }
        }
    }

    if drift_count == 0 {
        return Ok(());
    }

    let remaining = drift_count.saturating_sub(examples.len());
    let suffix = if remaining == 0 {
        String::new()
    } else {
        format!("; and {remaining} more")
    };
    Err(AppError::Project(format!(
        "Source game changed since import: {drift_count} translated entries no longer match. Re-import the game before exporting. {}{suffix}",
        examples.join("; ")
    )))
}

fn copy_dir_recursive(source_dir: &Path, target_dir: &Path) -> Result<(), AppError> {
    reject_symlink_or_reparse(source_dir)?;
    let source_metadata = std::fs::symlink_metadata(source_dir)?;
    if !source_metadata.is_dir() {
        return Err(AppError::Project(format!(
            "Export source is not a directory: {}",
            source_dir.display()
        )));
    }

    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target_dir.join(entry.file_name());
        reject_symlink_or_reparse(&source_path)?;
        let metadata = std::fs::symlink_metadata(&source_path)?;

        if metadata.is_dir() {
            std::fs::create_dir_all(&target_path)?;
            copy_dir_recursive(&source_path, &target_path)?;
        } else if metadata.is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &target_path)?;
        } else {
            return Err(AppError::Project(format!(
                "Export refuses non-regular source path '{}'.",
                source_path.display()
            )));
        }
    }

    Ok(())
}

fn reject_symlink_or_reparse(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
        return Err(AppError::Project(format!(
            "Export refuses symbolic link or reparse point '{}'; copy real files into the source tree first.",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[derive(Debug, Clone, Default)]
struct AssetNameIndex {
    exact: HashSet<String>,
    normalized: HashMap<String, Option<String>>,
}

impl AssetNameIndex {
    fn from_dir(dir: &Path) -> Result<Self, AppError> {
        let mut index = Self::default();
        collect_asset_names(dir, &mut index)?;
        Ok(index)
    }

    fn insert(&mut self, name: String) {
        self.exact.insert(name.clone());
        let key = normalized_asset_name_key(&name);
        self.normalized
            .entry(key)
            .and_modify(|existing| {
                if existing.as_deref() != Some(name.as_str()) {
                    *existing = None;
                }
            })
            .or_insert(Some(name));
    }

    fn exact_or_normalized(&self, name: &str) -> Option<String> {
        if self.exact.contains(name) {
            return Some(name.to_string());
        }
        self.normalized
            .get(&normalized_asset_name_key(name))
            .and_then(Clone::clone)
    }
}

#[derive(Debug, Clone)]
struct MvMzAssetIndexes {
    pictures: AssetNameIndex,
    characters: AssetNameIndex,
    faces: AssetNameIndex,
    sv_actors: AssetNameIndex,
    enemies: AssetNameIndex,
    tilesets: AssetNameIndex,
    parallaxes: AssetNameIndex,
    titles1: AssetNameIndex,
    titles2: AssetNameIndex,
    bgm: AssetNameIndex,
    bgs: AssetNameIndex,
    me: AssetNameIndex,
    se: AssetNameIndex,
}

impl MvMzAssetIndexes {
    fn from_export_dir(output_dir: &Path) -> Result<Self, AppError> {
        let img_dir = output_dir.join("img");
        let audio_dir = output_dir.join("audio");
        Ok(Self {
            pictures: AssetNameIndex::from_dir(&img_dir.join("pictures"))?,
            characters: AssetNameIndex::from_dir(&img_dir.join("characters"))?,
            faces: AssetNameIndex::from_dir(&img_dir.join("faces"))?,
            sv_actors: AssetNameIndex::from_dir(&img_dir.join("sv_actors"))?,
            enemies: AssetNameIndex::from_dir(&img_dir.join("enemies"))?,
            tilesets: AssetNameIndex::from_dir(&img_dir.join("tilesets"))?,
            parallaxes: AssetNameIndex::from_dir(&img_dir.join("parallaxes"))?,
            titles1: AssetNameIndex::from_dir(&img_dir.join("titles1"))?,
            titles2: AssetNameIndex::from_dir(&img_dir.join("titles2"))?,
            bgm: AssetNameIndex::from_dir(&audio_dir.join("bgm"))?,
            bgs: AssetNameIndex::from_dir(&audio_dir.join("bgs"))?,
            me: AssetNameIndex::from_dir(&audio_dir.join("me"))?,
            se: AssetNameIndex::from_dir(&audio_dir.join("se"))?,
        })
    }
}

fn collect_asset_names(dir: &Path, index: &mut AssetNameIndex) -> Result<(), AppError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_asset_names(&path, index)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "rpgmvp" | "ogg" | "m4a" | "rpgmvo" | "webm" | "mp4"
        ) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        index.insert(stem.to_string());
    }

    Ok(())
}

fn normalized_asset_name_key(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_space = false;
    for ch in value.trim().chars() {
        let mapped = match ch {
            '０'..='９' => char::from(b'0' + (ch as u32 - '０' as u32) as u8),
            'Ａ'..='Ｚ' => char::from(b'a' + (ch as u32 - 'Ａ' as u32) as u8),
            'ａ'..='ｚ' => char::from(b'a' + (ch as u32 - 'ａ' as u32) as u8),
            '（' => '(',
            '）' => ')',
            _ if ch.is_ascii_alphabetic() => ch.to_ascii_lowercase(),
            _ => ch,
        };
        if mapped.is_whitespace() {
            if !previous_was_space {
                output.push(' ');
                previous_was_space = true;
            }
        } else {
            output.push(mapped);
            previous_was_space = false;
        }
    }
    output.trim().to_string()
}

fn normalize_exported_mv_mz_asset_references(output_dir: &Path) -> Result<usize, AppError> {
    let data_dir = output_dir.join("data");
    if !data_dir.is_dir() {
        return Ok(0);
    }

    let indexes = MvMzAssetIndexes::from_export_dir(output_dir)?;
    let mut refs_fixed = 0usize;
    for entry in std::fs::read_dir(&data_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let mut json: Value = serde_json::from_str(&content)?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        let before = json.clone();
        refs_fixed += normalize_asset_references_for_file(stem, &mut json, &indexes);
        if json != before {
            std::fs::write(&path, serde_json::to_string_pretty(&json)?)?;
        }
    }

    Ok(refs_fixed)
}

fn normalize_asset_references_for_file(
    stem: &str,
    json: &mut Value,
    indexes: &MvMzAssetIndexes,
) -> usize {
    let mut refs_fixed = 0usize;
    refs_fixed += normalize_nested_note_picture_references(json, &indexes.pictures);
    match stem {
        "System" => refs_fixed += normalize_system_asset_references(json, indexes),
        "Actors" => {
            refs_fixed +=
                normalize_database_asset_field(json, "characterName", &indexes.characters);
            refs_fixed += normalize_database_asset_field(json, "faceName", &indexes.faces);
            refs_fixed += normalize_database_asset_field(json, "battlerName", &indexes.sv_actors);
        }
        "Enemies" => {
            refs_fixed += normalize_database_asset_field(json, "battlerName", &indexes.enemies)
        }
        "Tilesets" => refs_fixed += normalize_tileset_asset_references(json, &indexes.tilesets),
        "MapInfos" => {}
        "CommonEvents" | "Troops" => {
            refs_fixed += normalize_nested_event_asset_references(json, indexes);
        }
        stem if stem.starts_with("Map") => {
            refs_fixed += normalize_map_asset_references(json, indexes);
        }
        _ => {}
    }

    refs_fixed
}

fn normalize_system_asset_references(json: &mut Value, indexes: &MvMzAssetIndexes) -> usize {
    let mut refs_fixed = 0usize;
    refs_fixed += normalize_string_field(json, "title1Name", &indexes.titles1);
    refs_fixed += normalize_string_field(json, "title2Name", &indexes.titles2);
    refs_fixed += normalize_audio_object_field(json, "titleBgm", &indexes.bgm);
    refs_fixed += normalize_audio_object_field(json, "battleBgm", &indexes.bgm);
    refs_fixed += normalize_audio_object_field(json, "victoryMe", &indexes.me);
    refs_fixed += normalize_audio_object_field(json, "defeatMe", &indexes.me);
    refs_fixed += normalize_audio_object_field(json, "gameoverMe", &indexes.me);
    if let Some(sounds) = json.get_mut("sounds").and_then(Value::as_array_mut) {
        for sound in sounds {
            refs_fixed += normalize_audio_object(sound, &indexes.se);
        }
    }
    for vehicle_key in ["boat", "ship", "airship"] {
        if let Some(vehicle) = json.get_mut(vehicle_key) {
            refs_fixed += normalize_string_field(vehicle, "characterName", &indexes.characters);
        }
    }
    refs_fixed
}

fn normalize_database_asset_field(json: &mut Value, field: &str, index: &AssetNameIndex) -> usize {
    let Some(rows) = json.as_array_mut() else {
        return 0;
    };
    rows.iter_mut()
        .map(|row| normalize_string_field(row, field, index))
        .sum()
}

fn normalize_tileset_asset_references(json: &mut Value, index: &AssetNameIndex) -> usize {
    let Some(rows) = json.as_array_mut() else {
        return 0;
    };
    let mut refs_fixed = 0usize;
    for row in rows {
        let Some(names) = row.get_mut("tilesetNames").and_then(Value::as_array_mut) else {
            continue;
        };
        for name in names {
            refs_fixed += normalize_string_value(name, index);
        }
    }
    refs_fixed
}

fn normalize_map_asset_references(json: &mut Value, indexes: &MvMzAssetIndexes) -> usize {
    let mut refs_fixed = 0usize;
    refs_fixed += normalize_string_field(json, "parallaxName", &indexes.parallaxes);
    refs_fixed += normalize_audio_object_field(json, "bgm", &indexes.bgm);
    refs_fixed += normalize_audio_object_field(json, "bgs", &indexes.bgs);
    refs_fixed += normalize_nested_event_asset_references(json, indexes);
    refs_fixed
}

fn normalize_nested_event_asset_references(value: &mut Value, indexes: &MvMzAssetIndexes) -> usize {
    let mut refs_fixed = 0usize;
    match value {
        Value::Object(map) => {
            if let Some(image) = map.get_mut("image") {
                refs_fixed += normalize_string_field(image, "characterName", &indexes.characters);
            }
            if map.get("code").and_then(Value::as_i64) == Some(CODE_SHOW_PICTURE) {
                refs_fixed += normalize_command_parameter_string(map, 1, &indexes.pictures);
            }
            if map.get("code").and_then(Value::as_i64) == Some(CODE_CHANGE_ACTOR_IMAGES) {
                refs_fixed += normalize_command_parameter_string(map, 1, &indexes.characters);
                refs_fixed += normalize_command_parameter_string(map, 3, &indexes.faces);
                refs_fixed += normalize_command_parameter_string(map, 5, &indexes.sv_actors);
            }
            if matches!(
                map.get("code").and_then(Value::as_i64),
                Some(CODE_SCRIPT | CODE_SCRIPT_CONTINUATION)
            ) {
                refs_fixed += normalize_script_asset_references(map, indexes);
            }
            if map.get("code").and_then(Value::as_i64) == Some(MOVEMENT_CODE_CHANGE_IMAGE) {
                refs_fixed += normalize_command_parameter_string(map, 0, &indexes.characters);
            }
            match map.get("code").and_then(Value::as_i64) {
                Some(CODE_CHANGE_BATTLE_BGM | CODE_PLAY_BGM) => {
                    refs_fixed += normalize_command_parameter_audio(map, &indexes.bgm);
                }
                Some(CODE_PLAY_BGS) => {
                    refs_fixed += normalize_command_parameter_audio(map, &indexes.bgs);
                }
                Some(CODE_CHANGE_VICTORY_ME | CODE_CHANGE_DEFEAT_ME | CODE_PLAY_ME) => {
                    refs_fixed += normalize_command_parameter_audio(map, &indexes.me);
                }
                Some(CODE_PLAY_SE) => {
                    refs_fixed += normalize_command_parameter_audio(map, &indexes.se);
                }
                _ => {}
            }
            for child in map.values_mut() {
                refs_fixed += normalize_nested_event_asset_references(child, indexes);
            }
        }
        Value::Array(values) => {
            for child in values {
                refs_fixed += normalize_nested_event_asset_references(child, indexes);
            }
        }
        _ => {}
    }
    refs_fixed
}

fn normalize_command_parameter_string(
    command: &mut Map<String, Value>,
    index: usize,
    asset_index: &AssetNameIndex,
) -> usize {
    command
        .get_mut("parameters")
        .and_then(Value::as_array_mut)
        .and_then(|parameters| parameters.get_mut(index))
        .map(|value| normalize_string_value(value, asset_index))
        .unwrap_or(0)
}

fn normalize_command_parameter_audio(
    command: &mut Map<String, Value>,
    asset_index: &AssetNameIndex,
) -> usize {
    command
        .get_mut("parameters")
        .and_then(Value::as_array_mut)
        .and_then(|parameters| parameters.get_mut(0))
        .map(|value| normalize_audio_object(value, asset_index))
        .unwrap_or(0)
}

fn normalize_string_field(value: &mut Value, field: &str, index: &AssetNameIndex) -> usize {
    value
        .get_mut(field)
        .map(|value| normalize_string_value(value, index))
        .unwrap_or(0)
}

fn normalize_audio_object_field(value: &mut Value, field: &str, index: &AssetNameIndex) -> usize {
    value
        .get_mut(field)
        .map(|value| normalize_audio_object(value, index))
        .unwrap_or(0)
}

fn normalize_audio_object(value: &mut Value, index: &AssetNameIndex) -> usize {
    normalize_string_field(value, "name", index)
}

fn normalize_string_value(value: &mut Value, index: &AssetNameIndex) -> usize {
    let Some(current) = value.as_str() else {
        return 0;
    };
    let Some(canonical) = index.exact_or_normalized(current) else {
        return 0;
    };
    if canonical == current {
        return 0;
    }
    *value = Value::String(canonical);
    1
}

fn normalize_script_asset_references(
    command: &mut Map<String, Value>,
    indexes: &MvMzAssetIndexes,
) -> usize {
    command
        .get_mut("parameters")
        .and_then(Value::as_array_mut)
        .and_then(|parameters| parameters.get_mut(0))
        .map(|value| normalize_script_asset_reference_value(value, indexes))
        .unwrap_or(0)
}

fn normalize_script_asset_reference_value(value: &mut Value, indexes: &MvMzAssetIndexes) -> usize {
    let Some(script) = value.as_str() else {
        return 0;
    };
    let (updated, refs_fixed) = normalize_script_asset_reference_text(script, indexes);
    if refs_fixed == 0 {
        return 0;
    }
    *value = Value::String(updated);
    refs_fixed
}

fn normalize_script_asset_reference_text(
    script: &str,
    indexes: &MvMzAssetIndexes,
) -> (String, usize) {
    let mut updated = script.to_string();
    let mut refs_fixed = 0usize;
    for (method, index) in [
        ("setCharacterImage", &indexes.characters),
        ("setImage", &indexes.characters),
        ("setFaceImage", &indexes.faces),
        ("setBattlerImage", &indexes.sv_actors),
    ] {
        let (next, fixed) = normalize_js_method_first_string_arg(&updated, method, index);
        updated = next;
        refs_fixed += fixed;
    }
    (updated, refs_fixed)
}

fn normalize_js_method_first_string_arg(
    script: &str,
    method: &str,
    index: &AssetNameIndex,
) -> (String, usize) {
    let marker = format!(".{method}");
    let bytes = script.as_bytes();
    let mut output = String::with_capacity(script.len());
    let mut cursor = 0usize;
    let mut refs_fixed = 0usize;

    while let Some(relative_start) = script[cursor..].find(&marker) {
        let method_start = cursor + relative_start;
        let after_method = method_start + marker.len();
        let mut pos = skip_ascii_whitespace(bytes, after_method);
        if bytes.get(pos) != Some(&b'(') {
            output.push_str(&script[cursor..after_method]);
            cursor = after_method;
            continue;
        }
        pos = skip_ascii_whitespace(bytes, pos + 1);
        let Some(&quote) = bytes.get(pos) else {
            output.push_str(&script[cursor..after_method]);
            cursor = after_method;
            continue;
        };
        if quote != b'"' && quote != b'\'' {
            output.push_str(&script[cursor..after_method]);
            cursor = after_method;
            continue;
        }

        let name_start = pos + 1;
        let Some(name_end) = find_js_string_literal_end(bytes, name_start, quote) else {
            output.push_str(&script[cursor..after_method]);
            cursor = after_method;
            continue;
        };
        let raw_name = &script[name_start..name_end];
        if raw_name.contains('\\') {
            output.push_str(&script[cursor..name_end]);
            cursor = name_end;
            continue;
        }
        let Some(canonical) = index.exact_or_normalized(raw_name) else {
            output.push_str(&script[cursor..name_end]);
            cursor = name_end;
            continue;
        };
        if canonical == raw_name {
            output.push_str(&script[cursor..name_end]);
            cursor = name_end;
            continue;
        }

        output.push_str(&script[cursor..name_start]);
        output.push_str(&escape_js_string_literal(&canonical, quote));
        cursor = name_end;
        refs_fixed += 1;
    }

    output.push_str(&script[cursor..]);
    (output, refs_fixed)
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        index += 1;
    }
    index
}

fn find_js_string_literal_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut index = start;
    let mut escaped = false;
    while let Some(&byte) = bytes.get(index) {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn escape_js_string_literal(value: &str, quote: u8) -> String {
    let quote = quote as char;
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\\' || ch == quote {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn normalize_nested_note_picture_references(value: &mut Value, index: &AssetNameIndex) -> usize {
    match value {
        Value::Object(map) => {
            let mut refs_fixed = 0usize;
            if let Some(note) = map.get_mut("note") {
                refs_fixed += normalize_note_picture_reference_value(note, index);
            }
            for child in map.values_mut() {
                refs_fixed += normalize_nested_note_picture_references(child, index);
            }
            refs_fixed
        }
        Value::Array(values) => values
            .iter_mut()
            .map(|child| normalize_nested_note_picture_references(child, index))
            .sum(),
        _ => 0,
    }
}

fn normalize_note_picture_reference_value(value: &mut Value, index: &AssetNameIndex) -> usize {
    let Some(note) = value.as_str() else {
        return 0;
    };
    let (updated, refs_fixed) = normalize_note_picture_reference_text(note, index);
    if refs_fixed == 0 {
        return 0;
    }
    *value = Value::String(updated);
    refs_fixed
}

fn normalize_note_picture_reference_text(note: &str, index: &AssetNameIndex) -> (String, usize) {
    let mut output = String::with_capacity(note.len());
    let mut cursor = 0usize;
    let mut refs_fixed = 0usize;

    while let Some(relative_tag_start) = note[cursor..].find('<') {
        let tag_start = cursor + relative_tag_start;
        let Some(relative_tag_end) = note[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_tag_end;
        let tag_body = &note[tag_start + 1..tag_end];
        let Some(relative_colon) = tag_body.find(':') else {
            output.push_str(&note[cursor..tag_end + 1]);
            cursor = tag_end + 1;
            continue;
        };
        let tag_name = &tag_body[..relative_colon];
        if !is_picture_note_tag(tag_name) {
            output.push_str(&note[cursor..tag_end + 1]);
            cursor = tag_end + 1;
            continue;
        }

        let value_start = tag_start + 1 + relative_colon + 1;
        let value_end = note[value_start..tag_end]
            .find(',')
            .map(|relative_comma| value_start + relative_comma)
            .unwrap_or(tag_end);
        let (trimmed_start, trimmed_end) =
            trim_ascii_whitespace_range(note, value_start, value_end);
        let current = &note[trimmed_start..trimmed_end];
        let Some(canonical) = index.exact_or_normalized(current) else {
            output.push_str(&note[cursor..tag_end + 1]);
            cursor = tag_end + 1;
            continue;
        };
        if canonical == current {
            output.push_str(&note[cursor..tag_end + 1]);
            cursor = tag_end + 1;
            continue;
        }

        output.push_str(&note[cursor..trimmed_start]);
        output.push_str(&canonical);
        output.push_str(&note[trimmed_end..tag_end + 1]);
        cursor = tag_end + 1;
        refs_fixed += 1;
    }

    output.push_str(&note[cursor..]);
    (output, refs_fixed)
}

fn is_picture_note_tag(tag_name: &str) -> bool {
    tag_name.contains("ピクチャ") || tag_name.to_ascii_lowercase().contains("picture")
}

fn trim_ascii_whitespace_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let mut trimmed_start = start;
    let mut trimmed_end = end;
    while matches!(bytes.get(trimmed_start), Some(b' ' | b'\t' | b'\r' | b'\n'))
        && trimmed_start < trimmed_end
    {
        trimmed_start += 1;
    }
    while trimmed_end > trimmed_start
        && matches!(
            bytes.get(trimmed_end - 1),
            Some(b' ' | b'\t' | b'\r' | b'\n')
        )
    {
        trimmed_end -= 1;
    }
    (trimmed_start, trimmed_end)
}

fn is_same_or_nested(base_dir: &Path, candidate_dir: &Path) -> Result<bool, AppError> {
    let normalized_base = resolve_existing_path_prefix(base_dir)?;
    let normalized_candidate = resolve_existing_path_prefix(candidate_dir)?;

    Ok(normalized_candidate == normalized_base
        || normalized_candidate.starts_with(&normalized_base))
}

fn resolve_existing_path_prefix(path: &Path) -> Result<PathBuf, AppError> {
    let absolute = normalize_path(path)?;
    let mut existing = absolute.clone();
    let mut missing_segments = Vec::new();

    while !existing.exists() {
        let segment = existing.file_name().ok_or_else(|| {
            AppError::Project(format!(
                "Could not resolve export path '{}'.",
                path.display()
            ))
        })?;
        missing_segments.push(segment.to_os_string());
        if !existing.pop() {
            return Err(AppError::Project(format!(
                "Could not resolve export path '{}'.",
                path.display()
            )));
        }
    }

    let mut resolved = std::fs::canonicalize(existing)?;
    while let Some(segment) = missing_segments.pop() {
        resolved.push(segment);
    }
    Ok(normalize_lexically(&resolved))
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

fn append_visible_source_text_warnings(
    project: &Project,
    output_dir: &Path,
    warnings: &mut Vec<String>,
) -> Result<(), AppError> {
    if !is_japanese_source_project(project) {
        return Ok(());
    }

    let parsed_files = parse_mv_mz_game(output_dir)?;
    let mut leftovers = Vec::new();
    for file in parsed_files {
        for entry in file.entries {
            if contains_japanese_script(&entry.original) {
                leftovers.push(format!(
                    "{} :: {} = {}",
                    file.relative_path,
                    entry.id,
                    truncate_for_warning(&entry.original, 64)
                ));
            }
        }
    }

    if leftovers.is_empty() {
        return Ok(());
    }

    const MAX_EXAMPLES: usize = 5;
    let shown = leftovers
        .iter()
        .take(MAX_EXAMPLES)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    let remaining = leftovers.len().saturating_sub(MAX_EXAMPLES);
    let suffix = if remaining > 0 {
        format!("; and {remaining} more")
    } else {
        String::new()
    };
    warnings.push(format!(
        "Post-export visible text QA found {} extracted entries that still contain Japanese script: {}{}",
        leftovers.len(),
        shown,
        suffix
    ));

    Ok(())
}

fn is_japanese_source_project(project: &Project) -> bool {
    let source = project.source_lang.trim().to_ascii_lowercase();
    source == "ja" || source == "jp" || source.starts_with("ja-") || source.contains("japanese")
}

fn contains_japanese_script(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
        )
    })
}

fn truncate_for_warning(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut truncated = normalized.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
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
        FileKind::Html => Ok(0),
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
        return decode_legacy_string(bytes);
    }

    None
}

fn decode_legacy_string(bytes: &[Value]) -> Option<String> {
    let decoded = bytes
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|byte| *byte <= u8::MAX as u64)
                .map(|byte| byte as u8)
        })
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(decoded).ok()
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
        if decode_legacy_string(bytes).is_none() {
            return false;
        }
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
    target_lang: &str,
) -> Result<usize, AppError> {
    let choice_prompt = choice_prompt_guard_text(target_lang);
    match kind {
        FileKind::Database => apply_database(stem, json, translations),
        FileKind::Map => apply_map(stem, json, translations, choice_prompt),
        FileKind::CommonEvents => apply_common_events(json, translations, choice_prompt),
        FileKind::Troops => apply_troops(json, translations, choice_prompt),
        FileKind::System => apply_system(json, translations),
        FileKind::Plugins => apply_plugins(json, translations),
        FileKind::Html => apply_index_html(stem, json, translations),
        FileKind::Ignored => Ok(0),
    }
}

fn choice_prompt_guard_text(target_lang: &str) -> &'static str {
    let lang = target_lang.to_ascii_lowercase();
    if lang.starts_with("es") || lang.contains("spanish") || lang.contains("español") {
        "Elige una opción."
    } else if lang.starts_with("ja") || lang.starts_with("jp") || lang.contains("japanese") {
        "選択してください。"
    } else {
        "Choose an option."
    }
}

fn apply_index_html(
    stem: &str,
    json: &mut Value,
    translations: &HashMap<String, String>,
) -> Result<usize, AppError> {
    let Some(translation) = translations.get("index.html:title") else {
        return Ok(0);
    };

    let content = json
        .as_str()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected HTML content string")))?;
    let Some(updated) = replace_html_title(content, translation) else {
        return Ok(0);
    };
    if updated == content {
        return Ok(0);
    }

    *json = Value::String(updated);
    Ok(1)
}

fn replace_html_title(content: &str, translation: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?is)(<title[^>]*>)(.*?)(</title>)").ok()?;
    if !re.is_match(content) {
        return None;
    }
    let escaped = escape_html_text(translation);
    Some(
        re.replace(content, |captures: &regex::Captures| {
            format!("{}{}{}", &captures[1], escaped, &captures[3])
        })
        .to_string(),
    )
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
            let parameter_prefix = format!("Plugins:{plugin_index}:parameters:");
            for (entry_id, translation) in translations {
                let Some(parameter_name) = entry_id.strip_prefix(&parameter_prefix) else {
                    continue;
                };
                if parameter_name.contains(':') || parameters.contains_key(parameter_name) {
                    continue;
                }
                parameters.insert(
                    parameter_name.to_string(),
                    Value::String(translation.clone()),
                );
                applied += 1;
            }

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
    choice_prompt_guard: &str,
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
                        applied += apply_event_commands(
                            list,
                            &id_prefix,
                            translations,
                            choice_prompt_guard,
                        );
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
    choice_prompt_guard: &str,
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
            applied += apply_event_commands(list, &id_prefix, translations, choice_prompt_guard);
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
    choice_prompt_guard: &str,
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
                    applied +=
                        apply_event_commands(list, &id_prefix, translations, choice_prompt_guard);
                }
            }
        }
    }

    Ok(applied)
}

// ---------------------------------------------------------------------------
// System (gameTitle + visible vocabulary + terms)
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

    for field in SYSTEM_TEXT_FIELDS {
        if let Some(value) = obj.get_mut(*field) {
            applied += apply_terms_recursive(value, &format!("System:{field}"), translations);
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
    _choice_prompt_guard: &str,
) -> usize {
    let mut applied = 0;
    let mut i = 0;

    while i < list.len() {
        let code = list[i].get("code").and_then(|v| v.as_i64()).unwrap_or(0);

        match code {
            CODE_SHOW_TEXT_HEADER => {
                let entry_id = format!("{id_prefix}:speakerName:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    let has_speaker_name = list[i]
                        .get("parameters")
                        .and_then(Value::as_array)
                        .and_then(|parameters| parameters.get(4))
                        .and_then(Value::as_str)
                        .is_some();
                    if has_speaker_name {
                        set_parameter(list, i, 4, translation);
                        applied += 1;
                    }
                }
            }
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
                    let followed_by_choice = list
                        .get(i)
                        .and_then(|command| command.get("code"))
                        .and_then(Value::as_i64)
                        == Some(CODE_SHOW_CHOICES);
                    let prepared = wrap_mv_mz_message_translation(
                        translation,
                        followed_by_choice,
                        line_indices.len(),
                    );
                    apply_multiline_text(list, &line_indices, &prepared);
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
            CODE_CONTROL_VARIABLES => {
                let entry_id = format!("{id_prefix}:variableText:{i}");
                if let Some(translation) = translations.get(&entry_id) {
                    if let Ok(quoted) = serde_json::to_string(translation) {
                        set_parameter(list, i, 4, &quoted);
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply a multi-line translation to consecutive 401/405 commands.
///
/// - Lines are distributed 1:1 across the commands.
/// - If the translation has fewer lines, excess commands get empty strings.
/// - If the translation has more lines, overflow is compacted into the last
///   existing command so scene/event command structure is left unchanged.
fn apply_multiline_text(list: &mut Vec<Value>, line_indices: &[usize], translation: &str) {
    let lines: Vec<&str> = translation.split('\n').collect();
    let num_commands = line_indices.len();
    if num_commands == 0 {
        return;
    }

    for (j, &cmd_idx) in line_indices.iter().enumerate() {
        let text = if j + 1 == num_commands && lines.len() > num_commands {
            join_overflow_message_lines(&lines[j..])
        } else if j < lines.len() {
            lines[j].to_string()
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

    fn write_mv_engine_marker(game_dir: &Path) {
        fs::create_dir_all(game_dir.join("js")).unwrap();
        fs::write(game_dir.join("js").join("rpg_core.js"), "").unwrap();
    }

    fn write_unity_mono_fixture(root: &Path, localization_bytes: &[u8]) -> PathBuf {
        let data_dir = root.join("Sample_Data");
        let localization_path = data_dir
            .join("StreamingAssets")
            .join("Localization")
            .join("strings.json");
        fs::create_dir_all(data_dir.join("Managed")).unwrap();
        fs::create_dir_all(localization_path.parent().unwrap()).unwrap();
        fs::write(root.join("UnityPlayer.dll"), []).unwrap();
        fs::write(data_dir.join("globalgamemanagers"), []).unwrap();
        fs::write(data_dir.join("Managed/Assembly-CSharp.dll"), []).unwrap();
        fs::write(&localization_path, localization_bytes).unwrap();
        localization_path
    }

    fn unity_project_from_source(root: &Path) -> Project {
        let parsed = crate::parser::parse_game(root).unwrap();
        assert_eq!(parsed.engine, Engine::Unity);
        Project {
            id: "unity-test".into(),
            name: "Unity Loose JSON".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: parsed.engine,
            game_dir: root.to_string_lossy().into(),
            files: parsed.files,
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn snapshot_file_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, files: &mut Vec<(String, Vec<u8>)>) {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else if path.is_file() {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    files.push((relative, fs::read(path).unwrap()));
                }
            }
        }

        let mut files = Vec::new();
        visit(root, root, &mut files);
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    #[test]
    fn test_invalid_utf8_legacy_string_is_not_decoded_or_overwritten() {
        let mut value = json!({ "$string": [72, 255] });
        let original = value.clone();

        assert_eq!(string_from_legacy_value(&value), None);
        assert!(!set_string_value(&mut value, "Translated"));
        assert_eq!(value, original);
    }

    #[test]
    fn test_valid_utf8_legacy_string_remains_translatable() {
        let mut value = json!({ "$string": [72, 105] });

        assert_eq!(string_from_legacy_value(&value).as_deref(), Some("Hi"));
        assert!(set_string_value(&mut value, "Sí"));
        assert_eq!(string_from_legacy_value(&value).as_deref(), Some("Sí"));
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
    fn test_export_project_warns_on_multiline_drift() {
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
        let result = export_project(&project, &output_dir).unwrap();

        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("cantidad de líneas")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_applies_quoted_variable_text() {
        let temp_dir =
            std::env::temp_dir().join(format!("fairtl_export_variable_{}", uuid::Uuid::new_v4()));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        fs::write(
            data_dir.join("CommonEvents.json"),
            r#"[null,{"id":1,"list":[{"code":122,"parameters":[10,10,0,4,"\"Waste Plant\""]},{"code":0,"parameters":[]}]}]"#,
        )
        .unwrap();

        let project = Project {
            id: "test".into(),
            name: "Variable Text".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![TranslationFile {
                relative_path: "data/CommonEvents.json".into(),
                entries: vec![make_entry(
                    "CommonEvents:1:variableText:0",
                    "Waste Plant",
                    "Planta de tratamiento de residuos",
                    EntryStatus::Draft,
                )],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_variable_out_{}",
            uuid::Uuid::new_v4()
        ));
        export_project(&project, &output_dir).unwrap();
        let exported: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("CommonEvents.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            exported[1]["list"][0]["parameters"][4],
            Value::String("\"Planta de tratamiento de residuos\"".into())
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_warns_about_visible_japanese_after_export() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_visible_ja_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            js_dir.join("plugins.js"),
            r#"var $plugins =
[
  {
    "name": "BB_CustomSaveWindow",
    "status": true,
    "description": "",
    "parameters": {
      "Item1title": "操作キャラ"
    }
  }
];"#,
        )
        .unwrap();

        let files = parse_mv_mz_game(&temp_dir).unwrap();
        let project = Project {
            id: "test".into(),
            name: "Visible QA".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files,
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_visible_ja_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let warning = result
            .warnings
            .iter()
            .find(|warning| warning.contains("Post-export visible text QA found"))
            .expect("export should warn about untranslated visible Japanese");

        assert!(warning.contains("js/plugins.js :: Plugins:0:parameters:Item1title"));

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

        let applied = apply_map("Map001", &mut json, &translations, "Choose an option.").unwrap();
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
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Choose an option.");
        assert_eq!(applied, 1);
        assert_eq!(
            list[0]["parameters"][0],
            "¡Hola, viajero! Bienvenido a nuestro pueblo."
        );
        assert_eq!(list[1]["parameters"][0], "");
    }

    #[test]
    fn test_apply_event_commands_wraps_long_show_text_lines() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["", 0, 2, 2]},
                {"code": 401, "parameters": ["\\C[27]\\N[1]\\C[0]"]},
                {"code": 401, "parameters": ["Original"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "Map001:1:0:text:1".into(),
            "\\C[27]\\N[1]\\C[0]\nUna chica de secundaria de pelo corto vuelve caminando tranquilamente de la escuela.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Choose an option.");
        assert_eq!(applied, 1);
        assert_eq!(list[1]["parameters"][0], "\\C[27]\\N[1]\\C[0]");
        assert_eq!(
            list[2]["parameters"][0],
            "Una chica de secundaria de pelo corto vuelve caminando tranquilamente de la escuela."
        );
        assert_eq!(list[3]["code"], 0);
    }

    #[test]
    fn test_apply_event_commands_does_not_insert_oversized_show_text_pages() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["", 0, 2, 2]},
                {"code": 401, "parameters": ["Line 1"]},
                {"code": 401, "parameters": ["Line 2"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "Map001:1:0:text:1".into(),
            "Esta es una explicación larga que debería ocupar varias líneas completas de la caja de diálogo antes de pasar a una segunda página del mismo mensaje porque no cabe en cuatro líneas. La frase continúa con más detalles para comprobar que el exportador divide el texto por ancho real y no por saltos previos del proveedor.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Choose an option.");
        assert_eq!(applied, 1);
        let message_count = list
            .iter()
            .filter(|command| command.get("code").and_then(Value::as_i64) == Some(101))
            .count();
        let lines = list
            .iter()
            .filter(|command| command.get("code").and_then(Value::as_i64) == Some(CODE_SHOW_TEXT))
            .filter_map(|command| {
                command
                    .get("parameters")
                    .and_then(Value::as_array)
                    .and_then(|parameters| parameters.first())
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>();
        assert_eq!(message_count, 1);
        assert_eq!(lines.len(), 2);
        assert_eq!(list[3]["code"], 0);
        assert!(lines[0].contains("Esta es una explicación larga"));
        assert!(lines[1].contains("proveedor"));
    }

    #[test]
    fn test_apply_event_commands_compacts_blank_lines_before_choices() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["", 0, 2, 2]},
                {"code": 401, "parameters": ["Original"]},
                {"code": 102, "parameters": [["Yes", "No"]]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert(
            "Map001:1:0:text:1".into(),
            "Selecciona la cantidad de vello púbico.\n\n※Estado actual: Normal.".into(),
        );

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Elige una opción.");
        assert_eq!(applied, 1);
        assert_eq!(
            list[1]["parameters"][0],
            "Selecciona la cantidad de vello púbico. ※Estado actual: Normal."
        );
        assert_eq!(list[2]["code"], 102);
    }

    #[test]
    fn test_apply_event_commands_does_not_add_final_prompt_before_split_choice() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["", 0, 2, 2]},
                {"code": 401, "parameters": ["Primera explicación."]},
                {"code": 101, "parameters": ["", 0, 2, 2]},
                {"code": 401, "parameters": ["Segunda explicación."]},
                {"code": 102, "parameters": [["Sí", "No"]]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let translations = HashMap::new();
        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Elige una opción.");
        assert_eq!(applied, 0);
        assert_eq!(list.len(), 6);
        assert_eq!(list[4]["code"], 102);
        assert!(list.iter().all(|command| {
            command
                .get("parameters")
                .and_then(Value::as_array)
                .and_then(|parameters| parameters.first())
                .and_then(Value::as_str)
                != Some("Elige una opción.")
        }));
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
        let applied = apply_event_commands(list, "Map001:2:0", &translations, "Choose an option.");
        assert_eq!(applied, 2);
        assert_eq!(list[0]["parameters"][0][0], "Sí");
        assert_eq!(list[0]["parameters"][0][1], "No");
    }

    #[test]
    fn test_apply_event_commands_mz_speaker_name_keeps_header_and_text_group() {
        let mut json: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["Actor1", 2, 0, 2, "Alice"]},
                {"code": 401, "parameters": ["First line"]},
                {"code": 401, "parameters": ["Second line"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("Map001:1:0:speakerName:0".into(), "Alicia".into());

        let list = json.as_array_mut().unwrap();
        let applied = apply_event_commands(list, "Map001:1:0", &translations, "Choose an option.");

        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"], json!(["Actor1", 2, 0, 2, "Alicia"]));
        assert_eq!(
            list.iter()
                .map(|command| command["code"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![101, 401, 401, 0]
        );
        assert_eq!(list[1]["parameters"][0], "First line");
        assert_eq!(list[2]["parameters"][0], "Second line");
    }

    #[test]
    fn test_apply_system() {
        let mut json: Value = serde_json::from_str(
            r#"{
                "gameTitle": "My RPG Adventure",
                "currencyUnit": "G",
                "elements": ["", "Fire"],
                "skillTypes": ["", "Magic"],
                "weaponTypes": ["", "Sword"],
                "armorTypes": ["", "Shield"],
                "equipTypes": ["", "Weapon"],
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
        translations.insert("System:currencyUnit".into(), "Oro".into());
        translations.insert("System:elements.1".into(), "Fuego".into());
        translations.insert("System:skillTypes.1".into(), "Magia".into());
        translations.insert("System:weaponTypes.1".into(), "Espada".into());
        translations.insert("System:armorTypes.1".into(), "Escudo".into());
        translations.insert("System:equipTypes.1".into(), "Arma".into());
        translations.insert("System:terms.basic.0".into(), "Nivel".into());
        translations.insert(
            "System:terms.messages.actionFailure".into(),
            "¡%1 no tiene efecto!".into(),
        );

        let applied = apply_system(&mut json, &translations).unwrap();
        assert_eq!(applied, 9);
        assert_eq!(json["gameTitle"], "Mi Aventura RPG");
        assert_eq!(json["currencyUnit"], "Oro");
        assert_eq!(json["elements"], json!(["", "Fuego"]));
        assert_eq!(json["skillTypes"], json!(["", "Magia"]));
        assert_eq!(json["weaponTypes"], json!(["", "Espada"]));
        assert_eq!(json["armorTypes"], json!(["", "Escudo"]));
        assert_eq!(json["equipTypes"], json!(["", "Arma"]));
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
        assert_eq!(applied, 1);
        assert_eq!(list[0]["parameters"][0], "Línea 1 Línea 2 Línea 3");
        assert_eq!(list[1]["code"], 0);
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
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
    fn test_apply_plugins_inserts_missing_direct_parameter() {
        let mut json = parse_plugins_js(
            r#"var $plugins =
[
  {
    "name": "gamEnd",
    "status": true,
    "description": "",
    "parameters": {}
  }
];"#,
        )
        .unwrap();

        let mut translations = HashMap::new();
        translations.insert("Plugins:0:parameters:endName".into(), "Salir".into());

        let applied = apply_plugins(&mut json, &translations).unwrap();
        assert_eq!(applied, 1);
        assert_eq!(json[0]["parameters"]["endName"], "Salir");
    }

    #[test]
    fn test_export_project_only_updates_structured_plugin_parameters() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_plugin_source_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        let plugins_dir = js_dir.join("plugins");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&plugins_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            js_dir.join("plugins.js"),
            r#"var $plugins =
[
  {
    "name": "gamEnd",
    "status": true,
    "description": "",
    "parameters": {}
  }
];"#,
        )
        .unwrap();
        let plugin_source = "/*:\n * @param endName\n * @default ゲーム終了\n */\nvar parameters = PluginManager.parameters('gameEnd');\nvar EndName = String(parameters['endName'] || 'ゲーム終了');\nvar internalMarker = 'ゲーム終了';\n";
        fs::write(plugins_dir.join("gamEnd.js"), plugin_source).unwrap();

        let project = Project {
            id: "test".into(),
            name: "Plugin Source".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "js/plugins.js".into(),
                    entries: vec![make_entry(
                        "Plugins:0:parameters:endName",
                        "ゲーム終了",
                        "Salir",
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
            "fairtl_export_plugin_source_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        assert_eq!(result.entries_applied, 1);

        let plugins_output = fs::read_to_string(output_dir.join("js").join("plugins.js")).unwrap();
        assert!(plugins_output.contains(r#""endName": "Salir""#));

        let source_output =
            fs::read(output_dir.join("js").join("plugins").join("gamEnd.js")).unwrap();
        assert_eq!(source_output, plugin_source.as_bytes());

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_keeps_prompt_based_plugin_source_unchanged() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_prompt_plugin_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        let plugins_dir = js_dir.join("plugins");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&plugins_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            js_dir.join("plugins.js"),
            r#"var $plugins =
[
  {
    "name": "Torigoya_InputNamePrompt",
    "status": true,
    "description": "",
    "parameters": {}
  }
];"#,
        )
        .unwrap();
        let plugin_source = r#"    InputNamePrompt.runCommand = function (args) {
        var actor = $gameActors.actor(~~args.shift());
        var max = Number(args.shift());
        if (isNaN(max)) max = InputNamePrompt.settings.maxLength;
        var message = args.join(' ') || InputNamePrompt.settings.getMessage();
        message += '\n' + InputNamePrompt.settings.getMaximumMessage().replace(/%1/g, '' + max);
        var name = window.prompt(message, actor.name());
        if (name) {
            name = name.trim();
            if (name.length > 0) {
                actor.setName(name.substr(0, max));
            }
        }
    };"#;
        fs::write(
            plugins_dir.join("Torigoya_InputNamePrompt.js"),
            plugin_source,
        )
        .unwrap();

        let project = Project {
            id: "test".into(),
            name: "Prompt Plugin".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![TranslationFile {
                relative_path: "data/System.json".into(),
                entries: vec![],
            }],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_prompt_plugin_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        export_project(&project, &output_dir).unwrap();

        let source_output = fs::read(
            output_dir
                .join("js")
                .join("plugins")
                .join("Torigoya_InputNamePrompt.js"),
        )
        .unwrap();
        assert_eq!(source_output, plugin_source.as_bytes());

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_normalizes_asset_references_to_copied_file_names() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_asset_refs_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let pictures_dir = temp_dir.join("img").join("pictures");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&pictures_dir).unwrap();
        write_mv_engine_marker(&temp_dir);
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("CommonEvents.json"),
            r#"[null,{"id":1,"list":[{"code":231,"parameters":[1,"保乃花 立ち絵 眉１",0,0,0,0,100,100,255,0]},{"code":0,"parameters":[]}]}]"#,
        )
        .unwrap();
        fs::write(pictures_dir.join("保乃花　立ち絵　眉１.rpgmvp"), "asset").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Asset Refs".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/CommonEvents.json".into(),
                    entries: vec![],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_asset_refs_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let common_events: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("CommonEvents.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            common_events[1]["list"][0]["parameters"][1],
            "保乃花　立ち絵　眉１"
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("restored 1 RPG Maker asset references")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_normalizes_script_actor_image_references() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_script_asset_refs_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let characters_dir = temp_dir.join("img").join("characters");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&characters_dir).unwrap();
        write_mv_engine_marker(&temp_dir);
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("CommonEvents.json"),
            r#"[null,{"id":1,"list":[{"code":355,"parameters":["$gameActors.actor(5).setCharacterImage(\"主人公 男１\", 0)"]},{"code":0,"parameters":[]}]}]"#,
        )
        .unwrap();
        fs::write(characters_dir.join("主人公　男１.rpgmvp"), "asset").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Script Asset Refs".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/CommonEvents.json".into(),
                    entries: vec![],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_script_asset_refs_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let common_events: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("CommonEvents.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            common_events[1]["list"][0]["parameters"][0],
            "$gameActors.actor(5).setCharacterImage(\"主人公　男１\", 0)"
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("restored 1 RPG Maker asset references")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_normalizes_movement_route_image_references() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_movement_asset_refs_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let characters_dir = temp_dir.join("img").join("characters");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&characters_dir).unwrap();
        write_mv_engine_marker(&temp_dir);
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("Map001.json"),
            r#"{"displayName":"","events":[null,{"id":1,"pages":[{"image":{"characterName":"","characterIndex":0},"list":[{"code":205,"parameters":[1,{"list":[{"code":41,"parameters":["主人公 女１",0]},{"code":0}],"repeat":false,"skippable":false,"wait":true}]},{"code":505,"parameters":[{"code":41,"parameters":["主人公 女１",0]}]},{"code":0,"parameters":[]}]}]}]}"#,
        )
        .unwrap();
        fs::write(characters_dir.join("主人公　女１.rpgmvp"), "asset").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Movement Asset Refs".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_movement_asset_refs_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let map: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("Map001.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            map["events"][1]["pages"][0]["list"][0]["parameters"][1]["list"][0]["parameters"][0],
            "主人公　女１"
        );
        assert_eq!(
            map["events"][1]["pages"][0]["list"][1]["parameters"][0]["parameters"][0],
            "主人公　女１"
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("restored 2 RPG Maker asset references")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_normalizes_note_picture_references() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_note_asset_refs_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let pictures_dir = temp_dir.join("img").join("pictures");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&pictures_dir).unwrap();
        write_mv_engine_marker(&temp_dir);
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("Items.json"),
            r#"[null,{"id":1,"name":"Gallery","note":"<SG追加1ピクチャ:サムネ １,8,60>"}]"#,
        )
        .unwrap();
        fs::write(pictures_dir.join("サムネ　１.rpgmvp"), "asset").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Note Asset Refs".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/Items.json".into(),
                    entries: vec![],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_note_asset_refs_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let items: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("Items.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(items[1]["note"], "<SG追加1ピクチャ:サムネ　１,8,60>");
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("restored 1 RPG Maker asset references")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_normalizes_audio_references() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_audio_refs_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let bgm_dir = temp_dir.join("audio").join("bgm");
        let bgs_dir = temp_dir.join("audio").join("bgs");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&bgm_dir).unwrap();
        fs::create_dir_all(&bgs_dir).unwrap();
        write_mv_engine_marker(&temp_dir);
        fs::write(
            data_dir.join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{},"titleBgm":{"name":"テーマ(通常)","volume":90,"pitch":100,"pan":0}}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("Map001.json"),
            r#"{"displayName":"","bgm":{"name":"","volume":90,"pitch":100,"pan":0},"bgs":{"name":"ピストン(中速10秒)","volume":90,"pitch":80,"pan":0},"events":[]}"#,
        )
        .unwrap();
        fs::write(
            data_dir.join("CommonEvents.json"),
            r#"[null,{"id":1,"list":[{"code":245,"parameters":[{"name":"ピストン(中速10秒)","volume":90,"pitch":80,"pan":0}]},{"code":0,"parameters":[]}]}]"#,
        )
        .unwrap();
        fs::write(bgm_dir.join("テーマ（通常）.rpgmvo"), "asset").unwrap();
        fs::write(bgs_dir.join("ピストン（中速10秒）.rpgmvo"), "asset").unwrap();

        let project = Project {
            id: "test".into(),
            name: "Audio Refs".into(),
            source_lang: "ja".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: temp_dir.to_string_lossy().into(),
            files: vec![
                TranslationFile {
                    relative_path: "data/System.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/Map001.json".into(),
                    entries: vec![],
                },
                TranslationFile {
                    relative_path: "data/CommonEvents.json".into(),
                    entries: vec![],
                },
            ],
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_audio_refs_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();
        let system: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("System.json")).unwrap(),
        )
        .unwrap();
        let map: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("Map001.json")).unwrap(),
        )
        .unwrap();
        let common_events: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("CommonEvents.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(system["titleBgm"]["name"], "テーマ（通常）");
        assert_eq!(map["bgs"]["name"], "ピストン（中速10秒）");
        assert_eq!(
            common_events[1]["list"][0]["parameters"][0]["name"],
            "ピストン（中速10秒）"
        );
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("restored 3 RPG Maker asset references")));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_apply_index_html_rewrites_title() {
        let mut json = Value::String(
            r#"<!doctype html><html><head><title>露出に目覚めた人妻は寝取られる ver1.02</title></head></html>"#
                .into(),
        );
        let mut translations = HashMap::new();
        translations.insert(
            "index.html:title".into(),
            "La esposa & el secreto ver1.02".into(),
        );

        let applied = apply_index_html("index", &mut json, &translations).unwrap();
        assert_eq!(applied, 1);
        let output = json.as_str().unwrap();
        assert!(output.contains("<title>La esposa &amp; el secreto ver1.02</title>"));
        assert!(!output.contains("露出に目覚めた人妻は寝取られる"));
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
        let applied = apply_event_commands(list, "test", &translations, "Choose an option.");
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

    #[cfg(windows)]
    #[test]
    fn test_nested_output_detection_is_case_insensitive_on_windows() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_export_path_case_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(&root).unwrap();
        let differently_cased = PathBuf::from(root.to_string_lossy().to_uppercase()).join("output");

        assert!(is_same_or_nested(&root, &differently_cased).unwrap());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_export_project_rejects_source_drift_before_creating_output() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_source_drift_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();

        let mut events = vec![Value::Null];
        for id in 1..=6 {
            events.push(json!({
                "id": id,
                "name": format!("EV{id:03}"),
                "pages": [{"list": [
                    {"code": 401, "parameters": [format!("Original {id}")]},
                    {"code": 0, "parameters": []}
                ]}]
            }));
        }
        fs::write(
            data_dir.join("Map001.json"),
            serde_json::to_string_pretty(&json!({"displayName": "", "events": events})).unwrap(),
        )
        .unwrap();

        let parsed = crate::parser::parse_game(&temp_dir).unwrap();
        let mut project = Project {
            id: "test".into(),
            name: "Source Drift".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: parsed.engine,
            game_dir: temp_dir.to_string_lossy().into(),
            files: parsed.files,
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        for entry in project
            .files
            .iter_mut()
            .flat_map(|file| file.entries.iter_mut())
        {
            entry.translation = format!("Translated {}", entry.id);
            entry.status = EntryStatus::Reviewed;
        }

        let mut changed_events = vec![Value::Null];
        for id in 1..=6 {
            let list = if id == 1 {
                vec![
                    json!({"code": 401, "parameters": ["Inserted before"]}),
                    json!({"code": 401, "parameters": ["Externally changed 1"]}),
                    json!({"code": 0, "parameters": []}),
                ]
            } else {
                vec![
                    json!({"code": 401, "parameters": [format!("Externally changed {id}")]}),
                    json!({"code": 0, "parameters": []}),
                ]
            };
            changed_events.push(json!({
                "id": id,
                "name": format!("EV{id:03}"),
                "pages": [{"list": list}]
            }));
        }
        let changed_source =
            serde_json::to_string_pretty(&json!({"displayName": "", "events": changed_events}))
                .unwrap();
        fs::write(data_dir.join("Map001.json"), &changed_source).unwrap();

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_source_drift_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let error = export_project(&project, &output_dir).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("Source game changed since import"));
        assert!(message.contains("6 translated entries"));
        assert!(message.contains("Map001:1:0:text:0"));
        assert!(message.contains("and 1 more"));
        assert!(!message.contains("Map001:6:0:text:0"));
        assert!(!output_dir.exists());
        assert_eq!(
            fs::read_to_string(data_dir.join("Map001.json")).unwrap(),
            changed_source
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_rejects_engine_change_before_creating_output() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_engine_drift_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rpg_core.js"), "").unwrap();
        let map_source = r#"{"displayName":"","events":[null,{"id":1,"pages":[{"list":[{"code":401,"parameters":["Original"]},{"code":0,"parameters":[]}]}]}]}"#;
        fs::write(data_dir.join("Map001.json"), map_source).unwrap();

        let parsed = crate::parser::parse_game(&temp_dir).unwrap();
        let mut project = Project {
            id: "test".into(),
            name: "Engine Drift".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: parsed.engine,
            game_dir: temp_dir.to_string_lossy().into(),
            files: parsed.files,
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let entry = &mut project.files[0].entries[0];
        entry.translation = "Traducido".into();
        entry.status = EntryStatus::Reviewed;

        fs::remove_file(js_dir.join("rpg_core.js")).unwrap();
        fs::write(js_dir.join("rmmz_core.js"), "").unwrap();
        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_engine_drift_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let error = export_project(&project, &output_dir).unwrap_err();

        assert!(error
            .to_string()
            .contains("Game engine changed since import"));
        assert!(!output_dir.exists());
        assert_eq!(
            fs::read_to_string(data_dir.join("Map001.json")).unwrap(),
            map_source
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_export_project_roundtrips_mz_system_vocabulary_and_speaker_name() {
        let temp_dir = std::env::temp_dir().join(format!(
            "fairtl_export_mz_visible_text_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let data_dir = temp_dir.join("data");
        let js_dir = temp_dir.join("js");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&js_dir).unwrap();
        fs::write(js_dir.join("rmmz_core.js"), "").unwrap();

        let system_source = r#"{
            "currencyUnit":"G",
            "elements":["","Fire"],
            "skillTypes":["","Magic"],
            "weaponTypes":["","Sword"],
            "armorTypes":["","Shield"],
            "equipTypes":["","Weapon"],
            "terms":{}
        }"#;
        let map_source = r#"{
            "displayName":"",
            "events":[null,{"id":1,"name":"EV001","pages":[{"list":[
                {"code":101,"parameters":["Actor1",2,0,2,"Alice"]},
                {"code":401,"parameters":["First line"]},
                {"code":401,"parameters":["Second line"]},
                {"code":0,"parameters":[]}
            ]}]}]
        }"#;
        fs::write(data_dir.join("System.json"), system_source).unwrap();
        fs::write(data_dir.join("Map001.json"), map_source).unwrap();

        let mut files = parse_mv_mz_game(&temp_dir).unwrap();
        let translations = HashMap::from([
            ("System:currencyUnit", "Oro"),
            ("System:elements.1", "Fuego"),
            ("System:skillTypes.1", "Magia"),
            ("System:weaponTypes.1", "Espada"),
            ("System:armorTypes.1", "Escudo"),
            ("System:equipTypes.1", "Arma"),
            ("Map001:1:0:speakerName:0", "Alicia"),
        ]);
        let mut selected = 0;
        for entry in files.iter_mut().flat_map(|file| file.entries.iter_mut()) {
            if let Some(translation) = translations.get(entry.id.as_str()) {
                entry.translation = (*translation).to_string();
                entry.status = EntryStatus::Reviewed;
                selected += 1;
            }
        }
        assert_eq!(selected, translations.len());

        let project = Project {
            id: "test".into(),
            name: "MZ Visible Text".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mz,
            game_dir: temp_dir.to_string_lossy().into(),
            files,
            glossary: vec![],
            style_guide: vec![],
            created_at: String::new(),
            updated_at: String::new(),
        };
        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_mz_visible_text_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));

        let result = export_project(&project, &output_dir).unwrap();
        assert_eq!(result.entries_applied, translations.len());

        let readback = parse_mv_mz_game(&output_dir)
            .unwrap()
            .into_iter()
            .flat_map(|file| file.entries)
            .map(|entry| (entry.id, entry.original))
            .collect::<HashMap<_, _>>();
        for (id, translation) in &translations {
            assert_eq!(readback.get(*id).map(String::as_str), Some(*translation));
        }
        assert!(!readback.contains_key("System:elements.0"));

        let exported_map: Value = serde_json::from_str(
            &fs::read_to_string(output_dir.join("data").join("Map001.json")).unwrap(),
        )
        .unwrap();
        let commands = exported_map["events"][1]["pages"][0]["list"]
            .as_array()
            .unwrap();
        assert_eq!(
            commands[0]["parameters"],
            json!(["Actor1", 2, 0, 2, "Alicia"])
        );
        assert_eq!(
            commands
                .iter()
                .map(|command| command["code"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![101, 401, 401, 0]
        );
        assert_eq!(
            fs::read_to_string(data_dir.join("System.json")).unwrap(),
            system_source
        );
        assert_eq!(
            fs::read_to_string(data_dir.join("Map001.json")).unwrap(),
            map_source
        );

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&output_dir);
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
    fn test_export_unity_loose_json_roundtrip_preserves_bom_and_source() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_export_unity_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let mut source = vec![0xef, 0xbb, 0xbf];
        source.extend_from_slice(br#"{"ui/menu~title":"Play","quit":"Quit","blank":"  "}"#);
        write_unity_mono_fixture(&root, &source);
        let mut project = unity_project_from_source(&root);
        let entry = project.files[0]
            .entries
            .iter_mut()
            .find(|entry| entry.id == "UnityJson:/ui~1menu~0title")
            .unwrap();
        entry.translation = "Jugar".into();
        entry.status = EntryStatus::Reviewed;
        let source_snapshot = snapshot_file_tree(&root);

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_unity_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let result = export_project(&project, &output_dir).unwrap();

        assert_eq!(result.files_exported, 1);
        assert_eq!(result.entries_applied, 1);
        assert_eq!(result.total_entries, 2);
        assert_eq!(snapshot_file_tree(&root), source_snapshot);

        let output_path = output_dir.join(&project.files[0].relative_path);
        let output_bytes = fs::read(&output_path).unwrap();
        assert!(output_bytes.starts_with(&[0xef, 0xbb, 0xbf]));
        let readback = crate::parser::parse_game(&output_dir).unwrap();
        assert_eq!(readback.engine, Engine::Unity);
        let readback_entries = readback.files[0]
            .entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry.original.as_str()))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            readback_entries.get("UnityJson:/ui~1menu~0title"),
            Some(&"Jugar")
        );
        assert_eq!(readback_entries.get("UnityJson:/quit"), Some(&"Quit"));

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn test_export_unity_rejects_source_drift_before_creating_output() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_export_unity_drift_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let source_path = write_unity_mono_fixture(&root, br#"{"play":"Play"}"#);
        let mut project = unity_project_from_source(&root);
        project.files[0].entries[0].translation = "Jugar".into();
        project.files[0].entries[0].status = EntryStatus::Reviewed;
        let changed_source = br#"{"play":"Launch"}"#;
        fs::write(&source_path, changed_source).unwrap();

        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_unity_drift_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let error = export_project(&project, &output_dir)
            .unwrap_err()
            .to_string();

        assert!(error.contains("Source game changed since import"));
        assert!(error.contains("original text changed"));
        assert!(!output_dir.exists());
        assert_eq!(fs::read(&source_path).unwrap(), changed_source);

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(output_dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_export_rejects_source_symlink_and_cleans_output() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "fairtl_export_unity_symlink_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        write_unity_mono_fixture(&root, br#"{"play":"Play"}"#);
        let mut project = unity_project_from_source(&root);
        project.files[0].entries[0].translation = "Jugar".into();
        project.files[0].entries[0].status = EntryStatus::Reviewed;
        let external = std::env::temp_dir().join(format!(
            "fairtl_export_external_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::write(&external, "outside").unwrap();
        symlink(&external, root.join("outside-link.txt")).unwrap();
        let output_dir = std::env::temp_dir().join(format!(
            "fairtl_export_unity_symlink_out_{}",
            uuid::Uuid::new_v4().as_simple()
        ));

        let error = export_project(&project, &output_dir)
            .unwrap_err()
            .to_string();
        assert!(error.contains("symbolic link") || error.contains("reparse point"));
        assert!(!output_dir.exists());
        assert_eq!(fs::read_to_string(&external).unwrap(), "outside");

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(output_dir);
        let _ = fs::remove_file(external);
    }

    #[cfg(windows)]
    #[test]
    fn test_export_rejects_source_symlink_and_cleans_output_when_windows_allows_creation() {
        use std::io::ErrorKind;
        use std::os::windows::fs::symlink_file;

        let root = std::env::temp_dir().join(format!(
            "fairtl_export_unity_symlink_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        write_unity_mono_fixture(&root, br#"{"play":"Play"}"#);
        let mut project = unity_project_from_source(&root);
        project.files[0].entries[0].translation = "Jugar".into();
        project.files[0].entries[0].status = EntryStatus::Reviewed;
        let external = std::env::temp_dir().join(format!(
            "fairtl_export_external_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::write(&external, "outside").unwrap();
        let link = root.join("outside-link.txt");
        match symlink_file(&external, &link) {
            Ok(()) => {
                let output_dir = std::env::temp_dir().join(format!(
                    "fairtl_export_unity_symlink_out_{}",
                    uuid::Uuid::new_v4().as_simple()
                ));
                let error = export_project(&project, &output_dir)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("symbolic link") || error.contains("reparse point"));
                assert!(!output_dir.exists());
                assert_eq!(fs::read_to_string(&external).unwrap(), "outside");
                let _ = fs::remove_dir_all(output_dir);
            }
            Err(error)
                if error.kind() == ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(1314) => {}
            Err(error) => panic!("could not create export symlink fixture: {error}"),
        }

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_file(external);
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

use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::AppError;
use crate::models::{EntryStatus, TranslationEntry, TranslationFile};

const LOCALIZATION_PATH_SIGNALS: &[&str] = &["localization", "locales", "lang", "i18n", "strings"];
const MAX_LOCALIZATION_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_STREAMING_ASSET_DEPTH: usize = 8;
const MAX_STREAMING_ASSET_DIRECTORIES: usize = 2_048;
const MAX_STREAMING_ASSET_ENTRIES: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityLooseJsonInventory {
    pub data_dir_relative_path: String,
    pub localization_file_relative_path: String,
}

/// Lightweight root recognition used by the shared game-directory resolver.
/// Validation of the supported Mono/JSON subset remains the inventory's job so
/// recognizable IL2CPP builds can report a precise unsupported-backend error.
pub(super) fn is_unity_game_root(game_dir: &Path) -> Result<bool, AppError> {
    let root_metadata = match fs::symlink_metadata(game_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(scan_error(game_dir, error)),
    };
    if metadata_is_link_or_reparse(&root_metadata) {
        return Err(symlink_error(game_dir));
    }
    if !root_metadata.is_dir() {
        return Ok(false);
    }

    let Some(unity_player) = find_child(game_dir, "UnityPlayer.dll")? else {
        return Ok(false);
    };
    if !fs::symlink_metadata(&unity_player)
        .map_err(|error| scan_error(&unity_player, error))?
        .is_file()
    {
        return Ok(false);
    }

    for entry in read_dir(game_dir)? {
        let entry = entry.map_err(|error| scan_error(game_dir, error))?;
        if !entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with("_data")
        {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| scan_error(&path, error))?;
        if metadata_is_link_or_reparse(&metadata) {
            return Err(symlink_error(&path));
        }
        if !metadata.is_dir() {
            continue;
        }
        let Some(global_game_managers) = find_child(&path, "globalgamemanagers")? else {
            continue;
        };
        if fs::symlink_metadata(&global_game_managers)
            .map_err(|error| scan_error(&global_game_managers, error))?
            .is_file()
        {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Inventory the deliberately narrow Unity Loose JSON v0 support boundary.
pub fn inventory_unity_game(game_dir: &Path) -> Result<UnityLooseJsonInventory, AppError> {
    let root_metadata = fs::symlink_metadata(game_dir).map_err(|error| {
        AppError::NotFound(format!(
            "Unity game directory '{}' could not be inspected: {error}",
            game_dir.display()
        ))
    })?;
    if metadata_is_link_or_reparse(&root_metadata) {
        return Err(symlink_error(game_dir));
    }
    if !root_metadata.is_dir() {
        return Err(AppError::NotFound(format!(
            "Unity game directory is not a directory: {}",
            game_dir.display()
        )));
    }

    let mut data_dirs = Vec::new();
    for entry in read_dir(game_dir)? {
        let entry = entry.map_err(|error| scan_error(game_dir, error))?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with("_data") {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| scan_error(&path, error))?;
        if metadata_is_link_or_reparse(&metadata) {
            return Err(symlink_error(&path));
        }
        if metadata.is_dir() {
            data_dirs.push(path);
        }
    }
    data_dirs.sort();
    if data_dirs.len() != 1 {
        return Err(AppError::Project(format!(
            "Unity Loose JSON v0 requires exactly one *_Data directory at the game root; found {}.",
            data_dirs.len()
        )));
    }
    let data_dir = data_dirs.remove(0);

    require_file(game_dir, "UnityPlayer.dll")?;
    require_file(&data_dir, "globalgamemanagers")?;

    let has_game_assembly = find_child(game_dir, "GameAssembly.dll")?.is_some();
    let has_global_metadata = find_nested_child(
        &data_dir,
        &["il2cpp_data", "Metadata", "global-metadata.dat"],
    )?
    .is_some();
    if has_game_assembly || has_global_metadata {
        return Err(AppError::Project(
            "IL2CPP markers were found. Unity Loose JSON v0 supports Windows Mono builds only and will not patch IL2CPP games."
                .into(),
        ));
    }

    let managed_dir = require_directory(&data_dir, "Managed")?;
    require_file(&managed_dir, "Assembly-CSharp.dll")?;
    let streaming_assets = require_directory(&data_dir, "StreamingAssets")?;
    let mut candidates = scan_localization_candidates(&streaming_assets)?;
    candidates.sort();
    if candidates.len() != 1 {
        let shown = candidates
            .iter()
            .take(5)
            .filter_map(|path| normalized_relative(game_dir, path).ok())
            .collect::<Vec<_>>()
            .join(", ");
        let details = if shown.is_empty() {
            String::new()
        } else {
            format!(" Candidates: {shown}.")
        };
        return Err(AppError::Project(format!(
            "Unity Loose JSON v0 requires exactly one localization JSON candidate under StreamingAssets; found {} candidates.{details}",
            candidates.len()
        )));
    }

    Ok(UnityLooseJsonInventory {
        data_dir_relative_path: normalized_relative(game_dir, &data_dir)?,
        localization_file_relative_path: normalized_relative(game_dir, &candidates[0])?,
    })
}

/// Parse the single supported loose Unity localization JSON without modifying it.
/// Empty keys are rejected; blank string values are valid but ignored.
pub fn parse_unity_game(game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
    let inventory = inventory_unity_game(game_dir)?;
    let source_path = game_dir.join(Path::new(&inventory.localization_file_relative_path));
    let metadata =
        fs::symlink_metadata(&source_path).map_err(|error| scan_error(&source_path, error))?;
    if metadata_is_link_or_reparse(&metadata) {
        return Err(symlink_error(&source_path));
    }
    if metadata.len() > MAX_LOCALIZATION_JSON_BYTES as u64 {
        return Err(AppError::Project(format!(
            "Unity localization JSON '{}' exceeds the {} byte size limit.",
            inventory.localization_file_relative_path, MAX_LOCALIZATION_JSON_BYTES
        )));
    }

    let bytes = fs::read(&source_path).map_err(|error| scan_error(&source_path, error))?;
    if bytes.len() > MAX_LOCALIZATION_JSON_BYTES {
        return Err(AppError::Project(format!(
            "Unity localization JSON '{}' exceeds the {} byte size limit.",
            inventory.localization_file_relative_path, MAX_LOCALIZATION_JSON_BYTES
        )));
    }
    let bytes = bytes
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(bytes.as_slice());
    let text = std::str::from_utf8(bytes).map_err(|error| {
        AppError::Parse(format!(
            "Unity localization JSON '{}' must be UTF-8: {error}",
            inventory.localization_file_relative_path
        ))
    })?;
    let json: Value = serde_json::from_str(text).map_err(|error| {
        AppError::Parse(format!(
            "Unity localization JSON '{}' is invalid JSON: {error}",
            inventory.localization_file_relative_path
        ))
    })?;
    let object = json.as_object().ok_or_else(|| {
        AppError::Parse(format!(
            "Unity localization JSON '{}' must be a flat string-to-string object.",
            inventory.localization_file_relative_path
        ))
    })?;

    let mut entries = Vec::new();
    for (key, value) in object {
        if key.trim().is_empty() {
            return Err(AppError::Parse(format!(
                "Unity localization JSON '{}' contains an empty key; keys must be stable non-empty IDs.",
                inventory.localization_file_relative_path
            )));
        }
        if key.chars().any(char::is_control) {
            return Err(AppError::Parse(format!(
                "Unity localization JSON '{}' contains a control character in key {:?}.",
                inventory.localization_file_relative_path, key
            )));
        }
        let original = value.as_str().ok_or_else(|| {
            AppError::Parse(format!(
                "Unity localization JSON '{}' must be a flat string-to-string object; key {:?} has a non-string value.",
                inventory.localization_file_relative_path, key
            ))
        })?;
        if original.trim().is_empty() {
            continue;
        }
        entries.push(TranslationEntry {
            id: format!("UnityJson:/{}", escape_json_pointer_segment(key)),
            file_context: format!("{} > {}", inventory.localization_file_relative_path, key),
            original: original.to_string(),
            translation: String::new(),
            status: EntryStatus::Untranslated,
            notes: String::new(),
        });
    }
    if entries.is_empty() {
        return Err(AppError::Parse(format!(
            "Unity localization JSON '{}' contains no non-empty translatable string values.",
            inventory.localization_file_relative_path
        )));
    }

    Ok(vec![TranslationFile {
        relative_path: inventory.localization_file_relative_path,
        entries,
    }])
}

fn scan_localization_candidates(streaming_assets: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut queue = VecDeque::from([(streaming_assets.to_path_buf(), 0usize)]);
    let mut scanned_directories = 0usize;
    let mut scanned_entries = 0usize;
    let mut candidates = Vec::new();

    while let Some((directory, depth)) = queue.pop_front() {
        scanned_directories += 1;
        if scanned_directories > MAX_STREAMING_ASSET_DIRECTORIES {
            return Err(AppError::Project(format!(
                "Unity StreamingAssets scan exceeded the {} directory limit. Narrow or flatten the localization path.",
                MAX_STREAMING_ASSET_DIRECTORIES
            )));
        }

        for entry in read_dir(&directory)? {
            scanned_entries += 1;
            if scanned_entries > MAX_STREAMING_ASSET_ENTRIES {
                return Err(AppError::Project(format!(
                    "Unity StreamingAssets scan exceeded the {} entry limit. Narrow the localization folder before importing.",
                    MAX_STREAMING_ASSET_ENTRIES
                )));
            }
            let entry = entry.map_err(|error| scan_error(&directory, error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| scan_error(&path, error))?;
            if metadata_is_link_or_reparse(&metadata) {
                return Err(symlink_error(&path));
            }
            if metadata.is_dir() {
                if depth >= MAX_STREAMING_ASSET_DEPTH {
                    return Err(AppError::Project(format!(
                        "Unity StreamingAssets scan reached the depth limit of {} at '{}'.",
                        MAX_STREAMING_ASSET_DEPTH,
                        path.display()
                    )));
                }
                queue.push_back((path, depth + 1));
                continue;
            }
            if metadata.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| extension.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
                && path_has_localization_signal(streaming_assets, &path)
            {
                candidates.push(path);
            }
        }
    }

    Ok(candidates)
}

fn path_has_localization_signal(streaming_assets: &Path, path: &Path) -> bool {
    path.strip_prefix(streaming_assets)
        .unwrap_or(path)
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .any(|component| {
            let lower = component.to_ascii_lowercase();
            let stem = lower.strip_suffix(".json").unwrap_or(&lower);
            LOCALIZATION_PATH_SIGNALS.iter().any(|signal| {
                stem.strip_prefix(signal).is_some_and(|suffix| {
                    suffix.is_empty()
                        || matches!(suffix.as_bytes().first(), Some(b'-' | b'_' | b'.'))
                })
            })
        })
}

fn find_nested_child(root: &Path, segments: &[&str]) -> Result<Option<PathBuf>, AppError> {
    let mut current = root.to_path_buf();
    for segment in segments {
        let Some(next) = find_child(&current, segment)? else {
            return Ok(None);
        };
        current = next;
    }
    Ok(Some(current))
}

fn require_file(directory: &Path, name: &str) -> Result<PathBuf, AppError> {
    let path = find_child(directory, name)?.ok_or_else(|| {
        AppError::Project(format!(
            "Unity Loose JSON v0 requires file '{}' under '{}'.",
            name,
            directory.display()
        ))
    })?;
    if !fs::symlink_metadata(&path)
        .map_err(|error| scan_error(&path, error))?
        .is_file()
    {
        return Err(AppError::Project(format!(
            "Unity marker '{}' is not a regular file.",
            path.display()
        )));
    }
    Ok(path)
}

fn require_directory(directory: &Path, name: &str) -> Result<PathBuf, AppError> {
    let path = find_child(directory, name)?.ok_or_else(|| {
        AppError::Project(format!(
            "Unity Loose JSON v0 requires directory '{}' under '{}'.",
            name,
            directory.display()
        ))
    })?;
    if !fs::symlink_metadata(&path)
        .map_err(|error| scan_error(&path, error))?
        .is_dir()
    {
        return Err(AppError::Project(format!(
            "Unity path '{}' is not a directory.",
            path.display()
        )));
    }
    Ok(path)
}

fn find_child(directory: &Path, name: &str) -> Result<Option<PathBuf>, AppError> {
    let mut found = None;
    for entry in read_dir(directory)? {
        let entry = entry.map_err(|error| scan_error(directory, error))?;
        if !entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(name)
        {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| scan_error(&path, error))?;
        if metadata_is_link_or_reparse(&metadata) {
            return Err(symlink_error(&path));
        }
        if found.is_some() {
            return Err(AppError::Project(format!(
                "Unity path '{}' contains multiple case-insensitive matches for '{}'.",
                directory.display(),
                name
            )));
        }
        found = Some(path);
    }
    Ok(found)
}

fn read_dir(directory: &Path) -> Result<fs::ReadDir, AppError> {
    fs::read_dir(directory).map_err(|error| scan_error(directory, error))
}

fn normalized_relative(root: &Path, path: &Path) -> Result<String, AppError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        AppError::Project(format!(
            "Unity path '{}' escaped game root '{}'.",
            path.display(),
            root.display()
        ))
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        AppError::Project(format!(
            "Unity path '{}' is not valid Unicode.",
            relative.display()
        ))
    })?;
    Ok(relative.replace('\\', "/"))
}

fn escape_json_pointer_segment(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn scan_error(path: &Path, error: std::io::Error) -> AppError {
    AppError::Io(format!(
        "Could not inspect Unity path '{}': {error}",
        path.display()
    ))
}

fn symlink_error(path: &Path) -> AppError {
    AppError::Project(format!(
        "Unity Loose JSON v0 refuses symbolic link or reparse point '{}'; copy real files into the game tree before importing.",
        path.display()
    ))
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || metadata_is_reparse_point(metadata)
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fairtl_unity_{label}_{}",
            uuid::Uuid::new_v4().as_simple()
        ))
    }

    fn write_mono_root(root: &Path) -> PathBuf {
        let data_dir = root.join("Sample_Data");
        fs::create_dir_all(data_dir.join("Managed")).unwrap();
        fs::create_dir_all(data_dir.join("StreamingAssets")).unwrap();
        fs::write(root.join("UnityPlayer.dll"), []).unwrap();
        fs::write(data_dir.join("globalgamemanagers"), []).unwrap();
        fs::write(data_dir.join("Managed").join("Assembly-CSharp.dll"), []).unwrap();
        data_dir
    }

    fn write_streaming_file(data_dir: &Path, relative_path: &str, bytes: &[u8]) -> PathBuf {
        let path = data_dir.join("StreamingAssets").join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn inventories_and_parses_one_bom_utf8_flat_json_without_mutating_source() {
        let root = temp_root("valid");
        let data_dir = write_mono_root(&root);
        let mut source = vec![0xef, 0xbb, 0xbf];
        source.extend_from_slice(br#"{"menu.play":"Play","blank":"  "}"#);
        let source_path = write_streaming_file(&data_dir, "Localization/strings.json", &source);

        let inventory = inventory_unity_game(&root).unwrap();
        assert_eq!(inventory.data_dir_relative_path, "Sample_Data");
        assert_eq!(
            inventory.localization_file_relative_path,
            "Sample_Data/StreamingAssets/Localization/strings.json"
        );

        let files = parse_unity_game(&root).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].relative_path,
            inventory.localization_file_relative_path
        );
        assert_eq!(files[0].entries.len(), 1);
        assert_eq!(files[0].entries[0].id, "UnityJson:/menu.play");
        assert_eq!(files[0].entries[0].original, "Play");
        assert!(files[0].entries[0].file_context.contains("menu.play"));
        assert_eq!(fs::read(source_path).unwrap(), source);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn escapes_json_pointer_key_segments() {
        let root = temp_root("pointer");
        let data_dir = write_mono_root(&root);
        write_streaming_file(&data_dir, "i18n/text.json", br#"{"ui/menu~title":"Play"}"#);

        let files = parse_unity_game(&root).unwrap();
        assert_eq!(files[0].entries[0].id, "UnityJson:/ui~1menu~0title");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_il2cpp_markers_even_when_mono_markers_exist() {
        let root = temp_root("il2cpp");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            br#"{"play":"Play"}"#,
        );
        fs::write(root.join("GameAssembly.dll"), []).unwrap();

        let error = inventory_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("IL2CPP"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_multiple_localization_candidates() {
        let root = temp_root("multiple_json");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            br#"{"play":"Play"}"#,
        );
        write_streaming_file(&data_dir, "Locales/menu.json", br#"{"quit":"Quit"}"#);

        let error = inventory_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("exactly one"));
        assert!(error.contains("2 candidates"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_nested_or_non_string_json_values() {
        let root = temp_root("nested");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            br#"{"menu":{"play":"Play"},"lives":3}"#,
        );

        let error = parse_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("flat string-to-string"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let root = temp_root("invalid_utf8");
        let data_dir = write_mono_root(&root);
        write_streaming_file(&data_dir, "Localization/strings.json", &[0xff, 0xfe, 0xfd]);

        let error = parse_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("UTF-8"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_empty_keys() {
        let root = temp_root("empty_key");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            br#"{"":"Visible text"}"#,
        );

        let error = parse_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("empty key"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_oversized_json_before_reading_it() {
        let root = temp_root("oversized");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            &vec![b' '; MAX_LOCALIZATION_JSON_BYTES + 1],
        );

        let error = parse_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("size limit"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_streaming_assets_beyond_scan_depth() {
        let root = temp_root("depth");
        let data_dir = write_mono_root(&root);
        let mut relative = PathBuf::new();
        for index in 0..=MAX_STREAMING_ASSET_DEPTH {
            relative.push(format!("level{index}"));
        }
        relative.push("localization.json");
        write_streaming_file(
            &data_dir,
            &relative.to_string_lossy(),
            br#"{"play":"Play"}"#,
        );

        let error = inventory_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("depth limit"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_multiple_data_directories() {
        let root = temp_root("multiple_data");
        let data_dir = write_mono_root(&root);
        write_streaming_file(
            &data_dir,
            "Localization/strings.json",
            br#"{"play":"Play"}"#,
        );
        fs::create_dir_all(root.join("Other_Data")).unwrap();

        let error = inventory_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("exactly one *_Data"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_inside_streaming_assets() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink");
        let data_dir = write_mono_root(&root);
        let target = write_streaming_file(&data_dir, "target.txt", br#"{"play":"Play"}"#);
        let link = data_dir
            .join("StreamingAssets")
            .join("Localization")
            .join("strings.json");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(target, link).unwrap();

        let error = inventory_unity_game(&root).unwrap_err().to_string();
        assert!(error.contains("symbolic link"));

        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn rejects_symlinks_inside_streaming_assets_when_windows_allows_creation() {
        use std::io::ErrorKind;
        use std::os::windows::fs::symlink_file;

        let root = temp_root("symlink");
        let data_dir = write_mono_root(&root);
        let target = write_streaming_file(&data_dir, "target.txt", br#"{"play":"Play"}"#);
        let link = data_dir
            .join("StreamingAssets")
            .join("Localization")
            .join("strings.json");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        match symlink_file(target, &link) {
            Ok(()) => {
                let error = inventory_unity_game(&root).unwrap_err().to_string();
                assert!(error.contains("symbolic link"));
            }
            Err(error)
                if error.kind() == ErrorKind::PermissionDenied
                    || error.raw_os_error() == Some(1314) => {}
            Err(error) => panic!("could not create Unity symlink fixture: {error}"),
        }

        let _ = fs::remove_dir_all(root);
    }
}

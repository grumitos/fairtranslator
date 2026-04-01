use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::AppError;
use crate::models::{Engine, EntryStatus, TranslationEntry, TranslationFile};

// ---------------------------------------------------------------------------
// Database files whose array elements contain translatable text fields
// ---------------------------------------------------------------------------

/// Text fields we extract from database array entries (Actors, Items, Skills, etc.)
pub const DB_TEXT_FIELDS: &[&str] = &[
    "name",
    "description",
    "profile",
    "nickname",
    "message1",
    "message2",
    "message3",
    "message4",
];

/// Files in /data/ that are treated as database arrays
const DATABASE_FILES: &[&str] = &[
    "Actors", "Armors", "Classes", "Enemies", "Items", "Skills", "States", "Weapons",
];

/// Event command codes that contain translatable text
pub const CODE_SHOW_TEXT: i64 = 401;
pub const CODE_SHOW_CHOICES: i64 = 102;
pub const CODE_SCROLL_TEXT: i64 = 405;
pub const CODE_CHANGE_NAME: i64 = 320;
pub const CODE_CHANGE_NICKNAME: i64 = 324;
pub const CODE_CHANGE_PROFILE: i64 = 325;

const MAX_GAME_ROOT_SEARCH_DEPTH: usize = 8;
const MAX_GAME_ROOT_SCAN_DIRS: usize = 20_000;

// ---------------------------------------------------------------------------
// FileKind — classify a /data/ JSON file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum FileKind {
    Database,
    Map,
    CommonEvents,
    Troops,
    System,
    Ignored,
}

/// Classify a filename (stem, no extension) into a FileKind.
pub fn classify_file(stem: &str) -> FileKind {
    if DATABASE_FILES
        .iter()
        .any(|&db| db.eq_ignore_ascii_case(stem))
    {
        FileKind::Database
    } else if stem.eq_ignore_ascii_case("CommonEvents") {
        FileKind::CommonEvents
    } else if stem.eq_ignore_ascii_case("Troops") {
        FileKind::Troops
    } else if stem.eq_ignore_ascii_case("System") {
        FileKind::System
    } else if stem.len() >= 4
        && stem[..3].eq_ignore_ascii_case("Map")
        && stem[3..].chars().all(|c| c.is_ascii_digit())
    {
        FileKind::Map
    } else {
        FileKind::Ignored
    }
}

// ---------------------------------------------------------------------------
// Engine detection
// ---------------------------------------------------------------------------

/// Detect RPG Maker engine from game directory markers.
/// - MZ: `js/rmmz_core.js`
/// - MV: `js/rpg_core.js`
/// - VX Ace / VX / XP: `Data/*.rvdata2|*.rvdata|*.rxdata`
pub fn detect_engine(game_dir: &Path) -> Result<Engine, AppError> {
    if game_dir.join("js").join("rmmz_core.js").exists() {
        Ok(Engine::Mz)
    } else if game_dir.join("js").join("rpg_core.js").exists() {
        Ok(Engine::Mv)
    } else if let Some(legacy_engine) = detect_legacy_engine(game_dir) {
        Ok(legacy_engine)
    } else {
        Err(AppError::NotFound(format!(
            "not found: could not detect RPG Maker engine under '{}'. Expected MV/MZ markers (js/rmmz_core.js or js/rpg_core.js) or legacy data markers (Data/*.rvdata2|*.rvdata|*.rxdata).",
            game_dir.display()
        )))
    }
}

fn data_dir_for(dir: &Path) -> Option<PathBuf> {
    let data = dir.join("data");
    if data.is_dir() {
        return Some(data);
    }
    let alt = dir.join("Data");
    if alt.is_dir() {
        return Some(alt);
    }
    None
}

fn detect_legacy_engine(dir: &Path) -> Option<Engine> {
    let data_dir = data_dir_for(dir)?;
    if data_dir.join("System.rvdata2").is_file() {
        return Some(Engine::VxAce);
    }
    if data_dir.join("System.rvdata").is_file() {
        return Some(Engine::Vx);
    }
    if data_dir.join("System.rxdata").is_file() {
        return Some(Engine::Xp);
    }

    let mut has_rvdata2 = false;
    let mut has_rvdata = false;
    let mut has_rxdata = false;
    let entries = std::fs::read_dir(&data_dir).ok()?;
    for entry in entries.flatten() {
        let ext = entry
            .path()
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rvdata2" => has_rvdata2 = true,
            "rvdata" => has_rvdata = true,
            "rxdata" => has_rxdata = true,
            _ => {}
        }
        // Keep scanning to avoid directory-order bias in mixed legacy folders.
        if has_rvdata2 && has_rvdata && has_rxdata {
            break;
        }
    }

    if has_rvdata2 {
        Some(Engine::VxAce)
    } else if has_rvdata {
        Some(Engine::Vx)
    } else if has_rxdata {
        Some(Engine::Xp)
    } else {
        None
    }
}

fn is_mv_mz_root(dir: &Path) -> bool {
    let has_engine = dir.join("js").join("rmmz_core.js").is_file()
        || dir.join("js").join("rpg_core.js").is_file();
    let has_data = data_dir_for(dir).is_some();
    has_engine && has_data
}

fn is_rpgmaker_root(dir: &Path) -> bool {
    is_mv_mz_root(dir) || detect_legacy_engine(dir).is_some()
}

/// Resolve the actual RPG Maker game root from a selected directory.
///
/// Behavior:
/// - If `selected_dir` is already a game root, returns it.
/// - Otherwise, recursively searches nested subdirectories up to a bounded depth.
/// - If exactly one root is found, returns it.
/// - If multiple roots are found, returns an error so user can choose explicitly.
pub fn resolve_game_dir(selected_dir: &Path) -> Result<PathBuf, AppError> {
    if is_rpgmaker_root(selected_dir) {
        return Ok(selected_dir.to_path_buf());
    }

    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((selected_dir.to_path_buf(), 0));

    let mut scanned_dirs = 0usize;
    let mut candidates: Vec<PathBuf> = Vec::new();

    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= MAX_GAME_ROOT_SEARCH_DEPTH {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            scanned_dirs += 1;
            if scanned_dirs > MAX_GAME_ROOT_SCAN_DIRS {
                return Err(AppError::Project(format!(
                    "Search limit reached while scanning '{}'. Please select the exact game folder.",
                    selected_dir.display()
                )));
            }

            if is_rpgmaker_root(&path) {
                candidates.push(path);
                if candidates.len() > 1 {
                    break;
                }
            } else {
                queue.push_back((path, depth + 1));
            }
        }

        if candidates.len() > 1 {
            break;
        }
    }

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(AppError::NotFound(format!(
            "No RPG Maker game root found under '{}'. Select a folder containing MV/MZ markers (js/ + data/) or legacy Data/*.rvdata2|*.rvdata|*.rxdata.",
            selected_dir.display()
        ))),
        _ => {
            candidates.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
            let list = candidates
                .iter()
                .take(5)
                .map(|p| format!("- {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            Err(AppError::Project(format!(
                "Multiple RPG Maker game roots were found under '{}'. Please select one game folder directly:\n{}",
                selected_dir.display(),
                list
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Scan game directory
// ---------------------------------------------------------------------------

/// Scan the game's `/data/` directory and return sorted list of JSON files
/// that contain translatable content (ignoring MapInfos, Tilesets, Animations, etc.).
pub fn scan_game_dir(game_dir: &Path) -> Result<Vec<PathBuf>, AppError> {
    let data_dir = game_dir.join("data");
    if !data_dir.is_dir() {
        // MV sometimes uses "Data" (capital D)
        let alt = game_dir.join("Data");
        if alt.is_dir() {
            return scan_data_dir(&alt);
        }
        return Err(AppError::NotFound(format!(
            "No 'data' directory found in {}",
            game_dir.display()
        )));
    }
    scan_data_dir(&data_dir)
}

fn scan_data_dir(data_dir: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(data_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        })
        .filter(|path| {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            classify_file(stem) != FileKind::Ignored
        })
        .collect();

    files.sort_by(|a, b| {
        let sa = a.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let sb = b.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let order = |s: &str| -> u8 {
            match classify_file(s) {
                FileKind::System => 0,
                FileKind::Database => 1,
                FileKind::CommonEvents => 2,
                FileKind::Troops => 3,
                FileKind::Map => 4,
                FileKind::Ignored => 5,
            }
        };
        order(sa).cmp(&order(sb)).then(sa.cmp(sb))
    });

    Ok(files)
}

// ---------------------------------------------------------------------------
// Top-level parse_game
// ---------------------------------------------------------------------------

/// Parse MV/MZ JSON files into translatable entries.
pub fn parse_mv_mz_game(game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
    let json_files = scan_game_dir(game_dir)?;

    let mut translation_files = Vec::new();

    for path in &json_files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let kind = classify_file(&stem);

        let content = std::fs::read_to_string(path)?;
        let json: Value = serde_json::from_str(&content)?;

        let entries = extract_entries(&stem, &kind, &json)?;

        if !entries.is_empty() {
            // Relative path from game_dir
            let rel_path = path
                .strip_prefix(game_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            translation_files.push(TranslationFile {
                relative_path: rel_path,
                entries,
            });
        }
    }

    Ok(translation_files)
}

// ---------------------------------------------------------------------------
// Entry extraction dispatcher
// ---------------------------------------------------------------------------

fn extract_entries(
    stem: &str,
    kind: &FileKind,
    json: &Value,
) -> Result<Vec<TranslationEntry>, AppError> {
    match kind {
        FileKind::Database => extract_database_entries(stem, json),
        FileKind::Map => extract_map_entries(stem, json),
        FileKind::CommonEvents => extract_common_events_entries(json),
        FileKind::Troops => extract_troops_entries(json),
        FileKind::System => extract_system_entries(json),
        FileKind::Ignored => Ok(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Database entries (Actors, Items, Skills, Weapons, Armors, etc.)
// ---------------------------------------------------------------------------

fn extract_database_entries(stem: &str, json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected JSON array")))?;

    let mut entries = Vec::new();

    for (idx, item) in arr.iter().enumerate() {
        if item.is_null() {
            continue; // First element is always null in RPG Maker arrays
        }
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };

        let item_id = obj.get("id").and_then(|v| v.as_u64()).unwrap_or(idx as u64);

        let item_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("???");

        for &field in DB_TEXT_FIELDS {
            if let Some(Value::String(text)) = obj.get(field) {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                entries.push(make_entry(
                    &format!("{stem}:{field}:{item_id}"),
                    &format!("{stem} > #{item_id} ({item_name}) > {field}"),
                    text.clone(),
                ));
            }
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Map entries — displayName + event commands
// ---------------------------------------------------------------------------

fn extract_map_entries(stem: &str, json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let obj = json
        .as_object()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected JSON object")))?;

    let mut entries = Vec::new();

    // Map display name
    if let Some(Value::String(display_name)) = obj.get("displayName") {
        let trimmed = display_name.trim();
        if !trimmed.is_empty() {
            entries.push(make_entry(
                &format!("{stem}:displayName"),
                &format!("{stem} > displayName"),
                display_name.clone(),
            ));
        }
    }

    // Events
    if let Some(Value::Array(events)) = obj.get("events") {
        for (ev_idx, event) in events.iter().enumerate() {
            if event.is_null() {
                continue;
            }
            let ev_obj = match event.as_object() {
                Some(o) => o,
                None => continue,
            };
            let ev_id = ev_obj
                .get("id")
                .and_then(|v| v.as_u64())
                .unwrap_or(ev_idx as u64);
            let ev_name = ev_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ev_label = if ev_name.is_empty() {
                format!("Event {ev_id}")
            } else {
                format!("Event {ev_id} ({ev_name})")
            };

            if let Some(Value::Array(pages)) = ev_obj.get("pages") {
                for (page_idx, page) in pages.iter().enumerate() {
                    if let Some(Value::Array(list)) = page.get("list") {
                        let context_prefix = format!("{stem} > {ev_label} > Page {}", page_idx + 1);
                        let id_prefix = format!("{stem}:{ev_id}:{page_idx}");
                        let cmds = extract_event_commands(list, &id_prefix, &context_prefix);
                        entries.extend(cmds);
                    }
                }
            }
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// CommonEvents — event commands from each common event
// ---------------------------------------------------------------------------

fn extract_common_events_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse("CommonEvents: expected JSON array".into()))?;

    let mut entries = Vec::new();

    for (idx, item) in arr.iter().enumerate() {
        if item.is_null() {
            continue;
        }
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        let ce_id = obj.get("id").and_then(|v| v.as_u64()).unwrap_or(idx as u64);
        let ce_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let ce_label = if ce_name.is_empty() {
            format!("CommonEvent {ce_id}")
        } else {
            format!("CommonEvent {ce_id} ({ce_name})")
        };

        if let Some(Value::Array(list)) = obj.get("list") {
            let id_prefix = format!("CommonEvents:{ce_id}");
            let context_prefix = format!("CommonEvents > {ce_label}");
            let cmds = extract_event_commands(list, &id_prefix, &context_prefix);
            entries.extend(cmds);
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Troops — troop names + battle event commands
// ---------------------------------------------------------------------------

fn extract_troops_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse("Troops: expected JSON array".into()))?;

    let mut entries = Vec::new();

    for (idx, item) in arr.iter().enumerate() {
        if item.is_null() {
            continue;
        }
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        let troop_id = obj.get("id").and_then(|v| v.as_u64()).unwrap_or(idx as u64);
        let troop_name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");

        // Troop name
        if !troop_name.trim().is_empty() {
            entries.push(make_entry(
                &format!("Troops:name:{troop_id}"),
                &format!("Troops > #{troop_id} ({troop_name}) > name"),
                troop_name.to_string(),
            ));
        }

        // Battle event pages
        if let Some(Value::Array(pages)) = obj.get("pages") {
            for (page_idx, page) in pages.iter().enumerate() {
                if let Some(Value::Array(list)) = page.get("list") {
                    let context_prefix = format!(
                        "Troops > #{troop_id} ({troop_name}) > Page {}",
                        page_idx + 1
                    );
                    let id_prefix = format!("Troops:{troop_id}:{page_idx}");
                    let cmds = extract_event_commands(list, &id_prefix, &context_prefix);
                    entries.extend(cmds);
                }
            }
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------
// System.json — gameTitle + terms
// ---------------------------------------------------------------------------

fn extract_system_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let obj = json
        .as_object()
        .ok_or_else(|| AppError::Parse("System: expected JSON object".into()))?;

    let mut entries = Vec::new();

    // gameTitle
    if let Some(Value::String(title)) = obj.get("gameTitle") {
        let trimmed = title.trim();
        if !trimmed.is_empty() {
            entries.push(make_entry(
                "System:gameTitle",
                "System > gameTitle",
                title.clone(),
            ));
        }
    }

    // terms / terms.basic / terms.commands / terms.params / terms.messages
    if let Some(terms) = obj.get("terms") {
        extract_terms_recursive(terms, "System:terms", "System > terms", &mut entries);
    }

    Ok(entries)
}

/// Recursively extract string values from the terms object.
fn extract_terms_recursive(
    value: &Value,
    id_prefix: &str,
    context_prefix: &str,
    entries: &mut Vec<TranslationEntry>,
) {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                entries.push(make_entry(id_prefix, context_prefix, s.clone()));
            }
        }
        Value::Object(map) => {
            for (key, val) in map {
                extract_terms_recursive(
                    val,
                    &format!("{id_prefix}.{key}"),
                    &format!("{context_prefix} > {key}"),
                    entries,
                );
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                if val.is_string() {
                    extract_terms_recursive(
                        val,
                        &format!("{id_prefix}.{i}"),
                        &format!("{context_prefix}[{i}]"),
                        entries,
                    );
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Event command extraction (shared by Maps, CommonEvents, Troops)
// ---------------------------------------------------------------------------

/// Extract translatable entries from an event command list.
///
/// Supported codes:
/// - 401: Show Text (continuation lines)
/// - 102: Show Choices
/// - 405: Scrolling Text (continuation lines)
/// - 320: Change Actor Name
/// - 324: Change Actor Nickname
/// - 325: Change Actor Profile
fn extract_event_commands(
    list: &[Value],
    id_prefix: &str,
    context_prefix: &str,
) -> Vec<TranslationEntry> {
    let mut entries = Vec::new();
    let mut text_accumulator: Vec<String> = Vec::new();
    let mut text_start_idx: Option<usize> = None;
    let mut scroll_accumulator: Vec<String> = Vec::new();
    let mut scroll_start_idx: Option<usize> = None;

    for (cmd_idx, cmd) in list.iter().enumerate() {
        let code = cmd.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        let params = cmd.get("parameters");

        // Flush Show Text accumulator if we hit a non-401 code
        if code != CODE_SHOW_TEXT && !text_accumulator.is_empty() {
            let full_text = text_accumulator.join("\n");
            let start = text_start_idx.unwrap_or(cmd_idx);
            entries.push(make_entry(
                &format!("{id_prefix}:text:{start}"),
                &format!("{context_prefix} > text #{}", entries.len() + 1),
                full_text,
            ));
            text_accumulator.clear();
            text_start_idx = None;
        }

        // Flush Scrolling Text accumulator if we hit a non-405 code
        if code != CODE_SCROLL_TEXT && !scroll_accumulator.is_empty() {
            let full_text = scroll_accumulator.join("\n");
            let start = scroll_start_idx.unwrap_or(cmd_idx);
            entries.push(make_entry(
                &format!("{id_prefix}:scroll:{start}"),
                &format!("{context_prefix} > scroll #{}", entries.len() + 1),
                full_text,
            ));
            scroll_accumulator.clear();
            scroll_start_idx = None;
        }

        match code {
            CODE_SHOW_TEXT => {
                // parameters[0] is the text line
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(line)) = p.first() {
                        if text_start_idx.is_none() {
                            text_start_idx = Some(cmd_idx);
                        }
                        text_accumulator.push(line.clone());
                    }
                }
            }
            CODE_SHOW_CHOICES => {
                // parameters[0] is an array of choice strings
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::Array(choices)) = p.first() {
                        for (ci, choice) in choices.iter().enumerate() {
                            if let Value::String(text) = choice {
                                let trimmed = text.trim();
                                if !trimmed.is_empty() {
                                    entries.push(make_entry(
                                        &format!("{id_prefix}:choice:{cmd_idx}:{ci}"),
                                        &format!(
                                            "{context_prefix} > choice #{} option {}",
                                            entries.len() + 1,
                                            ci + 1
                                        ),
                                        text.clone(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            CODE_SCROLL_TEXT => {
                // parameters[0] is the scrolling text line
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(line)) = p.first() {
                        if scroll_start_idx.is_none() {
                            scroll_start_idx = Some(cmd_idx);
                        }
                        scroll_accumulator.push(line.clone());
                    }
                }
            }
            CODE_CHANGE_NAME => {
                // parameters[1] is the new name string
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(name)) = p.get(1) {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            entries.push(make_entry(
                                &format!("{id_prefix}:changeName:{cmd_idx}"),
                                &format!("{context_prefix} > changeName"),
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            CODE_CHANGE_NICKNAME => {
                // parameters[1] is the new nickname string
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(nick)) = p.get(1) {
                        let trimmed = nick.trim();
                        if !trimmed.is_empty() {
                            entries.push(make_entry(
                                &format!("{id_prefix}:changeNickname:{cmd_idx}"),
                                &format!("{context_prefix} > changeNickname"),
                                nick.clone(),
                            ));
                        }
                    }
                }
            }
            CODE_CHANGE_PROFILE => {
                // parameters[1] is the new profile string
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(profile)) = p.get(1) {
                        let trimmed = profile.trim();
                        if !trimmed.is_empty() {
                            entries.push(make_entry(
                                &format!("{id_prefix}:changeProfile:{cmd_idx}"),
                                &format!("{context_prefix} > changeProfile"),
                                profile.clone(),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Flush any remaining accumulators at end of list
    if !text_accumulator.is_empty() {
        let full_text = text_accumulator.join("\n");
        let start = text_start_idx.unwrap_or(0);
        entries.push(make_entry(
            &format!("{id_prefix}:text:{start}"),
            &format!("{context_prefix} > text #{}", entries.len() + 1),
            full_text,
        ));
    }

    if !scroll_accumulator.is_empty() {
        let full_text = scroll_accumulator.join("\n");
        let start = scroll_start_idx.unwrap_or(0);
        entries.push(make_entry(
            &format!("{id_prefix}:scroll:{start}"),
            &format!("{context_prefix} > scroll #{}", entries.len() + 1),
            full_text,
        ));
    }

    entries
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_entry(id: &str, file_context: &str, original: String) -> TranslationEntry {
    TranslationEntry {
        id: id.to_string(),
        file_context: file_context.to_string(),
        original,
        translation: String::new(),
        status: EntryStatus::Untranslated,
        notes: String::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_classify_file() {
        assert_eq!(classify_file("Actors"), FileKind::Database);
        assert_eq!(classify_file("Items"), FileKind::Database);
        assert_eq!(classify_file("Map001"), FileKind::Map);
        assert_eq!(classify_file("Map123"), FileKind::Map);
        assert_eq!(classify_file("CommonEvents"), FileKind::CommonEvents);
        assert_eq!(classify_file("Troops"), FileKind::Troops);
        assert_eq!(classify_file("System"), FileKind::System);
        assert_eq!(classify_file("Tilesets"), FileKind::Ignored);
        assert_eq!(classify_file("Animations"), FileKind::Ignored);
        assert_eq!(classify_file("MapInfos"), FileKind::Ignored);
    }

    #[test]
    fn test_extract_database_entries() {
        let json: Value = serde_json::from_str(
            r#"[
                null,
                {"id": 1, "name": "Harold", "description": "A brave warrior.", "profile": "Born in a small village."},
                {"id": 2, "name": "", "description": "", "profile": ""}
            ]"#,
        )
        .unwrap();

        let entries = extract_database_entries("Actors", &json).unwrap();
        assert_eq!(entries.len(), 3); // name, description, profile for Harold
        assert_eq!(entries[0].id, "Actors:name:1");
        assert_eq!(entries[0].original, "Harold");
        assert_eq!(entries[1].id, "Actors:description:1");
        assert_eq!(entries[1].original, "A brave warrior.");
        assert_eq!(entries[2].id, "Actors:profile:1");
        assert_eq!(entries[2].original, "Born in a small village.");
    }

    #[test]
    fn test_extract_map_entries_display_name() {
        let json: Value = serde_json::from_str(
            r#"{
                "displayName": "Forest of Trials",
                "events": []
            }"#,
        )
        .unwrap();

        let entries = extract_map_entries("Map001", &json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "Map001:displayName");
        assert_eq!(entries[0].original, "Forest of Trials");
    }

    #[test]
    fn test_extract_event_commands_show_text() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 401, "parameters": ["Hello, traveler!"]},
                {"code": 401, "parameters": ["Welcome to our village."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:1:0",
            "Map001 > Event 1 > Page 1",
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].original,
            "Hello, traveler!\nWelcome to our village."
        );
        assert_eq!(entries[0].id, "Map001:1:0:text:0");
    }

    #[test]
    fn test_extract_event_commands_choices() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 102, "parameters": [["Yes", "No", "Maybe"]]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:2:0",
            "Map001 > Event 2 > Page 1",
        );
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].original, "Yes");
        assert_eq!(entries[1].original, "No");
        assert_eq!(entries[2].original, "Maybe");
    }

    #[test]
    fn test_extract_system_entries() {
        let json: Value = serde_json::from_str(
            r#"{
                "gameTitle": "My RPG Adventure",
                "terms": {
                    "basic": ["Level", "Lv", "HP", "MP"],
                    "commands": ["Fight", "Escape"],
                    "messages": {
                        "actionFailure": "%1 has no effect!",
                        "actorDamage": "%1 took %2 damage!"
                    }
                }
            }"#,
        )
        .unwrap();

        let entries = extract_system_entries(&json).unwrap();
        // gameTitle + 4 basic + 2 commands + 2 messages = 9
        assert_eq!(entries.len(), 9);
        assert_eq!(entries[0].id, "System:gameTitle");
        assert_eq!(entries[0].original, "My RPG Adventure");
    }

    #[test]
    fn test_extract_event_commands_scroll_text() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 405, "parameters": ["In a faraway kingdom..."]},
                {"code": 405, "parameters": ["A hero was born."]},
                {"code": 405, "parameters": ["His name was Harold."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:1:0",
            "Map001 > Event 1 > Page 1",
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "Map001:1:0:scroll:0");
        assert_eq!(
            entries[0].original,
            "In a faraway kingdom...\nA hero was born.\nHis name was Harold."
        );
    }

    #[test]
    fn test_extract_event_commands_change_nickname() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 324, "parameters": [1, "The Brave"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:1:0",
            "Map001 > Event 1 > Page 1",
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "Map001:1:0:changeNickname:0");
        assert_eq!(entries[0].original, "The Brave");
    }

    #[test]
    fn test_extract_event_commands_change_profile() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 325, "parameters": [1, "A warrior from the north."]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:1:0",
            "Map001 > Event 1 > Page 1",
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "Map001:1:0:changeProfile:0");
        assert_eq!(entries[0].original, "A warrior from the north.");
    }

    #[test]
    fn test_resolve_game_dir_exact_root() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_resolve_exact_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("js")).unwrap();
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("js").join("rpg_core.js"), "// mv marker").unwrap();

        let resolved = resolve_game_dir(&root).unwrap();
        assert_eq!(resolved, root);

        let _ = fs::remove_dir_all(&resolved);
    }

    #[test]
    fn test_resolve_game_dir_nested_root() {
        let base = std::env::temp_dir().join(format!(
            "fairtl_resolve_nested_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let game = base.join("foo").join("bar").join("MyGame");
        fs::create_dir_all(game.join("js")).unwrap();
        fs::create_dir_all(game.join("Data")).unwrap();
        fs::write(game.join("js").join("rmmz_core.js"), "// mz marker").unwrap();

        let resolved = resolve_game_dir(&base).unwrap();
        assert_eq!(resolved, game);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_resolve_game_dir_multiple_candidates() {
        let base = std::env::temp_dir().join(format!(
            "fairtl_resolve_multiple_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let game_a = base.join("A").join("GameA");
        let game_b = base.join("B").join("GameB");

        fs::create_dir_all(game_a.join("js")).unwrap();
        fs::create_dir_all(game_a.join("data")).unwrap();
        fs::write(game_a.join("js").join("rpg_core.js"), "// mv marker").unwrap();

        fs::create_dir_all(game_b.join("js")).unwrap();
        fs::create_dir_all(game_b.join("Data")).unwrap();
        fs::write(game_b.join("js").join("rmmz_core.js"), "// mz marker").unwrap();

        let err = resolve_game_dir(&base).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Multiple RPG Maker game roots"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_resolve_game_dir_detects_legacy_root() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_resolve_legacy_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();
        fs::write(root.join("Data").join("System.rvdata2"), "legacy").unwrap();
        fs::write(root.join("Game.exe"), "stub").unwrap();

        let resolved = resolve_game_dir(&root).unwrap();
        assert_eq!(resolved, root);

        let _ = fs::remove_dir_all(&resolved);
    }

    #[test]
    fn test_detect_engine_identifies_legacy_engine() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_detect_legacy_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();
        fs::write(root.join("Data").join("System.rvdata2"), "legacy").unwrap();
        fs::write(root.join("Game.exe"), "stub").unwrap();

        let engine = detect_engine(&root).unwrap();
        assert_eq!(engine, Engine::VxAce);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_detect_engine_legacy_fallback_prioritizes_vxace_when_mixed_extensions() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_detect_legacy_mixed_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();
        // No System.* marker on purpose: detection must use extension fallback.
        fs::write(root.join("Data").join("A.rvdata"), "vx").unwrap();
        fs::write(root.join("Data").join("B.rxdata"), "xp").unwrap();
        fs::write(root.join("Data").join("C.rvdata2"), "vxace").unwrap();

        let engine = detect_engine(&root).unwrap();
        assert_eq!(engine, Engine::VxAce);

        let _ = fs::remove_dir_all(&root);
    }
}

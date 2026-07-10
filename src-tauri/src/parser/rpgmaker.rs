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

/// Visible vocabulary stored directly on System.json outside `terms`.
pub const SYSTEM_TEXT_FIELDS: &[&str] = &[
    "currencyUnit",
    "elements",
    "skillTypes",
    "weaponTypes",
    "armorTypes",
    "equipTypes",
];

/// Files in /data/ that are treated as database arrays
const DATABASE_FILES: &[&str] = &[
    "Actors", "Armors", "Classes", "Enemies", "Items", "Skills", "States", "Weapons",
];

/// Event command codes that contain translatable text
pub const CODE_SHOW_TEXT: i64 = 401;
pub const CODE_SHOW_TEXT_HEADER: i64 = 101;
pub const CODE_SHOW_CHOICES: i64 = 102;
pub const CODE_SCROLL_TEXT: i64 = 405;
pub const CODE_CHANGE_NAME: i64 = 320;
pub const CODE_CHANGE_NICKNAME: i64 = 324;
pub const CODE_CHANGE_PROFILE: i64 = 325;
pub const CODE_CONTROL_VARIABLES: i64 = 122;

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
    Plugins,
    Html,
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

pub fn classify_relative_path(path: &str) -> FileKind {
    let normalized = path.replace('\\', "/");
    if normalized.eq_ignore_ascii_case("js/plugins.js") {
        FileKind::Plugins
    } else if normalized.eq_ignore_ascii_case("index.html") {
        FileKind::Html
    } else {
        let stem = Path::new(&normalized)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        classify_file(stem)
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

pub(super) fn is_rpgmaker_root(dir: &Path) -> bool {
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
    let mut files = if !data_dir.is_dir() {
        // MV sometimes uses "Data" (capital D)
        let alt = game_dir.join("Data");
        if alt.is_dir() {
            scan_data_dir(&alt)?
        } else {
            return Err(AppError::NotFound(format!(
                "No 'data' directory found in {}",
                game_dir.display()
            )));
        }
    } else {
        scan_data_dir(&data_dir)?
    };

    if let Some(plugins_path) = mv_mz_plugins_path(game_dir) {
        files.push(plugins_path);
    }
    if let Some(index_path) = mv_mz_index_path(game_dir) {
        files.push(index_path);
    }

    files.sort_by(|a, b| {
        let rel_a = a
            .strip_prefix(game_dir)
            .unwrap_or(a)
            .to_string_lossy()
            .replace('\\', "/");
        let rel_b = b
            .strip_prefix(game_dir)
            .unwrap_or(b)
            .to_string_lossy()
            .replace('\\', "/");
        file_kind_sort_order(&classify_relative_path(&rel_a))
            .cmp(&file_kind_sort_order(&classify_relative_path(&rel_b)))
            .then(rel_a.cmp(&rel_b))
    });

    Ok(files)
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
                FileKind::Plugins => 1,
                FileKind::Database => 2,
                FileKind::CommonEvents => 3,
                FileKind::Troops => 4,
                FileKind::Map => 5,
                FileKind::Html => 6,
                FileKind::Ignored => 6,
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
        let rel_path = path
            .strip_prefix(game_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let kind = classify_relative_path(&rel_path);

        let content = std::fs::read_to_string(path)?;
        let mut json = match kind {
            FileKind::Plugins => parse_plugins_js(&content)?,
            FileKind::Html => Value::String(content),
            _ => serde_json::from_str(&content)?,
        };

        if kind == FileKind::Plugins {
            inject_plugin_source_defaults(&mut json, game_dir)?;
        }

        let entries = extract_entries(&stem, &kind, &json)?;

        if !entries.is_empty() {
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
        FileKind::Plugins => extract_plugins_entries(json),
        FileKind::Html => extract_index_html_entries(stem, json),
        FileKind::Ignored => Ok(Vec::new()),
    }
}

pub fn parse_plugins_js(content: &str) -> Result<Value, AppError> {
    let start = content
        .find('[')
        .ok_or_else(|| AppError::Parse("plugins.js did not contain a plugin array start".into()))?;
    let end = content
        .rfind(']')
        .ok_or_else(|| AppError::Parse("plugins.js did not contain a plugin array end".into()))?;
    let payload = &content[start..=end];
    serde_json::from_str(payload)
        .map_err(|error| AppError::Parse(format!("plugins.js payload was not valid JSON: {error}")))
}

pub fn render_plugins_js(json: &Value) -> Result<String, AppError> {
    Ok(format!(
        "var $plugins =\n{};\n",
        serde_json::to_string_pretty(json)?
    ))
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
// System.json — gameTitle + visible vocabulary + terms
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

    for field in SYSTEM_TEXT_FIELDS {
        if let Some(value) = obj.get(*field) {
            extract_terms_recursive(
                value,
                &format!("System:{field}"),
                &format!("System > {field}"),
                &mut entries,
            );
        }
    }

    // terms / terms.basic / terms.commands / terms.params / terms.messages
    if let Some(terms) = obj.get("terms") {
        extract_terms_recursive(terms, "System:terms", "System > terms", &mut entries);
    }

    Ok(entries)
}

fn extract_plugins_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let plugins = json
        .as_array()
        .ok_or_else(|| AppError::Parse("Plugins: expected JSON array".into()))?;

    let mut entries = Vec::new();

    for (plugin_index, plugin) in plugins.iter().enumerate() {
        let Some(plugin_obj) = plugin.as_object() else {
            continue;
        };
        let plugin_name = plugin_obj
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Plugin");

        if let Some(Value::String(description)) = plugin_obj.get("description") {
            if !description.trim().is_empty() {
                entries.push(make_entry(
                    &format!("Plugins:{plugin_index}:description"),
                    &format!("Plugins > #{plugin_index} ({plugin_name}) > description"),
                    description.clone(),
                ));
            }
        }

        if let Some(Value::Object(parameters)) = plugin_obj.get("parameters") {
            for (key, value) in parameters {
                extract_plugin_value(
                    value,
                    &format!("Plugins:{plugin_index}:parameters:{key}"),
                    &format!("Plugins > #{plugin_index} ({plugin_name}) > parameters > {key}"),
                    Some(key.as_str()),
                    &mut entries,
                );
            }
        }
    }

    Ok(entries)
}

fn extract_plugin_value(
    value: &Value,
    id_prefix: &str,
    context_prefix: &str,
    key_hint: Option<&str>,
    entries: &mut Vec<TranslationEntry>,
) {
    match value {
        Value::String(text) => {
            if let Some(parsed) = parse_embedded_plugin_json(text) {
                let before = entries.len();
                extract_plugin_value(
                    &parsed,
                    &format!("{id_prefix}:json"),
                    &format!("{context_prefix} > json"),
                    key_hint,
                    entries,
                );
                if entries.len() > before {
                    return;
                }
            }

            if should_extract_plugin_string(key_hint, text) {
                entries.push(make_entry(id_prefix, context_prefix, text.clone()));
            }
        }
        Value::Object(map) => {
            for (key, child) in map {
                extract_plugin_value(
                    child,
                    &format!("{id_prefix}:{key}"),
                    &format!("{context_prefix} > {key}"),
                    Some(key.as_str()),
                    entries,
                );
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                extract_plugin_value(
                    child,
                    &format!("{id_prefix}:{index}"),
                    &format!("{context_prefix}[{index}]"),
                    key_hint,
                    entries,
                );
            }
        }
        _ => {}
    }
}

fn parse_embedded_plugin_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn should_extract_plugin_string(key_hint: Option<&str>, text: &str) -> bool {
    if !plugin_string_is_translatable_candidate(text) {
        return false;
    }

    key_hint.map(plugin_key_is_translatable).unwrap_or(false)
}

fn plugin_string_is_translatable_candidate(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "true" | "false" | "on" | "off") {
        return false;
    }
    if trimmed.parse::<f64>().is_ok() {
        return false;
    }

    true
}

fn plugin_key_is_translatable(key: &str) -> bool {
    let trimmed = key.trim();
    let lower = trimmed.to_ascii_lowercase();
    let compact = lower
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();

    matches!(
        compact.as_str(),
        "description"
            | "name"
            | "parentname"
            | "endname"
            | "commandname"
            | "displayname"
            | "buttonname"
            | "text"
            | "helptext"
            | "commonhelptext"
            | "title"
            | "label"
            | "caption"
            | "message"
            | "maximummessage"
            | "completemessage"
            | "categoryhelp"
            | "confirmhelp"
            | "usinghelp"
            | "confirmuse"
            | "confirmnouse"
            | "optiontext"
            | "optionname"
    ) || compact.ends_with("title")
        || compact.ends_with("label")
        || compact.ends_with("caption")
        || compact.ends_with("help")
        || compact.ends_with("helptext")
        || compact.ends_with("message")
        || plugin_key_has_translatable_japanese_terms(trimmed)
}

fn plugin_key_has_translatable_japanese_terms(key: &str) -> bool {
    [
        "名称",
        "名前",
        "タイトル",
        "メッセージ",
        "テキスト",
        "ヘルプ",
    ]
    .iter()
    .any(|term| key.contains(term))
}

fn mv_mz_plugins_path(game_dir: &Path) -> Option<PathBuf> {
    ["plugins.js", "Plugins.js"]
        .into_iter()
        .map(|file_name| game_dir.join("js").join(file_name))
        .find(|path| path.is_file())
}

fn inject_plugin_source_defaults(json: &mut Value, game_dir: &Path) -> Result<(), AppError> {
    let Some(plugins) = json.as_array_mut() else {
        return Ok(());
    };

    for plugin in plugins {
        let Some(plugin_obj) = plugin.as_object_mut() else {
            continue;
        };
        let Some(plugin_name) = plugin_obj.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(source_path) = find_plugin_source_path(game_dir, plugin_name) else {
            continue;
        };

        let source_bytes = std::fs::read(&source_path)?;
        let source_content = String::from_utf8_lossy(&source_bytes);
        let defaults = extract_plugin_source_defaults(&source_content);
        if defaults.is_empty() {
            continue;
        }

        let parameters = plugin_obj
            .entry("parameters")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Some(parameters) = parameters.as_object_mut() else {
            continue;
        };

        for (key, default_value) in defaults {
            if !parameters.contains_key(&key)
                && plugin_key_is_translatable(&key)
                && plugin_string_is_translatable_candidate(&default_value)
            {
                parameters.insert(key, Value::String(default_value));
            }
        }
    }

    Ok(())
}

fn find_plugin_source_path(game_dir: &Path, plugin_name: &str) -> Option<PathBuf> {
    let plugins_dir = game_dir.join("js").join("plugins");
    let direct = plugins_dir.join(format!("{plugin_name}.js"));
    if direct.is_file() {
        return Some(direct);
    }

    let desired = format!("{}.js", plugin_name).to_ascii_lowercase();
    std::fs::read_dir(plugins_dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case(&desired))
                .unwrap_or(false)
        })
}

fn extract_plugin_source_defaults(content: &str) -> Vec<(String, String)> {
    let mut defaults = Vec::new();
    let mut current_param: Option<String> = None;

    for line in content.lines() {
        let Some(tag) = parse_plugin_doc_tag(line) else {
            continue;
        };

        if let Some(param) = tag.strip_prefix("param ") {
            current_param = Some(param.trim().to_string());
        } else if let Some(default_value) = tag.strip_prefix("default ") {
            if let Some(param) = current_param.take() {
                defaults.push((param, default_value.trim().to_string()));
            }
        } else if tag.starts_with("param") {
            current_param = None;
        }
    }

    defaults
}

fn parse_plugin_doc_tag(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let stripped = trimmed.strip_prefix('*')?.trim_start();
    stripped.strip_prefix('@')
}

fn mv_mz_index_path(game_dir: &Path) -> Option<PathBuf> {
    let path = game_dir.join("index.html");
    path.is_file().then_some(path)
}

fn file_kind_sort_order(kind: &FileKind) -> u8 {
    match kind {
        FileKind::System => 0,
        FileKind::Plugins => 1,
        FileKind::Html => 2,
        FileKind::Database => 3,
        FileKind::CommonEvents => 4,
        FileKind::Troops => 5,
        FileKind::Map => 6,
        FileKind::Ignored => 6,
    }
}

fn extract_index_html_entries(stem: &str, json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let content = json
        .as_str()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected HTML content string")))?;
    let Some(title) = extract_html_title(content) else {
        return Ok(Vec::new());
    };
    if title.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![make_entry(
        "index.html:title",
        "index.html > title",
        title,
    )])
}

fn extract_html_title(content: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()?;
    re.captures(content)
        .and_then(|captures| captures.get(1))
        .map(|title| title.as_str().to_string())
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
/// - 101: Show Text header speaker name (MZ parameters[4])
/// - 401: Show Text (continuation lines)
/// - 102: Show Choices
/// - 405: Scrolling Text (continuation lines)
/// - 320: Change Actor Name
/// - 324: Change Actor Nickname
/// - 325: Change Actor Profile
/// - 122: Control Variables with an exact quoted string literal
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
            CODE_SHOW_TEXT_HEADER => {
                if let Some(Value::Array(p)) = params {
                    if let Some(Value::String(speaker_name)) = p.get(4) {
                        if !speaker_name.trim().is_empty() {
                            entries.push(make_entry(
                                &format!("{id_prefix}:speakerName:{cmd_idx}"),
                                &format!("{context_prefix} > speakerName"),
                                speaker_name.clone(),
                            ));
                        }
                    }
                }
            }
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
            CODE_CONTROL_VARIABLES => {
                if let Some(Value::Array(p)) = params {
                    if p.get(3).and_then(Value::as_i64) == Some(4) {
                        if let Some(text) = p
                            .get(4)
                            .and_then(Value::as_str)
                            .and_then(parse_quoted_string_literal)
                        {
                            if !text.trim().is_empty() {
                                entries.push(make_entry(
                                    &format!("{id_prefix}:variableText:{cmd_idx}"),
                                    &format!("{context_prefix} > variable text"),
                                    text,
                                ));
                            }
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

fn parse_quoted_string_literal(value: &str) -> Option<String> {
    serde_json::from_str::<String>(value).ok()
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
        assert_eq!(classify_relative_path("js/plugins.js"), FileKind::Plugins);
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
    fn test_extract_event_commands_mz_speaker_name_keeps_text_grouping() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["Actor1", 0, 0, 2, "Alice"]},
                {"code": 401, "parameters": ["First line"]},
                {"code": 401, "parameters": ["Second line"]},
                {"code": 101, "parameters": ["Actor2", 1, 0, 2, ""]},
                {"code": 401, "parameters": ["Without a speaker"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "Map001:1:0",
            "Map001 > Event 1 > Page 1",
        );

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].id, "Map001:1:0:speakerName:0");
        assert_eq!(entries[0].original, "Alice");
        assert_eq!(entries[1].id, "Map001:1:0:text:1");
        assert_eq!(entries[1].original, "First line\nSecond line");
        assert_eq!(entries[2].id, "Map001:1:0:text:4");
        assert_eq!(entries[2].original, "Without a speaker");
    }

    #[test]
    fn test_extract_event_commands_mv_header_has_no_speaker_name() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 101, "parameters": ["Actor1", 0, 0, 2]},
                {"code": 401, "parameters": ["MV text"]},
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
        assert_eq!(entries[0].id, "Map001:1:0:text:1");
        assert_eq!(entries[0].original, "MV text");
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
    fn test_extract_event_commands_quoted_variable_text_only() {
        let list: Value = serde_json::from_str(
            r#"[
                {"code": 122, "parameters": [10, 10, 0, 4, "\"Waste Plant\""]},
                {"code": 122, "parameters": [11, 11, 0, 4, "$gameVariables.value(1) + 1"]},
                {"code": 0, "parameters": []}
            ]"#,
        )
        .unwrap();

        let entries = extract_event_commands(
            list.as_array().unwrap(),
            "CommonEvents:2",
            "CommonEvents > CommonEvent 2",
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "CommonEvents:2:variableText:0");
        assert_eq!(entries[0].original, "Waste Plant");
    }

    #[test]
    fn test_extract_system_entries() {
        let json: Value = serde_json::from_str(
            r#"{
                "gameTitle": "My RPG Adventure",
                "currencyUnit": "G",
                "elements": ["", "Fire", "Ice"],
                "skillTypes": ["", "Magic"],
                "weaponTypes": ["", "Sword"],
                "armorTypes": ["", "Shield"],
                "equipTypes": ["", "Weapon"],
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
        assert_eq!(entries.len(), 16);
        assert_eq!(entries[0].id, "System:gameTitle");
        assert_eq!(entries[0].original, "My RPG Adventure");
        let by_id = entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry.original.as_str()))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(by_id.get("System:currencyUnit"), Some(&"G"));
        assert_eq!(by_id.get("System:elements.1"), Some(&"Fire"));
        assert_eq!(by_id.get("System:elements.2"), Some(&"Ice"));
        assert_eq!(by_id.get("System:skillTypes.1"), Some(&"Magic"));
        assert_eq!(by_id.get("System:weaponTypes.1"), Some(&"Sword"));
        assert_eq!(by_id.get("System:armorTypes.1"), Some(&"Shield"));
        assert_eq!(by_id.get("System:equipTypes.1"), Some(&"Weapon"));
        assert!(!by_id.contains_key("System:elements.0"));
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
    fn test_extract_plugins_entries_from_stringified_json_parameters() {
        let json = parse_plugins_js(
            r#"var $plugins =
[
  {
    "name": "MenuSubCommand",
    "status": true,
    "description": "Menu plugin",
    "parameters": {
      "subCommands": "[\"{\\\"Name\\\":\\\"Character Introduction\\\",\\\"ParentName\\\":\\\"Character Introduction\\\"}\",\"{\\\"Name\\\":\\\"時間変更\\\",\\\"ParentName\\\":\\\"時間変更\\\"}\"]",
      "help": "{\"CommonHelpText\":\"Are you sure?\",\"CommandList\":\"[\\\"{\\\\\\\"Text\\\\\\\":\\\\\\\"Yes.\\\\\\\"}\\\",\\\"{\\\\\\\"Text\\\\\\\":\\\\\\\"No.\\\\\\\"}\\\"]\"}"
    }
  }
];"#,
        )
        .unwrap();

        let entries = extract_plugins_entries(&json).unwrap();
        let ids = entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.original.clone()))
            .collect::<Vec<_>>();

        assert!(ids.contains(&(
            "Plugins:0:description".to_string(),
            "Menu plugin".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:0:parameters:subCommands:json:0:json:Name".to_string(),
            "Character Introduction".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:0:parameters:subCommands:json:0:json:ParentName".to_string(),
            "Character Introduction".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:0:parameters:help:json:CommonHelpText".to_string(),
            "Are you sure?".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:0:parameters:help:json:CommandList:json:0:json:Text".to_string(),
            "Yes.".to_string()
        )));
    }

    #[test]
    fn test_extract_plugins_entries_from_visible_menu_parameters() {
        let json = parse_plugins_js(
            r#"var $plugins =
[
  {
    "name": "MessageWindowHidden",
    "status": true,
    "description": "",
    "parameters": {
      "ボタン名称": "右クリック",
      "連動ピクチャ番号": "50,52,53"
    }
  },
  {
    "name": "BB_CustomSaveWindow",
    "status": true,
    "description": "",
    "parameters": {
      "Item1title": "保乃花の陰毛",
      "Item2title": "操作キャラ",
      "Item3title": "現在地",
      "Item4title": "プレイ時間"
    }
  },
  {
    "name": "SceneGlossary",
    "status": true,
    "description": "",
    "parameters": {
      "GlossaryInfo": "[\"{\\\"CommandName\\\":\\\"その他\\\",\\\"BackPicture\\\":\\\"その他背景\\\",\\\"ConfirmUse\\\":\\\"使う\\\",\\\"ConfirmNoUse\\\":\\\"やめる\\\",\\\"CompleteMessage\\\":\\\"収集率 \\\\\\\\c[2]%1\\\\\\\\c[0] ％\\\",\\\"CategoryHelp\\\":\\\"カテゴリを選択してください。\\\"}\"]"
    }
  }
];"#,
        )
        .unwrap();

        let entries = extract_plugins_entries(&json).unwrap();
        let ids = entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.original.clone()))
            .collect::<Vec<_>>();

        assert!(ids.contains(&(
            "Plugins:0:parameters:ボタン名称".to_string(),
            "右クリック".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:1:parameters:Item1title".to_string(),
            "保乃花の陰毛".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:2:parameters:GlossaryInfo:json:0:json:CommandName".to_string(),
            "その他".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:2:parameters:GlossaryInfo:json:0:json:ConfirmUse".to_string(),
            "使う".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:2:parameters:GlossaryInfo:json:0:json:CompleteMessage".to_string(),
            "収集率 \\c[2]%1\\c[0] ％".to_string()
        )));
        assert!(ids.contains(&(
            "Plugins:2:parameters:GlossaryInfo:json:0:json:CategoryHelp".to_string(),
            "カテゴリを選択してください。".to_string()
        )));
        assert!(!ids.iter().any(|(id, _)| id.ends_with(":BackPicture")));
    }

    #[test]
    fn test_parse_mv_mz_game_extracts_index_html_title() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_index_title_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("js")).unwrap();
        fs::write(root.join("js").join("rpg_core.js"), "// mv marker").unwrap();
        fs::write(
            root.join("data").join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            root.join("index.html"),
            r#"<!doctype html><html><head><title>露出に目覚めた人妻は寝取られる ver1.02</title></head></html>"#,
        )
        .unwrap();

        let files = parse_mv_mz_game(&root).unwrap();
        let index_file = files
            .iter()
            .find(|file| file.relative_path == "index.html")
            .expect("index.html should be included when it has a title");

        assert_eq!(index_file.entries.len(), 1);
        assert_eq!(index_file.entries[0].id, "index.html:title");
        assert_eq!(
            index_file.entries[0].original,
            "露出に目覚めた人妻は寝取られる ver1.02"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_mv_mz_game_extracts_visible_plugin_default_parameters() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_plugin_default_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("js").join("plugins")).unwrap();
        fs::write(root.join("js").join("rpg_core.js"), "// mv marker").unwrap();
        fs::write(
            root.join("data").join("System.json"),
            r#"{"gameTitle":"Game Title","terms":{}}"#,
        )
        .unwrap();
        fs::write(
            root.join("js").join("plugins.js"),
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
        fs::write(
            root.join("js").join("plugins").join("gamEnd.js"),
            r#"/*:ja
 * @param endName
 * @desc ゲーム終了の表記です
 * @default ゲーム終了
 *
 * @param fileName
 * @default タイトル画像
 */"#,
        )
        .unwrap();

        let files = parse_mv_mz_game(&root).unwrap();
        let plugins_file = files
            .iter()
            .find(|file| file.relative_path == "js/plugins.js")
            .expect("plugins.js should be included");
        let ids = plugins_file
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), entry.original.clone()))
            .collect::<Vec<_>>();

        assert!(ids.contains(&(
            "Plugins:0:parameters:endName".to_string(),
            "ゲーム終了".to_string()
        )));
        assert!(!ids
            .iter()
            .any(|(id, original)| id.ends_with(":fileName") || original == "タイトル画像"));

        let _ = fs::remove_dir_all(&root);
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

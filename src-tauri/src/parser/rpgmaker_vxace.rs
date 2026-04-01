use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::AppError;
use crate::models::{EntryStatus, TranslationEntry, TranslationFile};
use crate::parser::rpgmaker::{
    CODE_CHANGE_NAME, CODE_CHANGE_NICKNAME, CODE_CHANGE_PROFILE, CODE_SCROLL_TEXT,
    CODE_SHOW_CHOICES, CODE_SHOW_TEXT,
};

const DATABASE_FILES: &[&str] = &[
    "Actors", "Armors", "Classes", "Enemies", "Items", "Skills", "States", "Weapons",
];

const LEGACY_DB_TEXT_FIELDS: &[(&str, &str)] = &[
    ("name", "@name"),
    ("description", "@description"),
    ("nickname", "@nickname"),
    ("message1", "@message1"),
    ("message2", "@message2"),
    ("message3", "@message3"),
    ("message4", "@message4"),
];

#[derive(Debug, Clone, PartialEq)]
enum FileKind {
    Database,
    Map,
    CommonEvents,
    Troops,
    System,
    Ignored,
}

pub fn parse_vx_ace_game(game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
    parse_legacy_game(game_dir, "rvdata2")
}

pub fn parse_vx_game(game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
    parse_legacy_game(game_dir, "rvdata")
}

pub fn parse_xp_game(game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
    parse_legacy_game(game_dir, "rxdata")
}

fn parse_legacy_game(game_dir: &Path, extension: &str) -> Result<Vec<TranslationFile>, AppError> {
    let rvdata_files = scan_legacy_game_dir(game_dir, extension)?;
    let mut translation_files = Vec::new();

    for path in rvdata_files {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let kind = classify_file(&stem);

        // Some real-world legacy builds ship placeholder map files with zero bytes.
        // Those should not abort the whole import; we skip them like an empty map.
        if kind == FileKind::Map
            && std::fs::metadata(&path)
                .map(|metadata| metadata.len() == 0)
                .unwrap_or(false)
        {
            continue;
        }

        let value = read_marshal_value(&path)?;
        let entries = extract_entries(&stem, &kind, &value)?;

        if !entries.is_empty() {
            let relative_path = path
                .strip_prefix(game_dir)
                .unwrap_or(path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            translation_files.push(TranslationFile {
                relative_path,
                entries,
            });
        }
    }

    Ok(translation_files)
}

fn data_dir_for(dir: &Path) -> Option<PathBuf> {
    let data = dir.join("Data");
    if data.is_dir() {
        return Some(data);
    }
    let lower = dir.join("data");
    if lower.is_dir() {
        return Some(lower);
    }
    None
}

fn scan_legacy_game_dir(game_dir: &Path, extension: &str) -> Result<Vec<PathBuf>, AppError> {
    let data_dir = data_dir_for(game_dir).ok_or_else(|| {
        AppError::NotFound(format!(
            "No 'Data' directory found in {}",
            game_dir.display()
        ))
    })?;

    let mut files: Vec<PathBuf> = std::fs::read_dir(data_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case(extension))
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

fn classify_file(stem: &str) -> FileKind {
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

fn extract_database_entries(stem: &str, json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected array for VX Ace database")))?;

    let mut entries = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        let fields = match object_fields(item) {
            Some(v) => v,
            None => continue,
        };

        let item_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);
        let item_name = string_from_value(fields.get("@name").unwrap_or(&Value::Null))
            .unwrap_or_else(|| "???".to_string());

        for (field_name, legacy_key) in LEGACY_DB_TEXT_FIELDS {
            if let Some(text) = fields.get(*legacy_key).and_then(string_from_value) {
                if text.trim().is_empty() {
                    continue;
                }
                entries.push(make_entry(
                    &format!("{stem}:{field_name}:{item_id}"),
                    &format!("{stem} > #{item_id} ({item_name}) > {field_name}"),
                    text,
                ));
            }
        }
    }

    Ok(entries)
}

fn extract_map_entries(stem: &str, json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let fields = object_fields(json)
        .ok_or_else(|| AppError::Parse(format!("{stem}: expected RPG::Map object")))?;

    let mut entries = Vec::new();

    if let Some(display_name) = fields
        .get("@display_name")
        .and_then(string_from_value)
        .filter(|s| !s.trim().is_empty())
    {
        entries.push(make_entry(
            &format!("{stem}:displayName"),
            &format!("{stem} > displayName"),
            display_name,
        ));
    }

    if let Some(events_hash) = fields.get("@events") {
        for (_, event) in hash_entries(events_hash) {
            let event_fields = match object_fields(event) {
                Some(v) => v,
                None => continue,
            };
            let ev_id = event_fields
                .get("@id")
                .and_then(Value::as_i64)
                .map(|v| v as u64)
                .unwrap_or(0);
            let ev_name = event_fields
                .get("@name")
                .and_then(string_from_value)
                .unwrap_or_default();
            let ev_label = if ev_name.is_empty() {
                format!("Event {ev_id}")
            } else {
                format!("Event {ev_id} ({ev_name})")
            };

            if let Some(pages) = event_fields.get("@pages").and_then(Value::as_array) {
                for (page_idx, page) in pages.iter().enumerate() {
                    let page_fields = match object_fields(page) {
                        Some(v) => v,
                        None => continue,
                    };
                    if let Some(list) = page_fields.get("@list").and_then(Value::as_array) {
                        let id_prefix = format!("{stem}:{ev_id}:{page_idx}");
                        let context_prefix = format!("{stem} > {ev_label} > Page {}", page_idx + 1);
                        entries.extend(extract_event_commands(list, &id_prefix, &context_prefix));
                    }
                }
            }
        }
    }

    Ok(entries)
}

fn extract_common_events_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse("CommonEvents: expected array".into()))?;

    let mut entries = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        let fields = match object_fields(item) {
            Some(v) => v,
            None => continue,
        };
        let ce_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);
        let ce_name = fields
            .get("@name")
            .and_then(string_from_value)
            .unwrap_or_default();
        let ce_label = if ce_name.is_empty() {
            format!("CommonEvent {ce_id}")
        } else {
            format!("CommonEvent {ce_id} ({ce_name})")
        };
        if let Some(list) = fields.get("@list").and_then(Value::as_array) {
            entries.extend(extract_event_commands(
                list,
                &format!("CommonEvents:{ce_id}"),
                &format!("CommonEvents > {ce_label}"),
            ));
        }
    }

    Ok(entries)
}

fn extract_troops_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let arr = json
        .as_array()
        .ok_or_else(|| AppError::Parse("Troops: expected array".into()))?;

    let mut entries = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        let fields = match object_fields(item) {
            Some(v) => v,
            None => continue,
        };
        let troop_id = fields
            .get("@id")
            .and_then(Value::as_i64)
            .map(|v| v as u64)
            .unwrap_or((idx + 1) as u64);
        let troop_name = fields
            .get("@name")
            .and_then(string_from_value)
            .unwrap_or_default();

        if !troop_name.trim().is_empty() {
            entries.push(make_entry(
                &format!("Troops:name:{troop_id}"),
                &format!("Troops > #{troop_id} ({troop_name}) > name"),
                troop_name.clone(),
            ));
        }

        if let Some(pages) = fields.get("@pages").and_then(Value::as_array) {
            for (page_idx, page) in pages.iter().enumerate() {
                let page_fields = match object_fields(page) {
                    Some(v) => v,
                    None => continue,
                };
                if let Some(list) = page_fields.get("@list").and_then(Value::as_array) {
                    let context_prefix = format!(
                        "Troops > #{troop_id} ({troop_name}) > Page {}",
                        page_idx + 1
                    );
                    let id_prefix = format!("Troops:{troop_id}:{page_idx}");
                    entries.extend(extract_event_commands(list, &id_prefix, &context_prefix));
                }
            }
        }
    }

    Ok(entries)
}

fn extract_system_entries(json: &Value) -> Result<Vec<TranslationEntry>, AppError> {
    let fields = object_fields(json)
        .ok_or_else(|| AppError::Parse("System: expected RPG::System object".into()))?;

    let mut entries = Vec::new();
    if let Some(title) = fields
        .get("@game_title")
        .and_then(string_from_value)
        .filter(|s| !s.trim().is_empty())
    {
        entries.push(make_entry("System:gameTitle", "System > gameTitle", title));
    }

    if let Some(terms) = fields.get("@terms") {
        extract_terms_recursive(terms, "System:terms", "System > terms", &mut entries);
    }

    Ok(entries)
}

fn extract_terms_recursive(
    value: &Value,
    id_prefix: &str,
    context_prefix: &str,
    entries: &mut Vec<TranslationEntry>,
) {
    if let Some(text) = string_from_value(value).filter(|s| !s.trim().is_empty()) {
        entries.push(make_entry(id_prefix, context_prefix, text));
        return;
    }

    if let Some(arr) = value.as_array() {
        for (i, val) in arr.iter().enumerate() {
            extract_terms_recursive(
                val,
                &format!("{id_prefix}.{i}"),
                &format!("{context_prefix}[{i}]"),
                entries,
            );
        }
        return;
    }

    if let Some(fields) = object_fields(value) {
        for (key, val) in fields {
            let clean_key = key.trim_start_matches('@');
            extract_terms_recursive(
                val,
                &format!("{id_prefix}.{clean_key}"),
                &format!("{context_prefix} > {clean_key}"),
                entries,
            );
        }
    }
}

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
        let code = command_code(cmd).unwrap_or(0);

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
                if let Some(line) = command_parameter(cmd, 0).and_then(string_from_value) {
                    if text_start_idx.is_none() {
                        text_start_idx = Some(cmd_idx);
                    }
                    text_accumulator.push(line);
                }
            }
            CODE_SHOW_CHOICES => {
                if let Some(choices) = command_parameter(cmd, 0).and_then(Value::as_array) {
                    for (ci, choice) in choices.iter().enumerate() {
                        if let Some(text) =
                            string_from_value(choice).filter(|s| !s.trim().is_empty())
                        {
                            entries.push(make_entry(
                                &format!("{id_prefix}:choice:{cmd_idx}:{ci}"),
                                &format!(
                                    "{context_prefix} > choice #{} option {}",
                                    entries.len() + 1,
                                    ci + 1
                                ),
                                text,
                            ));
                        }
                    }
                }
            }
            CODE_SCROLL_TEXT => {
                if let Some(line) = command_parameter(cmd, 0).and_then(string_from_value) {
                    if scroll_start_idx.is_none() {
                        scroll_start_idx = Some(cmd_idx);
                    }
                    scroll_accumulator.push(line);
                }
            }
            CODE_CHANGE_NAME => {
                if let Some(text) = command_parameter(cmd, 1)
                    .and_then(string_from_value)
                    .filter(|s| !s.trim().is_empty())
                {
                    entries.push(make_entry(
                        &format!("{id_prefix}:changeName:{cmd_idx}"),
                        &format!("{context_prefix} > changeName"),
                        text,
                    ));
                }
            }
            CODE_CHANGE_NICKNAME => {
                if let Some(text) = command_parameter(cmd, 1)
                    .and_then(string_from_value)
                    .filter(|s| !s.trim().is_empty())
                {
                    entries.push(make_entry(
                        &format!("{id_prefix}:changeNickname:{cmd_idx}"),
                        &format!("{context_prefix} > changeNickname"),
                        text,
                    ));
                }
            }
            CODE_CHANGE_PROFILE => {
                if let Some(text) = command_parameter(cmd, 1)
                    .and_then(string_from_value)
                    .filter(|s| !s.trim().is_empty())
                {
                    entries.push(make_entry(
                        &format!("{id_prefix}:changeProfile:{cmd_idx}"),
                        &format!("{context_prefix} > changeProfile"),
                        text,
                    ));
                }
            }
            _ => {}
        }
    }

    if !text_accumulator.is_empty() {
        entries.push(make_entry(
            &format!("{}:text:{}", id_prefix, text_start_idx.unwrap_or(0)),
            &format!("{context_prefix} > text #{}", entries.len() + 1),
            text_accumulator.join("\n"),
        ));
    }
    if !scroll_accumulator.is_empty() {
        entries.push(make_entry(
            &format!("{}:scroll:{}", id_prefix, scroll_start_idx.unwrap_or(0)),
            &format!("{context_prefix} > scroll #{}", entries.len() + 1),
            scroll_accumulator.join("\n"),
        ));
    }

    entries
}

fn object_fields(value: &Value) -> Option<&Map<String, Value>> {
    value
        .get("$object")
        .and_then(|o| o.get("fields"))
        .and_then(Value::as_object)
}

fn hash_entries(value: &Value) -> Vec<(&Value, &Value)> {
    value
        .get("$hash")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let key = entry.get("key")?;
                    let value = entry.get("value")?;
                    Some((key, value))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn command_code(cmd: &Value) -> Option<i64> {
    object_fields(cmd)?.get("@code").and_then(Value::as_i64)
}

fn command_parameter(cmd: &Value, idx: usize) -> Option<&Value> {
    object_fields(cmd)?
        .get("@parameters")
        .and_then(Value::as_array)
        .and_then(|params| params.get(idx))
}

fn string_from_value(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }

    if let Some(instance_value) = value.get("$instance").and_then(|v| v.get("value")) {
        return string_from_value(instance_value);
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

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::models::{Engine, TranslationFile};

pub mod rpgmaker;
pub mod rpgmaker_vxace;
pub mod unity;

/// Unified output for parser strategies.
#[derive(Debug)]
pub struct ParsedGame {
    pub engine: Engine,
    pub files: Vec<TranslationFile>,
}

trait EngineParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError>;
}

struct MvMzParser;
struct VxAceParser;
struct VxParser;
struct XpParser;
struct UnityParser;

impl EngineParser for MvMzParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
        rpgmaker::parse_mv_mz_game(game_dir)
    }
}

impl EngineParser for VxAceParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
        rpgmaker_vxace::parse_vx_ace_game(game_dir)
    }
}

impl EngineParser for VxParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
        rpgmaker_vxace::parse_vx_game(game_dir)
    }
}

impl EngineParser for XpParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
        rpgmaker_vxace::parse_xp_game(game_dir)
    }
}

impl EngineParser for UnityParser {
    fn parse(&self, game_dir: &Path) -> Result<Vec<TranslationFile>, AppError> {
        unity::parse_unity_game(game_dir)
    }
}

static MV_MZ_PARSER: MvMzParser = MvMzParser;
static VXACE_PARSER: VxAceParser = VxAceParser;
static VX_PARSER: VxParser = VxParser;
static XP_PARSER: XpParser = XpParser;
static UNITY_PARSER: UnityParser = UnityParser;

const MAX_GAME_ROOT_SEARCH_DEPTH: usize = 8;
const MAX_GAME_ROOT_SCAN_DIRS: usize = 20_000;

fn parser_for_engine(engine: &Engine) -> Result<&'static dyn EngineParser, AppError> {
    match engine {
        Engine::Mv | Engine::Mz => Ok(&MV_MZ_PARSER),
        Engine::VxAce => Ok(&VXACE_PARSER),
        Engine::Vx => Ok(&VX_PARSER),
        Engine::Xp => Ok(&XP_PARSER),
        Engine::Unity => Ok(&UNITY_PARSER),
    }
}

/// Resolve game directory from the selected path.
pub fn resolve_game_dir(selected_dir: &Path) -> Result<PathBuf, AppError> {
    if engine_at_root(selected_dir)?.is_some() {
        return Ok(selected_dir.to_path_buf());
    }

    let mut queue = VecDeque::from([(selected_dir.to_path_buf(), 0usize)]);
    let mut scanned_dirs = 0usize;
    let mut candidates = Vec::new();

    while let Some((directory, depth)) = queue.pop_front() {
        if depth >= MAX_GAME_ROOT_SEARCH_DEPTH {
            continue;
        }

        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink()
                || metadata_is_reparse_point(&metadata)
                || !metadata.is_dir()
            {
                continue;
            }

            scanned_dirs += 1;
            if scanned_dirs > MAX_GAME_ROOT_SCAN_DIRS {
                return Err(AppError::Project(format!(
                    "Search limit reached while scanning '{}'. Please select the exact game folder.",
                    selected_dir.display()
                )));
            }

            if engine_at_root(&path)?.is_some() {
                candidates.push(path);
            } else {
                queue.push_back((path, depth + 1));
            }
        }
    }

    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(AppError::NotFound(format!(
            "No supported game root found under '{}'. Select an RPG Maker game folder or an experimental Unity Loose JSON v0 Windows Mono build.",
            selected_dir.display()
        ))),
        _ => {
            candidates.sort();
            let shown = candidates
                .iter()
                .take(5)
                .map(|path| format!("- {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n");
            let remaining = candidates.len().saturating_sub(5);
            let suffix = if remaining == 0 {
                String::new()
            } else {
                format!("\n- and {remaining} more")
            };
            Err(AppError::Project(format!(
                "Multiple supported game roots were found under '{}'. Please select one game folder directly:\n{shown}{suffix}",
                selected_dir.display()
            )))
        }
    }
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

/// Detect game engine for the resolved game directory.
pub fn detect_engine(game_dir: &Path) -> Result<Engine, AppError> {
    let rpg_engine = rpgmaker::detect_engine(game_dir).ok();
    let unity = unity::is_unity_game_root(game_dir)?;

    match (rpg_engine, unity) {
        (Some(_), true) => Err(AppError::Project(format!(
            "Ambiguous game root '{}': both RPG Maker and Unity markers were found.",
            game_dir.display()
        ))),
        (Some(engine), false) => Ok(engine),
        (None, true) => Ok(Engine::Unity),
        (None, false) => Err(AppError::NotFound(format!(
            "Could not detect a supported game engine under '{}'. Expected RPG Maker markers or a Unity Windows build root.",
            game_dir.display()
        ))),
    }
}

fn engine_at_root(game_dir: &Path) -> Result<Option<Engine>, AppError> {
    let rpg_engine = if rpgmaker::is_rpgmaker_root(game_dir) {
        Some(rpgmaker::detect_engine(game_dir)?)
    } else {
        None
    };
    let unity = unity::is_unity_game_root(game_dir)?;

    match (rpg_engine, unity) {
        (Some(_), true) => Err(AppError::Project(format!(
            "Ambiguous game root '{}': both RPG Maker and Unity markers were found.",
            game_dir.display()
        ))),
        (Some(engine), false) => Ok(Some(engine)),
        (None, true) => Ok(Some(Engine::Unity)),
        (None, false) => Ok(None),
    }
}

/// Parse game with an engine-specific strategy.
pub fn parse_game(game_dir: &Path) -> Result<ParsedGame, AppError> {
    let engine = detect_engine(game_dir)?;
    let parser = parser_for_engine(&engine)?;
    let files = parser.parse(game_dir)?;

    Ok(ParsedGame { engine, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpgtool_common::{conv_write, Format};
    use serde_json::json;
    use std::fs;
    use std::io::BufWriter;

    fn write_marshal(path: &Path, json_value: serde_json::Value) {
        let value: rpgtool_common::Value = serde_json::from_value(json_value).unwrap();
        let file = std::fs::File::create(path).unwrap();
        let mut writer = BufWriter::new(file);
        conv_write(value, Format::Marshal, &mut writer).unwrap();
    }

    fn write_json(path: &Path, json_value: serde_json::Value) {
        let content = serde_json::to_string_pretty(&json_value).unwrap();
        fs::write(path, content).unwrap();
    }

    fn write_unity_mono_fixture(root: &Path, localization: serde_json::Value) {
        let data_dir = root.join("Sample_Data");
        fs::create_dir_all(data_dir.join("Managed")).unwrap();
        fs::create_dir_all(data_dir.join("StreamingAssets/Localization")).unwrap();
        fs::write(root.join("UnityPlayer.dll"), []).unwrap();
        fs::write(data_dir.join("globalgamemanagers"), []).unwrap();
        fs::write(data_dir.join("Managed/Assembly-CSharp.dll"), []).unwrap();
        write_json(
            &data_dir.join("StreamingAssets/Localization/strings.json"),
            localization,
        );
    }

    fn assert_parsed_contains_ids(parsed: &ParsedGame, expected_ids: &[&str]) {
        let mut ids = parsed
            .files
            .iter()
            .flat_map(|file| file.entries.iter().map(|entry| entry.id.clone()))
            .collect::<Vec<_>>();
        ids.sort();

        for expected_id in expected_ids {
            assert!(
                ids.iter().any(|id| id == expected_id),
                "Expected parsed IDs to include {expected_id}, got {ids:?}"
            );
        }
    }

    #[test]
    fn resolves_detects_and_parses_nested_unity_loose_json_game() {
        let selected = std::env::temp_dir().join(format!(
            "fairtl_parser_unity_nested_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let root = selected.join("download").join("SampleGame");
        write_unity_mono_fixture(&root, json!({"menu.play": "Play"}));

        let resolved = resolve_game_dir(&selected).unwrap();
        assert_eq!(resolved, root);
        assert_eq!(detect_engine(&resolved).unwrap(), Engine::Unity);

        let parsed = parse_game(&resolved).unwrap();
        assert_eq!(parsed.engine, Engine::Unity);
        assert_parsed_contains_ids(&parsed, &["UnityJson:/menu.play"]);

        let _ = fs::remove_dir_all(selected);
    }

    #[test]
    fn rejects_parent_with_rpg_maker_and_unity_game_roots() {
        let selected = std::env::temp_dir().join(format!(
            "fairtl_parser_mixed_roots_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        let rpg_root = selected.join("RpgGame");
        fs::create_dir_all(rpg_root.join("js")).unwrap();
        fs::create_dir_all(rpg_root.join("data")).unwrap();
        fs::write(rpg_root.join("js/rpg_core.js"), []).unwrap();
        write_unity_mono_fixture(&selected.join("UnityGame"), json!({"play": "Play"}));

        let error = resolve_game_dir(&selected).unwrap_err().to_string();
        assert!(error.contains("Multiple supported game roots"));
        assert!(error.contains("RpgGame"));
        assert!(error.contains("UnityGame"));

        let _ = fs::remove_dir_all(selected);
    }

    #[test]
    fn detects_il2cpp_root_then_reports_the_unsupported_backend() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_unity_il2cpp_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        write_unity_mono_fixture(&root, json!({"play": "Play"}));
        fs::write(root.join("GameAssembly.dll"), []).unwrap();

        assert_eq!(detect_engine(&root).unwrap(), Engine::Unity);
        let error = parse_game(&root).unwrap_err().to_string();
        assert!(error.contains("IL2CPP"));
        assert!(error.contains("Windows Mono"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_parse_game_mv_fixture_pack() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_mv_fixture_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("js")).unwrap();
        fs::write(root.join("js").join("rpg_core.js"), "RPG Maker MV").unwrap();

        write_json(
            &root.join("data").join("Actors.json"),
            json!([
                null,
                {
                    "id": 1,
                    "name": "Harold",
                    "description": "A brave actor",
                    "nickname": "Hero"
                }
            ]),
        );

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::Mv);
        assert!(parsed
            .files
            .iter()
            .any(|file| file.relative_path == "data/Actors.json"));
        assert_parsed_contains_ids(
            &parsed,
            &["Actors:name:1", "Actors:description:1", "Actors:nickname:1"],
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_game_mz_fixture_pack() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_mz_fixture_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("js")).unwrap();
        fs::write(root.join("js").join("rmmz_core.js"), "RPG Maker MZ").unwrap();

        write_json(
            &root.join("data").join("System.json"),
            json!({
                "gameTitle": "MZ Fixture",
                "terms": {
                    "basic": ["", "Level"]
                }
            }),
        );

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::Mz);
        assert!(parsed
            .files
            .iter()
            .any(|file| file.relative_path == "data/System.json"));
        assert_parsed_contains_ids(&parsed, &["System:gameTitle", "System:terms.basic.1"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_game_vxace_basic_entries() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_vxace_basic_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();

        write_marshal(
            &root.join("Data").join("System.rvdata2"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "Legacy Game", "fields": { "E": true } } },
                  "@terms": {
                    "$object": {
                      "class": "RPG::System::Terms",
                      "fields": {
                        "@basic": [
                          { "$instance": { "value": "", "fields": { "E": true } } },
                          { "$instance": { "value": "Level", "fields": { "E": true } } }
                        ]
                      }
                    }
                  }
                }
              }
            }),
        );
        write_marshal(
            &root.join("Data").join("Actors.rvdata2"),
            json!([
              {
                "$object": {
                  "class": "RPG::Actor",
                  "fields": {
                    "@id": 1,
                    "@name": { "$instance": { "value": "Hero", "fields": { "E": true } } },
                    "@description": { "$instance": { "value": "Brave warrior", "fields": { "E": true } } },
                    "@nickname": { "$instance": { "value": "The Brave", "fields": { "E": true } } }
                  }
                }
              }
            ]),
        );

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::VxAce);
        assert!(!parsed.files.is_empty());

        let mut ids = parsed
            .files
            .iter()
            .flat_map(|f| f.entries.iter().map(|e| e.id.clone()))
            .collect::<Vec<_>>();
        ids.sort();

        assert!(ids.iter().any(|id| id == "Actors:name:1"));
        assert!(ids.iter().any(|id| id == "System:gameTitle"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_game_vx_basic_entries() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_vx_basic_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();

        write_marshal(
            &root.join("Data").join("System.rvdata"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "VX Game", "fields": { "E": true } } }
                }
              }
            }),
        );
        write_marshal(
            &root.join("Data").join("Actors.rvdata"),
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

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::Vx);
        assert!(!parsed.files.is_empty());

        let mut ids = parsed
            .files
            .iter()
            .flat_map(|f| f.entries.iter().map(|e| e.id.clone()))
            .collect::<Vec<_>>();
        ids.sort();
        assert!(ids.iter().any(|id| id == "Actors:name:1"));
        assert!(ids.iter().any(|id| id == "System:gameTitle"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_game_vxace_skips_zero_byte_map_files() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_vxace_zero_map_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();

        write_marshal(
            &root.join("Data").join("System.rvdata2"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "Zero Map Game", "fields": { "E": true } } }
                }
              }
            }),
        );
        fs::write(root.join("Data").join("Map002.rvdata2"), []).unwrap();

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::VxAce);
        assert!(parsed
            .files
            .iter()
            .any(|file| file.relative_path == "Data/System.rvdata2"));
        assert!(!parsed
            .files
            .iter()
            .any(|file| file.relative_path == "Data/Map002.rvdata2"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_game_xp_basic_entries() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_parser_xp_basic_{}",
            uuid::Uuid::new_v4().as_simple()
        ));
        fs::create_dir_all(root.join("Data")).unwrap();

        write_marshal(
            &root.join("Data").join("System.rxdata"),
            json!({
              "$object": {
                "class": "RPG::System",
                "fields": {
                  "@game_title": { "$instance": { "value": "XP Game", "fields": { "E": true } } }
                }
              }
            }),
        );
        write_marshal(
            &root.join("Data").join("Actors.rxdata"),
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

        let parsed = parse_game(&root).unwrap();
        assert_eq!(parsed.engine, Engine::Xp);
        assert!(!parsed.files.is_empty());

        let mut ids = parsed
            .files
            .iter()
            .flat_map(|f| f.entries.iter().map(|e| e.id.clone()))
            .collect::<Vec<_>>();
        ids.sort();
        assert!(ids.iter().any(|id| id == "Actors:name:1"));
        assert!(ids.iter().any(|id| id == "System:gameTitle"));

        let _ = fs::remove_dir_all(&root);
    }
}

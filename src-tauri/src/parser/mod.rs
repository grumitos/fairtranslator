use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::{Engine, TranslationFile};

pub mod rpgmaker;
pub mod rpgmaker_vxace;
pub(crate) mod script_strings;
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

/// Encrypted RGSS archives: a game shipped this way keeps its data inside the
/// archive instead of a `Data` folder.
fn packed_archive_engine(path: &Path) -> Option<Engine> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "rgssad" => Some(Engine::Xp),
        "rgss2a" => Some(Engine::Vx),
        "rgss3a" => Some(Engine::VxAce),
        _ => None,
    }
}

/// An encrypted RGSS archive directly inside `dir`, if any.
fn packed_archive_in(dir: &Path) -> Option<(PathBuf, Engine)> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        let engine = packed_archive_engine(&path)?;
        path.is_file().then_some((path, engine))
    })
}

fn engine_label(engine: &Engine) -> &'static str {
    match engine {
        Engine::Mv => "RPG Maker MV",
        Engine::Mz => "RPG Maker MZ",
        Engine::VxAce => "RPG Maker VX Ace",
        Engine::Vx => "RPG Maker VX",
        Engine::Xp => "RPG Maker XP",
        Engine::Unity => "Unity",
    }
}

fn packed_game_error(archive: &Path, engine: &Engine) -> AppError {
    let folder = archive.parent().unwrap_or(archive);
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    AppError::Project(format!(
        "'{}' is a packed {} game: its data is inside the encrypted archive '{name}', which FairTranslator does not read. Select a copy of the game with its Data folder unpacked.",
        folder.display(),
        engine_label(engine)
    ))
}

/// Resolve game directory from the selected path.
pub fn resolve_game_dir(selected_dir: &Path) -> Result<PathBuf, AppError> {
    match locate_game(selected_dir)? {
        LocatedGame::Root(root) => Ok(root),
        LocatedGame::Packed { archive, engine } => Err(packed_game_error(&archive, &engine)),
        LocatedGame::Nothing => Err(AppError::NotFound(format!(
            "No supported game root found under '{}'. Select an RPG Maker game folder or an experimental Unity Loose JSON v0 Windows Mono build.",
            selected_dir.display()
        ))),
        LocatedGame::Several(mut candidates) => {
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

enum LocatedGame {
    Root(PathBuf),
    /// No unpacked game, but an encrypted archive was found.
    Packed {
        archive: PathBuf,
        engine: Engine,
    },
    Nothing,
    Several(Vec<PathBuf>),
}

/// Bounded search for the game root under the selected folder.
fn locate_game(selected_dir: &Path) -> Result<LocatedGame, AppError> {
    if engine_at_root(selected_dir)?.is_some() {
        return Ok(LocatedGame::Root(selected_dir.to_path_buf()));
    }

    let mut queue = VecDeque::from([(selected_dir.to_path_buf(), 0usize)]);
    let mut scanned_dirs = 0usize;
    let mut candidates = Vec::new();
    // Only reported when no unpacked game is found.
    let mut packed = None;

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
            if packed.is_none() && metadata.is_file() {
                packed = packed_archive_engine(&path).map(|engine| (path.clone(), engine));
            }
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

    Ok(match (candidates.len(), packed) {
        (1, _) => LocatedGame::Root(candidates.remove(0)),
        (0, Some((archive, engine))) => LocatedGame::Packed { archive, engine },
        (0, None) => LocatedGame::Nothing,
        _ => LocatedGame::Several(candidates),
    })
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

/// Why no project can be created from an inspected folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameBlockReason {
    /// Only an encrypted RGSS archive, no unpacked data.
    PackedArchive,
    NoGame,
    /// Several games under the folder; the user must pick one.
    MultipleGames,
}

/// What `project_create` would find in a folder, without writing anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInspection {
    /// Folder a project would use (for a packed game, the archive's folder).
    pub game_root: Option<String>,
    pub engine: Option<Engine>,
    /// Encrypted RGSS archive in the game folder (file name), if any. Next to
    /// unpacked data it is a caveat: the game may keep reading the archive.
    pub packed_archive: Option<String>,
    /// `None` when a project can be created.
    pub block_reason: Option<GameBlockReason>,
    /// Data files of the engine's format in the data folder (for Unity, the
    /// files with text).
    pub data_files: usize,
    /// Files with translatable text and their entries, from a full parse
    /// (well under a second on the test corpus); `None` when blocked.
    pub text_files: Option<usize>,
    pub entries: Option<usize>,
}

impl GameInspection {
    fn blocked(reason: GameBlockReason) -> Self {
        Self {
            game_root: None,
            engine: None,
            packed_archive: None,
            block_reason: Some(reason),
            data_files: 0,
            text_files: None,
            entries: None,
        }
    }
}

fn file_name_string(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

/// Inspects a selected folder the way `project_create` resolves it: reads
/// only, and reports why a project cannot be created instead of failing.
/// Errors are left for unreadable folders and invalid game data.
pub fn inspect_game(selected_dir: &Path) -> Result<GameInspection, AppError> {
    if !selected_dir.is_dir() {
        return Err(AppError::NotFound(format!(
            "Game folder not found: {}",
            selected_dir.display()
        )));
    }
    let game_root = match locate_game(selected_dir)? {
        LocatedGame::Root(root) => root,
        LocatedGame::Packed { archive, engine } => {
            return Ok(GameInspection {
                game_root: archive
                    .parent()
                    .map(|folder| folder.to_string_lossy().into_owned()),
                engine: Some(engine),
                packed_archive: file_name_string(&archive),
                ..GameInspection::blocked(GameBlockReason::PackedArchive)
            });
        }
        LocatedGame::Nothing => return Ok(GameInspection::blocked(GameBlockReason::NoGame)),
        LocatedGame::Several(_) => {
            return Ok(GameInspection::blocked(GameBlockReason::MultipleGames))
        }
    };

    let parsed = parse_game(&game_root)?;
    let extension = match parsed.engine {
        Engine::Mv | Engine::Mz => Some("json"),
        Engine::VxAce => Some("rvdata2"),
        Engine::Vx => Some("rvdata"),
        Engine::Xp => Some("rxdata"),
        Engine::Unity => None,
    };
    let data_files = match extension {
        Some(extension) => rpgmaker::data_dir_for(&game_root)
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        entry
                            .path()
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
                    })
                    .count()
            })
            .unwrap_or(0),
        None => parsed.files.len(),
    };
    Ok(GameInspection {
        game_root: Some(game_root.to_string_lossy().into_owned()),
        engine: Some(parsed.engine),
        packed_archive: packed_archive_in(&game_root)
            .and_then(|(archive, _)| file_name_string(&archive)),
        block_reason: None,
        data_files,
        text_files: Some(parsed.files.len()),
        entries: Some(parsed.files.iter().map(|file| file.entries.len()).sum()),
    })
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

    fn file_tree(root: &Path) -> Vec<(PathBuf, u64)> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push((path.clone(), fs::metadata(&path).unwrap().len()));
                }
            }
        }
        files.sort();
        files
    }

    /// `game_inspect` resolves folders like `project_create`, only reads, and
    /// reports why a folder cannot be used; a packed game makes project
    /// creation fail with a clear message instead of "no game found".
    #[test]
    fn inspect_reports_games_packed_archives_and_blocking_reasons() {
        let root = std::env::temp_dir().join(format!(
            "fairtl_inspect_{}",
            uuid::Uuid::new_v4().as_simple()
        ));

        // MV deployed layout, selected from its parent folder.
        let www = root.join("mv").join("Game").join("www");
        fs::create_dir_all(www.join("data")).unwrap();
        fs::create_dir_all(www.join("js")).unwrap();
        fs::write(www.join("js").join("rpg_core.js"), "RPG Maker MV").unwrap();
        write_json(
            &www.join("data").join("Actors.json"),
            json!([null, {"id": 1, "name": "Harold", "nickname": "Hero"}]),
        );
        write_json(&www.join("data").join("Animations.json"), json!([null]));

        // Packed VX Ace game: only the encrypted archive.
        let packed = root.join("packed").join("Game");
        fs::create_dir_all(&packed).unwrap();
        fs::write(packed.join("Game.ini"), "[Game]").unwrap();
        fs::write(packed.join("Game.rgss3a"), "RGSSAD").unwrap();

        // VX Ace game shipping both unpacked data and the archive.
        let both = root.join("both");
        fs::create_dir_all(both.join("Data")).unwrap();
        fs::write(both.join("Game.rgss3a"), "RGSSAD").unwrap();
        write_marshal(
            &both.join("Data").join("System.rvdata2"),
            json!({"$object": {"class": "RPG::System", "fields": {
                "@game_title": {"$instance": {"value": "Both", "fields": {"E": true}}}
            }}}),
        );

        fs::create_dir_all(root.join("empty")).unwrap();
        let before = file_tree(&root);

        let mv = inspect_game(&root.join("mv")).unwrap();
        assert_eq!(mv.block_reason, None);
        assert_eq!(mv.engine, Some(Engine::Mv));
        assert_eq!(
            mv.game_root.as_deref(),
            Some(www.to_string_lossy().as_ref())
        );
        assert_eq!(mv.packed_archive, None);
        assert_eq!(mv.data_files, 2);
        assert_eq!(mv.text_files, Some(1));
        assert_eq!(mv.entries, Some(2));

        let packed_report = inspect_game(&root.join("packed")).unwrap();
        assert_eq!(
            packed_report.block_reason,
            Some(GameBlockReason::PackedArchive)
        );
        assert_eq!(packed_report.engine, Some(Engine::VxAce));
        assert_eq!(packed_report.packed_archive.as_deref(), Some("Game.rgss3a"));
        assert_eq!(
            packed_report.game_root.as_deref(),
            Some(packed.to_string_lossy().as_ref())
        );
        assert_eq!(
            (packed_report.text_files, packed_report.entries),
            (None, None)
        );
        let error = resolve_game_dir(&root.join("packed"))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("packed RPG Maker VX Ace game") && error.contains("Game.rgss3a"),
            "{error}"
        );

        let both_report = inspect_game(&both).unwrap();
        assert_eq!(both_report.block_reason, None);
        assert_eq!(both_report.engine, Some(Engine::VxAce));
        assert_eq!(both_report.packed_archive.as_deref(), Some("Game.rgss3a"));
        assert_eq!(both_report.data_files, 1);

        assert_eq!(
            inspect_game(&root.join("empty")).unwrap().block_reason,
            Some(GameBlockReason::NoGame)
        );
        assert_eq!(
            inspect_game(&root).unwrap().block_reason,
            Some(GameBlockReason::MultipleGames)
        );
        assert_eq!(
            serde_json::to_value(&packed_report).unwrap()["blockReason"],
            json!("packed_archive")
        );

        assert_eq!(file_tree(&root), before);
        let _ = fs::remove_dir_all(&root);
    }
}

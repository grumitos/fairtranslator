use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::models::{Engine, TranslationFile};

pub mod rpgmaker;
pub mod rpgmaker_vxace;

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

static MV_MZ_PARSER: MvMzParser = MvMzParser;
static VXACE_PARSER: VxAceParser = VxAceParser;
static VX_PARSER: VxParser = VxParser;
static XP_PARSER: XpParser = XpParser;

fn parser_for_engine(engine: &Engine) -> Result<&'static dyn EngineParser, AppError> {
    match engine {
        Engine::Mv | Engine::Mz => Ok(&MV_MZ_PARSER),
        Engine::VxAce => Ok(&VXACE_PARSER),
        Engine::Vx => Ok(&VX_PARSER),
        Engine::Xp => Ok(&XP_PARSER),
    }
}

/// Resolve game directory from the selected path.
pub fn resolve_game_dir(selected_dir: &Path) -> Result<PathBuf, AppError> {
    rpgmaker::resolve_game_dir(selected_dir)
}

/// Detect game engine for the resolved game directory.
pub fn detect_engine(game_dir: &Path) -> Result<Engine, AppError> {
    rpgmaker::detect_engine(game_dir)
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

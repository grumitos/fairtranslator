//! On-screen text fitting for RPG Maker exports.
//!
//! RPG Maker never word-wraps message text: a line wider than its window is
//! drawn past the edge and clipped. This module estimates rendered widths per
//! engine and wraps translations at word boundaries so every line fits. Extra
//! lines become additional commands that MV, MZ, VX Ace and VX page natively;
//! XP has no native paging, so the exporter turns them into explicit pages.
//! Slots that cannot grow (choices, names, terms) are only measured and
//! reported.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::Engine;

/// Wrapping targets this share of the estimated window width, to absorb
/// proportional glyphs that are wider than the average estimate.
const WIDTH_SAFETY: f32 = 0.96;
/// Upper bound on overflow rows kept in a report; the count stays exact.
const MAX_REPORTED_OVERFLOW: usize = 1_000;
/// Halfwidth characters assumed for a `\V[n]` variable value.
const VARIABLE_ESTIMATE_CHARS: f32 = 4.0;
/// Halfwidth characters assumed for an unknown actor name.
const NAME_ESTIMATE_CHARS: f32 = 8.0;
/// A name/term is reported when it grows past this ratio of the original...
const NAME_GROWTH_RATIO: f32 = 1.5;
/// ...and is also wider than this many halfwidth characters.
const NAME_MIN_REPORT_CHARS: f32 = 10.0;

/// Kind of on-screen slot a translated entry is drawn into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextSlotKind {
    Message,
    Scroll,
    Choice,
    Description,
    Name,
    Term,
}

/// Whether the exporter rewraps this entry to its window (messages, scrolling
/// text, descriptions and profiles), so the translation's line breaks are
/// free. Must mirror the `fit_message`/`fit_scroll`/`fit_description` calls in
/// the exporter. Plugin parameters can nest keys such as `…:text:0` and are
/// never rewrapped.
pub fn exporter_rewraps(engine: &Engine, entry_id: &str) -> bool {
    if *engine == Engine::Unity || entry_id.starts_with("Plugins:") {
        return false;
    }
    let mut segments = entry_id.rsplit(':');
    let (Some(index), Some(kind)) = (segments.next(), segments.next()) else {
        return false;
    };
    index.parse::<usize>().is_ok()
        && matches!(
            kind,
            "text" | "scroll" | "changeProfile" | "description" | "profile"
        )
}

/// Entry whose translation still does not fit after fitting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverflow {
    pub file_path: String,
    pub entry_id: String,
    pub kind: TextSlotKind,
    /// Estimated width in pixels of the widest line.
    pub width: u32,
    /// Width budget in pixels for one line of this slot.
    pub max_width: u32,
    pub lines: u32,
    pub max_lines: Option<u32>,
}

/// What the exporter did to make text fit, plus what still overflows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFitReport {
    /// Active message plugin that wraps `<WordWrap>` messages by itself.
    pub wrap_plugin: Option<String>,
    /// Window sizes widened to match the original text (one note each).
    pub calibration: Vec<String>,
    /// Entries whose line breaks were changed to fit the window width.
    pub wrapped_entries: usize,
    /// Messages that now span more than one page.
    pub paginated_entries: usize,
    /// Pages added beyond the first page of each message.
    pub pages_added: usize,
    pub overflow_count: usize,
    pub overflow: Vec<TextOverflow>,
}

impl TextFitReport {
    pub fn record_overflow(&mut self, overflow: TextOverflow) {
        self.overflow_count += 1;
        if self.overflow.len() < MAX_REPORTED_OVERFLOW {
            self.overflow.push(overflow);
        }
    }
}

/// Estimated window geometry for one engine, in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub engine: Engine,
    /// Default message font size.
    pub font_size: f32,
    /// Advance of a halfwidth glyph relative to the font size.
    pub halfwidth_em: f32,
    /// Usable width of a message line without a face graphic.
    pub message_width: f32,
    /// Width a face graphic takes from every message line.
    pub face_offset: f32,
    /// Visible message rows per page.
    pub message_rows: usize,
    /// Whether the engine pages a message that has more lines than rows.
    pub native_paging: bool,
    /// Advance of an `\I[n]` icon, spacing included.
    pub icon_width: f32,
    pub font_step: f32,
    pub font_min: f32,
    pub font_max: f32,
    /// Usable width of the help window that shows descriptions.
    pub help_width: f32,
    pub help_rows: usize,
    /// Whether the help window turns `\n` into a line break.
    pub help_renders_newlines: bool,
    pub scroll_width: f32,
    pub choice_width: f32,
}

impl TextMetrics {
    /// Default-script geometry for each engine (see backend.md "Encaje de texto").
    pub fn defaults(engine: &Engine) -> Self {
        match engine {
            // MV: box 816, padding 18 -> contents 780; newLineX 0 or 168;
            // GameFont (M+ 1m, monospaced) at 28px.
            Engine::Mv => Self {
                engine: Engine::Mv,
                font_size: 28.0,
                halfwidth_em: 0.5,
                message_width: 780.0,
                face_offset: 168.0,
                message_rows: 4,
                native_paging: true,
                icon_width: 36.0,
                font_step: 12.0,
                font_min: 12.0,
                font_max: 108.0,
                help_width: 768.0,
                help_rows: 2,
                help_renders_newlines: true,
                scroll_width: 768.0,
                choice_width: 768.0,
            },
            // MZ: box = uiAreaWidth - 8 = 808, padding 12 -> inner 784;
            // text starts at 4, or faceWidth + 20 = 164 with a face.
            Engine::Mz => Self {
                engine: Engine::Mz,
                font_size: 26.0,
                halfwidth_em: 0.5,
                message_width: 780.0,
                face_offset: 160.0,
                message_rows: 4,
                native_paging: true,
                icon_width: 36.0,
                font_step: 12.0,
                font_min: 12.0,
                font_max: 108.0,
                help_width: 768.0,
                help_rows: 2,
                help_renders_newlines: true,
                scroll_width: 768.0,
                choice_width: 760.0,
            },
            // VX Ace: 544 wide, padding 12 -> 520; new_line_x 0 or 112;
            // VL Gothic at 24px.
            Engine::VxAce => Self {
                engine: Engine::VxAce,
                font_size: 24.0,
                halfwidth_em: 0.5,
                message_width: 520.0,
                face_offset: 112.0,
                message_rows: 4,
                native_paging: true,
                icon_width: 24.0,
                font_step: 8.0,
                font_min: 8.0,
                font_max: 72.0,
                help_width: 516.0,
                help_rows: 2,
                help_renders_newlines: true,
                scroll_width: 516.0,
                choice_width: 508.0,
            },
            // VX: 544 wide, padding 16 -> 512; text x 0 or 112; 20px font.
            // The help window is a single draw_text line.
            Engine::Vx => Self {
                engine: Engine::Vx,
                font_size: 20.0,
                halfwidth_em: 0.5,
                message_width: 512.0,
                face_offset: 112.0,
                message_rows: 4,
                native_paging: true,
                icon_width: 24.0,
                font_step: 8.0,
                font_min: 8.0,
                font_max: 72.0,
                help_width: 504.0,
                help_rows: 1,
                help_renders_newlines: false,
                scroll_width: 512.0,
                choice_width: 512.0,
            },
            // XP: 480 wide, padding 16 -> 448, text starts at x 4; no faces,
            // no native paging; 22px font; single-line help.
            Engine::Xp => Self {
                engine: Engine::Xp,
                font_size: 22.0,
                halfwidth_em: 0.5,
                message_width: 444.0,
                face_offset: 0.0,
                message_rows: 4,
                native_paging: false,
                icon_width: 24.0,
                font_step: 8.0,
                font_min: 8.0,
                font_max: 72.0,
                help_width: 600.0,
                help_rows: 1,
                help_renders_newlines: false,
                scroll_width: 444.0,
                choice_width: 436.0,
            },
            Engine::Unity => Self {
                engine: Engine::Unity,
                font_size: 24.0,
                halfwidth_em: 0.5,
                message_width: f32::MAX,
                face_offset: 0.0,
                message_rows: usize::MAX,
                native_paging: true,
                icon_width: 0.0,
                font_step: 0.0,
                font_min: 1.0,
                font_max: 1_000.0,
                help_width: f32::MAX,
                help_rows: usize::MAX,
                help_renders_newlines: true,
                scroll_width: f32::MAX,
                choice_width: f32::MAX,
            },
        }
    }

    fn halfwidth(&self) -> f32 {
        self.font_size * self.halfwidth_em
    }
}

/// Everything the fitter needs besides the text itself.
#[derive(Debug, Clone)]
pub struct TextFitContext {
    pub metrics: TextMetrics,
    /// Display names for `\N[n]` (translated when available).
    pub actor_names: HashMap<u32, String>,
    pub currency_unit: String,
    /// Active plugin that wraps `<WordWrap>` messages itself.
    pub wrap_plugin: Option<String>,
    /// Notes from `calibrate_from_game`.
    pub calibration: Vec<String>,
    /// The plugin wraps every message, not only `<WordWrap>` ones
    /// (YEP_MessageCore `Word Wrapping: true` prepends the tag itself).
    pub wraps_all_messages: bool,
    /// The plugin wraps help-window descriptions (YEP `Description Wrap`).
    pub wraps_descriptions: bool,
}

impl TextFitContext {
    pub fn new(metrics: TextMetrics) -> Self {
        Self {
            metrics,
            actor_names: HashMap::new(),
            currency_unit: String::new(),
            wrap_plugin: None,
            calibration: Vec::new(),
            wraps_all_messages: false,
            wraps_descriptions: false,
        }
    }

    fn plugin_wraps_message(&self, text: &str) -> bool {
        self.wrap_plugin.is_some() && (self.wraps_all_messages || has_word_wrap_tag(text))
    }

    /// Empty report that already names the detected plugin and calibration.
    pub fn new_report(&self) -> TextFitReport {
        TextFitReport {
            wrap_plugin: self.wrap_plugin.clone(),
            calibration: self.calibration.clone(),
            ..TextFitReport::default()
        }
    }

    /// Reads engine defaults plus overrides from the game's own settings:
    /// MZ `System.json` `advanced`, and MV/MZ message/core plugins.
    pub fn detect(engine: &Engine, game_dir: &Path) -> Self {
        let mut metrics = TextMetrics::defaults(engine);
        let mut wrapping = PluginWrapping::default();

        if matches!(engine, Engine::Mv | Engine::Mz) {
            if matches!(engine, Engine::Mz) {
                apply_mz_system_overrides(&mut metrics, game_dir);
            }
            if let Some(plugins) = read_active_plugins(game_dir) {
                wrapping = apply_plugin_overrides(&mut metrics, &plugins);
            }
        }

        let mut context = Self::new(metrics);
        context.wrap_plugin = wrapping.plugin;
        context.wraps_all_messages = wrapping.all_messages;
        context.wraps_descriptions = wrapping.descriptions;
        context
    }

    fn message_budget(&self, has_face: bool) -> f32 {
        let face = if has_face {
            self.metrics.face_offset
        } else {
            0.0
        };
        (self.metrics.message_width - face).max(self.metrics.halfwidth() * 8.0) * WIDTH_SAFETY
    }

    fn scaled(&self, width: f32) -> f32 {
        width * WIDTH_SAFETY
    }
}

// ---------------------------------------------------------------------------
// Game settings detection
// ---------------------------------------------------------------------------

fn apply_mz_system_overrides(metrics: &mut TextMetrics, game_dir: &Path) {
    let Some(system) = read_json(&game_dir.join("data").join("System.json")) else {
        return;
    };
    let Some(advanced) = system.get("advanced") else {
        return;
    };
    if let Some(font_size) = advanced.get("fontSize").and_then(Value::as_f64) {
        if (8.0..=96.0).contains(&font_size) {
            metrics.font_size = font_size as f32;
        }
    }
    if let Some(ui_width) = advanced.get("uiAreaWidth").and_then(Value::as_f64) {
        if (320.0..=4096.0).contains(&ui_width) {
            // boxWidth = uiAreaWidth - 8; inner = boxWidth - 2 * 12 padding;
            // text starts 4px in.
            let inner = ui_width as f32 - 8.0 - 24.0;
            metrics.message_width = inner - 4.0;
            metrics.help_width = inner - 16.0;
            metrics.scroll_width = inner - 16.0;
            metrics.choice_width = inner - 24.0;
        }
    }
}

struct ActivePlugin {
    name: String,
    parameters: serde_json::Map<String, Value>,
}

fn read_active_plugins(game_dir: &Path) -> Option<Vec<ActivePlugin>> {
    let path = ["plugins.js", "Plugins.js"]
        .iter()
        .map(|name| game_dir.join("js").join(name))
        .find(|path| path.is_file())?;
    let content = std::fs::read_to_string(path).ok()?;
    let plugins = crate::parser::rpgmaker::parse_plugins_js(&content).ok()?;
    Some(
        plugins
            .as_array()?
            .iter()
            .filter(|plugin| plugin.get("status").and_then(Value::as_bool) == Some(true))
            .filter_map(|plugin| {
                Some(ActivePlugin {
                    name: plugin.get("name")?.as_str()?.to_string(),
                    parameters: plugin
                        .get("parameters")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .collect(),
    )
}

/// Wrapping behaviour of an active message plugin.
#[derive(Debug, Default)]
struct PluginWrapping {
    plugin: Option<String>,
    all_messages: bool,
    descriptions: bool,
}

fn plugin_flag(plugin: &ActivePlugin, key: &str) -> bool {
    plugin
        .parameters
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

/// Applies geometry from well-known MV/MZ plugins (YEP_CoreEngine screen
/// width and font size; YEP_MessageCore rows, font and message width) and
/// reports which plugin word-wraps text by itself.
fn apply_plugin_overrides(metrics: &mut TextMetrics, plugins: &[ActivePlugin]) -> PluginWrapping {
    let mut wrapping = PluginWrapping::default();
    let mut box_width = None;
    let mut message_window_width = None;

    for plugin in plugins {
        let name = plugin.name.as_str();
        let param = |key: &str| plugin.parameters.get(key).and_then(Value::as_str);
        if name.eq_ignore_ascii_case("YEP_CoreEngine") {
            box_width = param("Screen Width")
                .and_then(parse_plain_number)
                .or(box_width);
            if let Some(size) = param("Font Size").and_then(parse_plain_number) {
                metrics.font_size = size;
            }
        } else if name.eq_ignore_ascii_case("YEP_MessageCore") {
            if let Some(rows) = param("Default Rows").and_then(parse_plain_number) {
                metrics.message_rows = (rows as usize).clamp(1, 12);
            }
            if let Some(size) = param("Font Size").and_then(parse_plain_number) {
                metrics.font_size = size;
            }
            if let Some(step) = param("Font Size Change").and_then(parse_plain_number) {
                metrics.font_step = step;
            }
            message_window_width = param("Default Width").and_then(parse_plain_number);
            // `<WordWrap>` messages are wrapped by the plugin; `Word Wrapping:
            // true` prepends that tag to every message (Game_Message.addText).
            wrapping.plugin = Some(plugin.name.clone());
            wrapping.all_messages = plugin_flag(plugin, "Word Wrapping");
            wrapping.descriptions = plugin_flag(plugin, "Description Wrap");
        } else if name.eq_ignore_ascii_case("VisuMZ_1_MessageCore") {
            if let Some(rows) = param("MessageRows").and_then(parse_plain_number) {
                metrics.message_rows = (rows as usize).clamp(1, 12);
            }
            wrapping.plugin = Some(plugin.name.clone());
        }
    }

    let padding = match metrics.engine {
        Engine::Mz => 12.0,
        _ => 18.0,
    };
    if let Some(width) = box_width.filter(|width| (320.0..=4096.0).contains(width)) {
        let contents = width - padding * 2.0;
        metrics.message_width = contents;
        metrics.help_width = contents - 12.0;
        metrics.scroll_width = contents - 12.0;
        metrics.choice_width = contents - 12.0;
    }
    if let Some(width) = message_window_width.filter(|width| (160.0..=4096.0).contains(width)) {
        metrics.message_width = width - padding * 2.0;
    }

    wrapping
}

/// Parses a plugin parameter only when it is a plain number; expressions such
/// as `Graphics.boxWidth` keep the default.
fn parse_plain_number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|number| *number > 0.0)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

// ---------------------------------------------------------------------------
// Tokenizing and measuring
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum CodeEffect {
    /// Zero width, no persistent state (waits, pauses, unknown plugin codes).
    None,
    /// Persistent text color: re-emitted at the start of a new page.
    Color,
    Icon,
    FontBigger,
    FontSmaller,
    FontSet(f32),
    Variable,
    ActorName(Option<u32>),
    PartyMember,
    Currency,
    /// A literal backslash written as `\\`.
    Backslash,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Char(char),
    Code { raw: String, effect: CodeEffect },
}

impl Token {
    fn raw(&self) -> String {
        match self {
            Token::Char(ch) => ch.to_string(),
            Token::Code { raw, .. } => raw.clone(),
        }
    }
}

/// Splits one line of message text into characters and escape codes.
fn tokenize(line: &str) -> Vec<Token> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        if ch != '\\' || index + 1 >= chars.len() {
            tokens.push(Token::Char(ch));
            index += 1;
            continue;
        }

        let next = chars[index + 1];
        if next == '\\' {
            tokens.push(Token::Code {
                raw: "\\\\".into(),
                effect: CodeEffect::Backslash,
            });
            index += 2;
            continue;
        }
        if matches!(next, '{' | '}' | '.' | '|' | '!' | '>' | '<' | '^' | '$') {
            let effect = match next {
                '{' => CodeEffect::FontBigger,
                '}' => CodeEffect::FontSmaller,
                _ => CodeEffect::None,
            };
            tokens.push(Token::Code {
                raw: chars[index..index + 2].iter().collect(),
                effect,
            });
            index += 2;
            continue;
        }
        if !next.is_ascii_alphabetic() {
            tokens.push(Token::Char(ch));
            index += 1;
            continue;
        }

        let mut cursor = index + 1;
        while cursor < chars.len() && chars[cursor].is_ascii_alphabetic() {
            cursor += 1;
        }
        let letters = chars[index + 1..cursor].iter().collect::<String>();
        let mut argument = None;
        if let Some(&open) = chars.get(cursor) {
            let close = match open {
                '[' => Some(']'),
                '<' => Some('>'),
                _ => None,
            };
            if let Some(close) = close {
                if let Some(offset) = chars[cursor + 1..].iter().position(|ch| *ch == close) {
                    argument = Some((
                        open,
                        chars[cursor + 1..cursor + 1 + offset]
                            .iter()
                            .collect::<String>(),
                    ));
                    cursor += offset + 2;
                }
            }
        }

        let upper = letters.to_ascii_uppercase();
        let numeric = argument
            .as_ref()
            .filter(|(open, _)| *open == '[')
            .and_then(|(_, value)| value.trim().parse::<f32>().ok());
        let effect = match (upper.as_str(), argument.as_ref().map(|(open, _)| *open)) {
            ("C", Some('[')) => CodeEffect::Color,
            ("I", Some('[')) => CodeEffect::Icon,
            ("V", Some('[')) => CodeEffect::Variable,
            ("N", Some('[')) => CodeEffect::ActorName(numeric.map(|value| value as u32)),
            ("P", Some('[')) => CodeEffect::PartyMember,
            ("FS", Some('[')) => numeric.map_or(CodeEffect::None, CodeEffect::FontSet),
            ("G", None) => CodeEffect::Currency,
            _ => CodeEffect::None,
        };
        tokens.push(Token::Code {
            raw: chars[index..cursor].iter().collect(),
            effect,
        });
        index = cursor;
    }

    tokens
}

/// Width class of a character in the Japanese fonts RPG Maker ships with:
/// East Asian wide/fullwidth and ambiguous symbols take a full em.
fn is_fullwidth(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x115F
        | 0x2014..=0x2015 // — ―
        | 0x2026 // …
        | 0x203B // ※
        | 0x2160..=0x217F
        | 0x2190..=0x21FF
        | 0x2460..=0x24FF
        | 0x2500..=0x257F
        | 0x25A0..=0x25FF // ○ ● ■
        | 0x2600..=0x26FF // ★ ♡
        | 0x2700..=0x27BF
        | 0x2E80..=0x303E // CJK punctuation, ideographic space
        | 0x3041..=0x33FF // kana
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD)
}

/// Mutable font state while walking text.
#[derive(Debug, Clone, PartialEq)]
struct FontState {
    size: f32,
    color: Option<String>,
}

impl FontState {
    fn new(context: &TextFitContext) -> Self {
        Self {
            size: context.metrics.font_size,
            color: None,
        }
    }

    fn apply(&mut self, token: &Token, context: &TextFitContext) {
        let Token::Code { raw, effect } = token else {
            return;
        };
        let metrics = &context.metrics;
        match effect {
            CodeEffect::FontBigger => {
                self.size = (self.size + metrics.font_step).min(metrics.font_max);
            }
            CodeEffect::FontSmaller => {
                self.size = (self.size - metrics.font_step).max(metrics.font_min);
            }
            CodeEffect::FontSet(size) => self.size = size.clamp(metrics.font_min, metrics.font_max),
            CodeEffect::Color => self.color = Some(raw.clone()),
            _ => {}
        }
    }

    /// Codes that restore this state on a fresh page, where the engine resets
    /// the font to its defaults.
    fn restore_codes(&self, context: &TextFitContext) -> String {
        let mut codes = String::new();
        if let Some(color) = &self.color {
            if !is_default_color(color) {
                codes.push_str(color);
            }
        }
        let metrics = &context.metrics;
        if metrics.font_step > 0.0 {
            let steps = ((self.size - metrics.font_size) / metrics.font_step).round() as i32;
            let code = if steps > 0 { "\\{" } else { "\\}" };
            for _ in 0..steps.unsigned_abs() {
                codes.push_str(code);
            }
        }
        codes
    }
}

fn is_default_color(code: &str) -> bool {
    let digits = code
        .trim_start_matches('\\')
        .trim_start_matches(['C', 'c'])
        .trim_matches(|ch| ch == '[' || ch == ']');
    digits.trim() == "0"
}

fn token_width(token: &Token, state: &FontState, context: &TextFitContext) -> f32 {
    let halfwidth = state.size * context.metrics.halfwidth_em;
    match token {
        Token::Char(ch) => {
            if is_fullwidth(*ch) {
                state.size
            } else {
                halfwidth
            }
        }
        Token::Code { effect, .. } => match effect {
            CodeEffect::Icon => context.metrics.icon_width,
            CodeEffect::Variable => halfwidth * VARIABLE_ESTIMATE_CHARS,
            CodeEffect::ActorName(id) => id
                .and_then(|id| context.actor_names.get(&id))
                .map(|name| plain_text_width(name, state, context))
                .unwrap_or(halfwidth * NAME_ESTIMATE_CHARS),
            CodeEffect::PartyMember => context
                .actor_names
                .values()
                .map(|name| plain_text_width(name, state, context))
                .fold(halfwidth * NAME_ESTIMATE_CHARS, f32::max),
            CodeEffect::Currency => {
                plain_text_width(&context.currency_unit, state, context).max(halfwidth)
            }
            CodeEffect::Backslash => halfwidth,
            _ => 0.0,
        },
    }
}

fn plain_text_width(text: &str, state: &FontState, context: &TextFitContext) -> f32 {
    text.chars()
        .map(|ch| token_width(&Token::Char(ch), state, context))
        .sum()
}

/// Width of one line drawn with `state`, which it advances: font size and
/// color codes persist to the following lines of the same window page.
fn measure_line(line: &str, state: &mut FontState, context: &TextFitContext) -> f32 {
    let mut width = 0.0;
    for token in tokenize(line) {
        state.apply(&token, context);
        width += token_width(&token, state, context);
    }
    width
}

/// Estimated width of one line (no `\n`) in pixels, from the default font.
pub(crate) fn line_width(line: &str, context: &TextFitContext) -> f32 {
    measure_line(line, &mut FontState::new(context), context)
}

/// Widest of consecutive lines drawn in one window, font state carried over.
fn widest_line(lines: &[String], context: &TextFitContext) -> f32 {
    let mut state = FontState::new(context);
    lines
        .iter()
        .map(|line| measure_line(line, &mut state, context))
        .fold(0.0, f32::max)
}

/// Widths of a message's lines as the engine draws them: font state carries
/// across lines and resets on each new page of a natively paged window.
fn message_line_widths(lines: &[String], context: &TextFitContext) -> Vec<f32> {
    let rows = context.metrics.message_rows.max(1);
    let mut state = FontState::new(context);
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if context.metrics.native_paging && index > 0 && index % rows == 0 {
                state = FontState::new(context);
            }
            measure_line(line, &mut state, context)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Line breaking
// ---------------------------------------------------------------------------

/// Unbreakable unit: a word, one CJK character with its attached punctuation,
/// or a run of spaces. Zero-width codes stick to the following unit.
#[derive(Debug, Clone)]
struct Segment {
    tokens: Vec<Token>,
    width: f32,
    is_space: bool,
}

fn closes_cjk(ch: char) -> bool {
    matches!(
        ch,
        '、' | '。'
            | '，'
            | '．'
            | '・'
            | '：'
            | '；'
            | '？'
            | '！'
            | '）'
            | '」'
            | '』'
            | '】'
            | '〕'
            | '〉'
            | '》'
            | '〟'
            | '’'
            | '”'
            | 'ー'
            | '…'
            | '‥'
            | '々'
            | 'ゝ'
            | 'ゞ'
            | 'ぁ'
            | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | '～'
            | '〜'
    )
}

fn opens_cjk(ch: char) -> bool {
    matches!(
        ch,
        '（' | '「' | '『' | '【' | '〔' | '〈' | '《' | '〝' | '‘' | '“'
    )
}

fn segments_for(line: &str, state: &mut FontState, context: &TextFitContext) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut word: Vec<Token> = Vec::new();
    let mut word_width = 0.0;
    let mut pending: Vec<Token> = Vec::new();
    let mut pending_width = 0.0;

    let flush_word = |segments: &mut Vec<Segment>, word: &mut Vec<Token>, width: &mut f32| {
        if !word.is_empty() {
            segments.push(Segment {
                tokens: std::mem::take(word),
                width: *width,
                is_space: false,
            });
            *width = 0.0;
        }
    };

    for token in tokenize(line) {
        state.apply(&token, context);
        let width = token_width(&token, state, context);
        match &token {
            Token::Code { .. } => {
                if word.is_empty() {
                    pending.push(token);
                    pending_width += width;
                } else {
                    word.push(token);
                    word_width += width;
                }
            }
            Token::Char(ch) if ch.is_whitespace() && *ch != '\u{3000}' => {
                flush_word(&mut segments, &mut word, &mut word_width);
                if !pending.is_empty() {
                    // Codes between two spaces become their own unit, in their
                    // original place: a space may be dropped at a line break,
                    // a code never, and an icon must not jump over a space.
                    segments.push(Segment {
                        tokens: std::mem::take(&mut pending),
                        width: pending_width,
                        is_space: false,
                    });
                    pending_width = 0.0;
                }
                match segments.last_mut() {
                    Some(last) if last.is_space => {
                        last.tokens.push(token);
                        last.width += width;
                    }
                    _ => segments.push(Segment {
                        tokens: vec![token],
                        width,
                        is_space: true,
                    }),
                }
            }
            Token::Char(ch) if is_fullwidth(*ch) => {
                let attaches_to_previous = closes_cjk(*ch)
                    || segments
                        .last()
                        .and_then(|segment| segment.tokens.last())
                        .is_some_and(|last| matches!(last, Token::Char(prev) if opens_cjk(*prev)))
                        && word.is_empty();
                if !word.is_empty() && !closes_cjk(*ch) {
                    flush_word(&mut segments, &mut word, &mut word_width);
                }
                let mut unit = std::mem::take(&mut pending);
                let unit_width = pending_width + width;
                pending_width = 0.0;
                unit.push(token);
                if !word.is_empty() {
                    word.extend(unit);
                    word_width += unit_width;
                    flush_word(&mut segments, &mut word, &mut word_width);
                } else if attaches_to_previous {
                    match segments.last_mut() {
                        Some(last) if !last.is_space => {
                            last.tokens.extend(unit);
                            last.width += unit_width;
                        }
                        _ => segments.push(Segment {
                            tokens: unit,
                            width: unit_width,
                            is_space: false,
                        }),
                    }
                } else {
                    segments.push(Segment {
                        tokens: unit,
                        width: unit_width,
                        is_space: false,
                    });
                }
            }
            Token::Char(_) => {
                if word.is_empty() && !pending.is_empty() {
                    word.append(&mut pending);
                    word_width += pending_width;
                    pending_width = 0.0;
                }
                word.push(token);
                word_width += width;
            }
        }
    }

    flush_word(&mut segments, &mut word, &mut word_width);
    if !pending.is_empty() {
        match segments.last_mut() {
            Some(last) if !last.is_space => {
                last.tokens.append(&mut pending);
                last.width += pending_width;
            }
            _ => segments.push(Segment {
                tokens: pending,
                width: pending_width,
                is_space: false,
            }),
        }
    }
    segments
}

fn render(tokens: &[Token]) -> String {
    tokens.iter().map(Token::raw).collect()
}

/// Greedy word wrap of one logical line drawn from `state` (advanced to the
/// end of the line). Never splits an escape code; splits a single word only
/// when it alone exceeds the width.
fn wrap_line(
    line: &str,
    max_width: f32,
    context: &TextFitContext,
    state: &mut FontState,
) -> Vec<String> {
    let line_start_state = state.clone();
    let segments = segments_for(line, state, context);
    let mut lines: Vec<String> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut current_width = 0.0;
    let mut pending_space: Option<Segment> = None;

    for segment in segments {
        if segment.is_space {
            if lines.is_empty() && current.is_empty() {
                // Keep the translator's indentation on the first line only.
                current_width += segment.width;
                current.extend(segment.tokens);
            } else if !current.is_empty() {
                pending_space = Some(match pending_space.take() {
                    Some(mut space) => {
                        space.tokens.extend(segment.tokens);
                        space.width += segment.width;
                        space
                    }
                    None => segment,
                });
            }
            continue;
        }

        let space_width = pending_space.as_ref().map_or(0.0, |space| space.width);
        if !current.is_empty() && current_width + space_width + segment.width > max_width {
            lines.push(render(&current).trim_end().to_string());
            current.clear();
            current_width = 0.0;
            pending_space = None;
        } else if let Some(space) = pending_space.take() {
            current.extend(space.tokens);
            current_width += space.width;
        }

        if current.is_empty() && segment.width > max_width {
            // One unit wider than the whole line: split it between characters.
            let mut piece: Vec<Token> = Vec::new();
            let mut piece_width = 0.0;
            // Approximation: measured from the font at the start of the line.
            let mut piece_state = line_start_state.clone();
            for token in segment.tokens {
                piece_state.apply(&token, context);
                let width = token_width(&token, &piece_state, context);
                let is_char = matches!(token, Token::Char(_));
                if is_char && !piece.is_empty() && piece_width + width > max_width {
                    lines.push(render(&piece));
                    piece.clear();
                    piece_width = 0.0;
                }
                piece.push(token);
                piece_width += width;
            }
            current = piece;
            current_width = piece_width;
            continue;
        }

        current.extend(segment.tokens);
        current_width += segment.width;
    }

    if !current.is_empty() {
        lines.push(render(&current).trim_end().to_string());
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Result of fitting one Show Text / Scrolling Text translation.
#[derive(Debug, Clone, PartialEq)]
pub struct FittedText {
    /// Lines to write, one per event command.
    pub lines: Vec<String>,
    /// Page boundaries for engines without native paging (XP): `lines` split
    /// into chunks of at most `rows` lines.
    pub rows_per_page: usize,
    pub pages: usize,
    /// True when line breaks differ from the translator's.
    pub changed: bool,
    pub widest: f32,
    pub max_width: f32,
    /// A line still exceeds the width (an unbreakable code sequence).
    pub overflow: bool,
    /// Left untouched because a message plugin wraps it (`<WordWrap>`).
    pub plugin_wrapped: bool,
}

fn has_word_wrap_tag(text: &str) -> bool {
    text.to_ascii_lowercase().contains("<wordwrap>")
}

/// A first line that labels the speaker in games without a name box:
/// `【Name】`, `Name:`, or a short line followed by a quoted line.
fn speaker_label(original: &str, translation: &str, context: &TextFitContext) -> Option<String> {
    let mut original_lines = original.split('\n');
    let original_first = original_lines.next()?.trim();
    let original_second = original_lines.next().map(str::trim).unwrap_or("");
    let translated_first = translation.split('\n').next()?.trim_end();
    if original_first.is_empty() || translated_first.trim().is_empty() {
        return None;
    }

    let halfwidth = context.metrics.halfwidth();
    let short = |line: &str| line_width(line, context) <= halfwidth * 24.0;
    let enclosed = |line: &str| {
        let line = line.trim();
        (line.starts_with('【') && line.ends_with('】'))
            || (line.starts_with('［') && line.ends_with('］'))
            || (line.starts_with('[') && line.ends_with(']') && !line.starts_with("\\"))
            || line.ends_with('：')
            || line.ends_with(':')
    };
    let opens_quote = original_second.starts_with(['「', '『', '（', '“', '"', '(', '‘']);

    let original_is_label = short(original_first)
        && (enclosed(original_first) || opens_quote)
        && !original_second.is_empty();
    (original_is_label && short(translated_first)).then(|| translated_first.to_string())
}

fn is_structural_line(line: &str, context: &TextFitContext) -> bool {
    line.contains('\\') && line_width(line, context) == 0.0 && !line.trim().is_empty()
}

fn ends_sentence(line: &str) -> bool {
    line.trim_end()
        .ends_with(['.', '!', '?', '…', '。', '！', '？', ',', ';', ':'])
}

/// Joins consecutive prose lines into paragraphs (keeping labels, blank and
/// code-only lines on their own) and wraps them to `max_width`.
fn reflow(
    lines: &[String],
    keep_first: bool,
    max_width: f32,
    context: &TextFitContext,
) -> Vec<String> {
    let mut output = Vec::new();
    let mut paragraph = String::new();
    let mut state = FontState::new(context);
    let flush = |paragraph: &mut String, output: &mut Vec<String>, state: &mut FontState| {
        if !paragraph.is_empty() {
            output.extend(wrap_line(paragraph, max_width, context, state));
            paragraph.clear();
        }
    };

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut paragraph, &mut output, &mut state);
            output.push(String::new());
            continue;
        }
        if (index == 0 && keep_first) || is_structural_line(line, context) {
            flush(&mut paragraph, &mut output, &mut state);
            output.extend(wrap_line(line, max_width, context, &mut state));
            continue;
        }
        if paragraph.is_empty() {
            paragraph.push_str(trimmed);
        } else {
            let joins_cjk = paragraph.chars().last().is_some_and(is_fullwidth)
                && trimmed.chars().next().is_some_and(is_fullwidth);
            if !joins_cjk {
                paragraph.push(' ');
            }
            paragraph.push_str(trimmed);
        }
    }
    flush(&mut paragraph, &mut output, &mut state);
    output
}

fn wrap_each(lines: &[String], max_width: f32, context: &TextFitContext) -> Vec<String> {
    let mut state = FontState::new(context);
    let mut output = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            output.push(String::new());
        } else {
            output.extend(wrap_line(line, max_width, context, &mut state));
        }
    }
    output
}

fn page_count(line_count: usize, rows: usize) -> usize {
    line_count.max(1).div_ceil(rows.max(1))
}

/// Repeats a plain-text speaker label and restores the font state at the top
/// of every page after the first (the engine resets the font on a new page).
/// Name box codes such as YEP's `\n<Name>` are resolved once per message
/// by the plugin and are not repeated.
fn paginate(
    body: Vec<String>,
    label: Option<&str>,
    rows: usize,
    context: &TextFitContext,
) -> Vec<String> {
    let rows = rows.max(1);
    let label_rows = usize::from(label.is_some());
    if rows <= label_rows {
        return body;
    }
    let body_rows = rows - label_rows;
    let mut output = Vec::new();
    let mut state = FontState::new(context);

    for (page_index, chunk) in body.chunks(body_rows).enumerate() {
        if page_index > 0 {
            if let Some(label) = label {
                output.push(label.to_string());
            }
        }
        for (line_index, line) in chunk.iter().enumerate() {
            let mut text = String::new();
            if page_index > 0 && line_index == 0 {
                text.push_str(&state.restore_codes(context));
            }
            text.push_str(line);
            for token in tokenize(line) {
                state.apply(&token, context);
            }
            output.push(text);
        }
    }
    output
}

/// Fits a Show Text translation: keeps the translator's line breaks when every
/// line fits, otherwise wraps (reflowing paragraphs when that saves a page),
/// then lays pages out for `rows`-line windows.
pub fn fit_message(
    translation: &str,
    original: &str,
    has_face: bool,
    compact_blank_lines: bool,
    context: &TextFitContext,
) -> FittedText {
    let max_width = context.message_budget(has_face);
    let rows = context.metrics.message_rows;
    let source_lines = translation
        .split('\n')
        .map(str::to_string)
        .filter(|line| !compact_blank_lines || !line.trim().is_empty())
        .collect::<Vec<_>>();
    let source_lines = if source_lines.is_empty() {
        vec![String::new()]
    } else {
        source_lines
    };

    if context.plugin_wraps_message(translation) {
        let widest = widest_line(&source_lines, context);
        return FittedText {
            pages: 1,
            rows_per_page: rows,
            changed: source_lines.join("\n") != translation,
            widest,
            max_width,
            overflow: false,
            plugin_wrapped: true,
            lines: source_lines,
        };
    }

    let label = speaker_label(original, translation, context);
    // A short first line without closing punctuation is usually a speaker
    // name or a title: never merge it into the following paragraph.
    let keep_first = label.is_some()
        || (source_lines.len() > 1
            && source_lines.first().is_some_and(|line| {
                line_width(line, context) <= context.metrics.halfwidth() * 32.0
                    && !ends_sentence(line)
            }));

    let (label_line, body_lines) = match &label {
        Some(_) => (source_lines.first().cloned(), source_lines[1..].to_vec()),
        None => (None, source_lines.clone()),
    };
    let label_rows = usize::from(label_line.is_some());
    let body_rows = rows.saturating_sub(label_rows).max(1);

    let wrapped = wrap_each(&body_lines, max_width, context);
    let reflowed = reflow(
        &body_lines,
        keep_first && label_line.is_none(),
        max_width,
        context,
    );
    let body = if page_count(reflowed.len(), body_rows) < page_count(wrapped.len(), body_rows) {
        reflowed
    } else {
        wrapped
    };

    let mut lines = Vec::new();
    if let Some(label) = &label_line {
        lines.push(label.clone());
    }
    let paged = paginate(body, label.as_deref(), rows, context);
    lines.extend(paged);

    let widest = widest_line(&lines, context);
    let pages = page_count(lines.len(), rows);
    FittedText {
        changed: lines != source_lines,
        overflow: widest > max_width / WIDTH_SAFETY + 0.5,
        widest,
        max_width,
        rows_per_page: rows,
        pages,
        plugin_wrapped: false,
        lines,
    }
}

/// Fits Scrolling Text: every line wrapped on its own, no row limit.
pub fn fit_scroll(translation: &str, context: &TextFitContext) -> FittedText {
    let max_width = context.scaled(context.metrics.scroll_width);
    let source_lines = translation
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let lines = wrap_each(&source_lines, max_width, context);
    let widest = widest_line(&lines, context);
    FittedText {
        changed: lines != source_lines,
        overflow: widest > max_width / WIDTH_SAFETY + 0.5,
        widest,
        max_width,
        rows_per_page: usize::MAX,
        pages: 1,
        plugin_wrapped: false,
        lines,
    }
}

/// Fits a database description or profile into the help window rows. Engines
/// whose help window ignores `\n` get the text back unchanged (measured only).
pub fn fit_description(translation: &str, context: &TextFitContext) -> FittedText {
    let metrics = &context.metrics;
    let max_width = context.scaled(metrics.help_width);
    let source_lines = translation
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let plugin_wrapped = context.wrap_plugin.is_some()
        && (context.wraps_descriptions || has_word_wrap_tag(translation));
    if !metrics.help_renders_newlines || plugin_wrapped {
        let widest = widest_line(&source_lines, context);
        return FittedText {
            changed: false,
            overflow: widest > max_width / WIDTH_SAFETY + 0.5
                || source_lines.len() > metrics.help_rows,
            widest,
            max_width,
            rows_per_page: metrics.help_rows,
            pages: 1,
            plugin_wrapped,
            lines: source_lines,
        };
    }

    let wrapped = wrap_each(&source_lines, max_width, context);
    let lines = if wrapped.len() > metrics.help_rows {
        let reflowed = reflow(&source_lines, false, max_width, context);
        if reflowed.len() < wrapped.len() {
            reflowed
        } else {
            wrapped
        }
    } else {
        wrapped
    };
    let widest = widest_line(&lines, context);
    FittedText {
        changed: lines != source_lines,
        overflow: lines.len() > metrics.help_rows || widest > max_width / WIDTH_SAFETY + 0.5,
        widest,
        max_width,
        rows_per_page: metrics.help_rows,
        pages: 1,
        plugin_wrapped: false,
        lines,
    }
}

/// Measures a choice against the choice window. Choices are never rewritten.
pub fn choice_overflow(translation: &str, context: &TextFitContext) -> Option<(f32, f32)> {
    let max_width = context.metrics.choice_width;
    let width = line_width(translation, context);
    (width > max_width).then_some((width, max_width))
}

/// Names and terms are squeezed, not wrapped, by the engines; report the ones
/// that grew well past the original that the game was designed around.
pub fn name_overflow(
    translation: &str,
    original: &str,
    context: &TextFitContext,
) -> Option<(f32, f32)> {
    let width = widest_line(
        &translation
            .split('\n')
            .map(str::to_string)
            .collect::<Vec<_>>(),
        context,
    );
    let original_width = widest_line(
        &original.split('\n').map(str::to_string).collect::<Vec<_>>(),
        context,
    );
    let budget = (original_width * NAME_GROWTH_RATIO)
        .max(context.metrics.halfwidth() * NAME_MIN_REPORT_CHARS);
    (width > budget).then_some((width, budget))
}

/// Builds a width-aware report row.
pub fn overflow_row(
    file_path: &str,
    entry_id: &str,
    kind: TextSlotKind,
    width: f32,
    max_width: f32,
    lines: usize,
    max_lines: Option<usize>,
) -> TextOverflow {
    TextOverflow {
        file_path: file_path.to_string(),
        entry_id: entry_id.to_string(),
        kind,
        width: width.round() as u32,
        max_width: max_width.round() as u32,
        lines: lines as u32,
        max_lines: max_lines.map(|rows| rows as u32),
    }
}

// ---------------------------------------------------------------------------
// Game text traversal: layout audit and budget calibration
// ---------------------------------------------------------------------------

/// One piece of on-screen text found in a game's data files.
pub enum TextSample<'a> {
    /// Show Text: all lines of one message (101 + 401, XP's 101 included).
    Message {
        lines: &'a [String],
        has_face: bool,
    },
    Scroll {
        line: &'a str,
    },
    Choice {
        text: &'a str,
    },
    Description {
        text: &'a str,
    },
}

const CODE_TEXT_HEADER: i64 = 101;
const CODE_TEXT_LINE: i64 = 401;
const CODE_CHOICES: i64 = 102;
const CODE_SCROLL_LINE: i64 = 405;

/// A command reduced to what the traversal needs, for MV/MZ JSON and Marshal.
struct PlainCommand {
    code: i64,
    parameters: Vec<Value>,
}

fn plain_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| crate::services::exporter::string_from_legacy_value(value))
}

fn plain_commands(list: &[Value], legacy: bool) -> Vec<PlainCommand> {
    list.iter()
        .filter_map(|command| {
            let (code, parameters) = if legacy {
                let fields = crate::services::exporter::object_fields(command)?;
                (fields.get("@code")?, fields.get("@parameters"))
            } else {
                (command.get("code")?, command.get("parameters"))
            };
            Some(PlainCommand {
                code: code.as_i64()?,
                parameters: parameters
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn visit_command_list(
    list: &[Value],
    legacy: bool,
    engine: &Engine,
    visit: &mut dyn FnMut(TextSample<'_>),
) {
    let commands = plain_commands(list, legacy);
    let first_param = |command: &PlainCommand| command.parameters.first().and_then(plain_string);
    let xp = matches!(engine, Engine::Xp);
    let mut index = 0usize;

    while index < commands.len() {
        let command = &commands[index];
        match command.code {
            CODE_TEXT_HEADER => {
                let has_face = !xp && first_param(command).is_some_and(|face| !face.is_empty());
                let mut lines = Vec::new();
                if xp {
                    if let Some(text) = first_param(command) {
                        lines.extend(text.split('\n').map(str::to_string));
                    }
                }
                let mut cursor = index + 1;
                while cursor < commands.len() && commands[cursor].code == CODE_TEXT_LINE {
                    if let Some(text) = first_param(&commands[cursor]) {
                        lines.extend(text.split('\n').map(str::to_string));
                    }
                    cursor += 1;
                }
                if !lines.is_empty() {
                    visit(TextSample::Message {
                        lines: &lines,
                        has_face,
                    });
                }
                index = cursor;
                continue;
            }
            CODE_SCROLL_LINE => {
                if let Some(text) = first_param(command) {
                    for line in text.split('\n') {
                        visit(TextSample::Scroll { line });
                    }
                }
            }
            CODE_CHOICES => {
                if let Some(choices) = command.parameters.first().and_then(Value::as_array) {
                    for choice in choices.iter().filter_map(plain_string) {
                        visit(TextSample::Choice { text: &choice });
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
}

/// Reads `key` from an MV/MZ object (`"key"`) or a Marshal object (`"@key"`).
fn data_field<'a>(item: &'a Value, key: &str, legacy: bool) -> Option<&'a Value> {
    if legacy {
        crate::services::exporter::object_fields(item)?.get(&format!("@{key}"))
    } else {
        item.get(key)
    }
}

fn event_pages_lists(event: &Value, legacy: bool) -> Vec<&Vec<Value>> {
    data_field(event, "pages", legacy)
        .and_then(Value::as_array)
        .map(|pages| {
            pages
                .iter()
                .filter_map(|page| data_field(page, "list", legacy).and_then(Value::as_array))
                .collect()
        })
        .unwrap_or_default()
}

fn is_map_stem(lower: &str) -> bool {
    lower.len() > 3 && lower.starts_with("map") && lower[3..].chars().all(|ch| ch.is_ascii_digit())
}

fn visit_data_file(
    stem: &str,
    value: &Value,
    legacy: bool,
    engine: &Engine,
    visit: &mut dyn FnMut(TextSample<'_>),
) {
    let lower = stem.to_ascii_lowercase();
    if is_map_stem(&lower) {
        let events = data_field(value, "events", legacy);
        let event_values: Vec<&Value> = if legacy {
            events
                .and_then(|events| events.get("$hash"))
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry.get("value"))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            events
                .and_then(Value::as_array)
                .map(|events| events.iter().filter(|event| !event.is_null()).collect())
                .unwrap_or_default()
        };
        for event in event_values {
            for list in event_pages_lists(event, legacy) {
                visit_command_list(list, legacy, engine, visit);
            }
        }
        return;
    }

    let Some(items) = value.as_array() else {
        return;
    };
    match lower.as_str() {
        "commonevents" => {
            for item in items {
                if let Some(list) = data_field(item, "list", legacy).and_then(Value::as_array) {
                    visit_command_list(list, legacy, engine, visit);
                }
            }
        }
        "troops" => {
            for item in items {
                for list in event_pages_lists(item, legacy) {
                    visit_command_list(list, legacy, engine, visit);
                }
            }
        }
        "items" | "skills" | "weapons" | "armors" | "actors" => {
            let key = if lower == "actors" {
                "profile"
            } else {
                "description"
            };
            for item in items {
                if let Some(text) = data_field(item, key, legacy).and_then(plain_string) {
                    if !text.trim().is_empty() {
                        visit(TextSample::Description { text: &text });
                    }
                }
            }
        }
        _ => {}
    }
}

/// Visits every message, scrolling line, choice and description stored in a
/// game folder (a source game or an export). Read-only.
pub fn visit_game_text(
    engine: &Engine,
    game_dir: &Path,
    visit: &mut dyn FnMut(TextSample<'_>),
) -> Result<(), crate::error::AppError> {
    let (legacy, extension) = match engine {
        Engine::Mv | Engine::Mz => (false, "json"),
        Engine::VxAce => (true, "rvdata2"),
        Engine::Vx => (true, "rvdata"),
        Engine::Xp => (true, "rxdata"),
        Engine::Unity => return Ok(()),
    };
    let data_dir = ["data", "Data"]
        .iter()
        .map(|name| game_dir.join(name))
        .find(|path| path.is_dir())
        .ok_or_else(|| {
            crate::error::AppError::NotFound(format!(
                "No data directory under '{}'",
                game_dir.display()
            ))
        })?;

    let mut paths = std::fs::read_dir(&data_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("")
            .to_string();
        let lower = stem.to_ascii_lowercase();
        let relevant = is_map_stem(&lower)
            || matches!(
                lower.as_str(),
                "commonevents" | "troops" | "items" | "skills" | "weapons" | "armors" | "actors"
            );
        if !relevant
            || std::fs::metadata(&path)
                .map(|meta| meta.len() == 0)
                .unwrap_or(true)
        {
            continue;
        }
        let value = if legacy {
            crate::services::exporter::read_marshal_value(&path)?
        } else {
            serde_json::from_str(&std::fs::read_to_string(&path)?)?
        };
        visit_data_file(&stem, &value, legacy, engine, visit);
    }

    Ok(())
}

/// Layout statistics of a game's text measured against the context's windows.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLayoutAudit {
    pub messages: usize,
    pub message_lines: usize,
    /// Message lines wider than the window (clipped in game).
    pub overflowing_lines: usize,
    pub overflowing_messages: usize,
    /// Messages with more lines than visible rows.
    pub multi_page_messages: usize,
    /// Lines past the visible rows on an engine without native paging (XP).
    pub hidden_lines: usize,
    pub scroll_lines: usize,
    pub overflowing_scroll_lines: usize,
    pub choices: usize,
    pub overflowing_choices: usize,
    pub descriptions: usize,
    pub overflowing_descriptions: usize,
    /// Widest message line found, in estimated pixels.
    pub widest_line: u32,
    /// Message line budget without a face graphic, in pixels.
    pub message_width: u32,
    pub message_rows: usize,
}

/// Measures a game folder against `context` (typically calibrated from the
/// source game, so an export is judged by the same windows).
pub fn audit_game_layout(
    context: &TextFitContext,
    game_dir: &Path,
) -> Result<TextLayoutAudit, crate::error::AppError> {
    let metrics = &context.metrics;
    let mut audit = TextLayoutAudit {
        message_width: metrics.message_width.round() as u32,
        message_rows: metrics.message_rows,
        ..TextLayoutAudit::default()
    };
    visit_game_text(&metrics.engine, game_dir, &mut |sample| match sample {
        TextSample::Message { lines, has_face } => {
            let budget = metrics.message_width - if has_face { metrics.face_offset } else { 0.0 };
            audit.messages += 1;
            audit.message_lines += lines.len();
            let mut overflowing = 0usize;
            for width in message_line_widths(lines, context) {
                audit.widest_line = audit.widest_line.max(width.round() as u32);
                overflowing += usize::from(width > budget + 0.5);
            }
            audit.overflowing_lines += overflowing;
            audit.overflowing_messages += usize::from(overflowing > 0);
            if lines.len() > metrics.message_rows {
                audit.multi_page_messages += 1;
                if !metrics.native_paging {
                    audit.hidden_lines += lines.len() - metrics.message_rows;
                }
            }
        }
        TextSample::Scroll { line } => {
            audit.scroll_lines += 1;
            audit.overflowing_scroll_lines +=
                usize::from(line_width(line, context) > metrics.scroll_width + 0.5);
        }
        TextSample::Choice { text } => {
            audit.choices += 1;
            audit.overflowing_choices += usize::from(choice_overflow(text, context).is_some());
        }
        TextSample::Description { text } => {
            audit.descriptions += 1;
            let lines = text.split('\n').map(str::to_string).collect::<Vec<_>>();
            let too_wide = widest_line(&lines, context) > metrics.help_width + 0.5;
            audit.overflowing_descriptions +=
                usize::from(too_wide || lines.len() > metrics.help_rows);
        }
    })?;
    Ok(audit)
}

/// Nearest-rank 99th percentile (the maximum for small samples), so a handful
/// of lines that were already clipped in the original do not widen budgets.
fn percentile_99(samples: &mut [f32]) -> Option<f32> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_by(f32::total_cmp);
    let rank = ((samples.len() as f32) * 0.99).ceil() as usize;
    Some(samples[rank.clamp(1, samples.len()) - 1])
}

impl TextFitContext {
    /// Widens the window budgets to what the original text already uses. A
    /// released game was authored to fit its own windows, and scripts or
    /// plugins often resize them (resolution, fonts, message systems).
    /// Returns a note for each budget that changed.
    pub fn calibrate_from_game(
        &mut self,
        game_dir: &Path,
    ) -> Result<Vec<String>, crate::error::AppError> {
        let mut plain = Vec::new();
        let mut faced = Vec::new();
        let mut scroll = Vec::new();
        let mut choices = Vec::new();
        let mut descriptions = Vec::new();
        let mut description_rows = 0usize;
        let engine = self.metrics.engine.clone();
        {
            let context = &*self;
            visit_game_text(&engine, game_dir, &mut |sample| match sample {
                TextSample::Message { lines, has_face } => {
                    let target = if has_face { &mut faced } else { &mut plain };
                    target.extend(message_line_widths(lines, context));
                }
                TextSample::Scroll { line } => scroll.push(line_width(line, context)),
                TextSample::Choice { text } => choices.push(line_width(text, context)),
                TextSample::Description { text } => {
                    description_rows = description_rows.max(text.split('\n').count());
                    descriptions.extend(text.split('\n').map(|line| line_width(line, context)));
                }
            })?;
        }

        let mut notes = Vec::new();
        let metrics = &mut self.metrics;
        let mut widen = |label: &str, current: &mut f32, samples: &mut Vec<f32>| {
            if let Some(observed) = percentile_99(samples) {
                if observed > *current + 0.5 {
                    notes.push(format!(
                        "{label} width widened from {:.0} to {:.0} px to match the original text",
                        *current, observed
                    ));
                    *current = observed;
                }
            }
        };
        widen("Message", &mut metrics.message_width, &mut plain);
        let mut face_budget = metrics.message_width - metrics.face_offset;
        widen("Message-with-face", &mut face_budget, &mut faced);
        metrics.face_offset = (metrics.message_width - face_budget).max(0.0);
        widen("Scrolling text", &mut metrics.scroll_width, &mut scroll);
        widen("Choice", &mut metrics.choice_width, &mut choices);
        widen("Description", &mut metrics.help_width, &mut descriptions);
        if metrics.help_renders_newlines && description_rows > metrics.help_rows {
            let rows = description_rows.min(4);
            notes.push(format!(
                "Description rows raised from {} to {rows} to match the original text",
                metrics.help_rows
            ));
            metrics.help_rows = rows;
        }
        Ok(notes)
    }
}

//! String literals that MV/MZ events assign to variables from Script commands
//! (codes 355/655), such as `$gameVariables.setValue(18, "ビール")`.
//!
//! The parser and the exporter both locate literals through
//! [`variable_assignments`], so an entry ID (`…:scriptText:{command}:{n}`)
//! always points at the same literal when the translation is written back.
//! Literals are decoded from and re-encoded to JavaScript source, so control
//! codes such as `\c[2]` (written `\\c[2]` in the script) reach the translator
//! in the form the message window reads them.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

static ASSIGNMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\$gameVariables\.setValue\(\s*(\d+)\s*,\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\s*\)"#,
    )
    .expect("valid script assignment pattern")
});

static LITERAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'"#).expect("valid string literal pattern")
});

/// A string literal passed as the value of `$gameVariables.setValue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptAssignment {
    /// Position among all `setValue` calls with a literal value in the script
    /// line, including the ones that are not extractable.
    pub occurrence: usize,
    pub variable_id: u32,
    /// Byte range of the literal in the script, quotes included.
    pub start: usize,
    pub end: usize,
    pub quote: char,
    /// The text the game shows, with JavaScript escapes resolved.
    pub value: String,
}

/// `setValue(id, literal)` calls in `script` whose literal can be decoded
/// without guessing. Literals with escapes this module does not model are
/// skipped rather than risk changing what the game evaluates.
pub(crate) fn variable_assignments(script: &str) -> Vec<ScriptAssignment> {
    ASSIGNMENT_RE
        .captures_iter(script)
        .enumerate()
        .filter_map(|(occurrence, captures)| {
            let literal = captures.get(2)?;
            let quote = literal.as_str().chars().next()?;
            let value = decode_literal(literal.as_str())?;
            Some(ScriptAssignment {
                occurrence,
                variable_id: captures.get(1)?.as_str().parse().ok()?,
                start: literal.start(),
                end: literal.end(),
                quote,
                value,
            })
        })
        .collect()
}

/// Every other string literal in `script`, with the assigned ones blanked out.
/// Used to keep literals that the event also compares or looks up, which would
/// stop matching once translated.
pub(crate) fn other_literals(script: &str, into: &mut HashSet<String>) {
    let mut masked = script.to_string();
    for assignment in variable_assignments(script).iter().rev() {
        masked.replace_range(assignment.start..assignment.end, "0");
    }
    for literal in LITERAL_RE.find_iter(&masked) {
        if let Some(value) = decode_literal(literal.as_str()) {
            into.insert(value);
        }
    }
}

/// JavaScript source for `value` as a literal quoted with `quote`.
pub(crate) fn encode_literal(value: &str, quote: char) -> String {
    let mut encoded = String::with_capacity(value.len() + 2);
    encoded.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\u{2028}' | '\u{2029}' => encoded.push_str(&format!("\\u{:04x}", ch as u32)),
            ch if ch == quote => {
                encoded.push('\\');
                encoded.push(ch);
            }
            ch if (ch as u32) < 0x20 => encoded.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => encoded.push(ch),
        }
    }
    encoded.push(quote);
    encoded
}

fn decode_literal(literal: &str) -> Option<String> {
    let inner = literal.get(1..literal.len().checked_sub(1)?)?;
    let mut decoded = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        match chars.next()? {
            '\\' => decoded.push('\\'),
            '"' => decoded.push('"'),
            '\'' => decoded.push('\''),
            '/' => decoded.push('/'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            'x' => decoded.push(char::from_u32(hex_digits(&mut chars, 2)?)?),
            'u' => {
                let unit = hex_digits(&mut chars, 4)?;
                if (0xD800..0xDC00).contains(&unit) {
                    if chars.next()? != '\\' || chars.next()? != 'u' {
                        return None;
                    }
                    let low = hex_digits(&mut chars, 4)?;
                    if !(0xDC00..0xE000).contains(&low) {
                        return None;
                    }
                    decoded.push(char::from_u32(
                        0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00),
                    )?);
                } else {
                    decoded.push(char::from_u32(unit)?);
                }
            }
            _ => return None,
        }
    }
    Some(decoded)
}

fn hex_digits(chars: &mut impl Iterator<Item = char>, count: usize) -> Option<u32> {
    let mut value = 0;
    for _ in 0..count {
        value = value * 16 + chars.next()?.to_digit(16)?;
    }
    Some(value)
}

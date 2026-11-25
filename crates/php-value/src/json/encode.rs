//! `json_encode($value, $flags, $depth)` ([`encode`]), written from the
//! PHP manual and PHP 8.4.27's recorded answers
//! (`tests/data/json_encode.json`, `oracle/recorders/php_value_json.php`):
//!
//! - `null`, `true`, `false`; an int in decimal; a float with the
//!   shortest digits that read back (`serialize_precision = -1`), in
//!   exponent form (`1.0e+25`) from 10^17 and below 10^-4, `-0` for
//!   negative zero, and `.0` added to a whole number with
//!   `JSON_PRESERVE_ZERO_FRACTION`;
//! - a string with `"` `\` `/` and `\b \f \n \r \t` escaped by name,
//!   other control characters as `\u00xx`, non-ASCII as `\uxxxx`
//!   (surrogate pairs above U+FFFF) unless `JSON_UNESCAPED_UNICODE`,
//!   U+2028 and U+2029 escaped unless `JSON_UNESCAPED_LINE_TERMINATORS`
//!   too; `JSON_HEX_TAG`, `_AMP`, `_APOS` and `_QUOT` write `< > & ' "`
//!   as `\u003C \u003E \u0026 \u0027 \u0022`; `JSON_UNESCAPED_SLASHES`
//!   keeps `/`; with `JSON_NUMERIC_CHECK` a numeric string is written as
//!   its number, unless that number is infinite;
//! - invalid UTF-8 is cut into pieces: a byte that cannot start a
//!   sequence is one piece; a lead byte with the continuation bytes that
//!   follow it, up to its length, is one piece, whether it is cut short,
//!   overlong, a surrogate or above U+10FFFF. `JSON_INVALID_UTF8_IGNORE`
//!   drops each piece, `JSON_INVALID_UTF8_SUBSTITUTE` writes U+FFFD for
//!   it, and otherwise the string is `JSON_ERROR_UTF8`;
//! - a list is a JSON array unless `JSON_FORCE_OBJECT`; any other array,
//!   and an object's public properties, a JSON object with every key as
//!   a string (an object's property whose name starts with a NUL byte,
//!   a mangled non-public name, is left out); a backed enum case is its value, a unit case
//!   `JSON_ERROR_NON_BACKED_ENUM`; a `\JsonSerializable` object is what
//!   `jsonSerialize()` returns, at the object's own depth, and what that
//!   throws is passed on;
//! - each array and object opens a level: past `$depth` levels is
//!   `JSON_ERROR_DEPTH`, raised once its members are written (so an error
//!   inside comes first); `JSON_PRETTY_PRINT` writes one member per line,
//!   indented by four spaces a level, with `": "`, empty ones as `[]` or
//!   `{}`;
//! - without `JSON_PARTIAL_OUTPUT_ON_ERROR`, an invalid string value, a
//!   unit enum case or a level too deep ends the encoding with that
//!   error; INF, NAN and an invalid member name set theirs and encoding
//!   goes on (so a later error replaces it); the result is then `false`, or with
//!   `JSON_THROW_ON_ERROR` a `\JsonException`. With
//!   `JSON_PARTIAL_OUTPUT_ON_ERROR` (which wins over
//!   `JSON_THROW_ON_ERROR`) nothing stops: an invalid string is written
//!   `null` (an invalid key `""`), INF, NAN and a unit case `0`, a level
//!   too deep is written all the same, and the last error is reported.
//!
//! A [`Value`] cannot hold a reference cycle, so `JSON_ERROR_RECURSION`
//! does not arise, nor `JSON_ERROR_UNSUPPORTED_TYPE` (no resources).
//!
//! Upstream: none

use std::fmt::Write;

use super::JsonError;
use crate::{
    Key, PhpArray, Thrown, Value,
    numeric::{Number, json_float, numeric_value},
    object::EnumCase,
};

pub const JSON_HEX_TAG: i64 = 1;
pub const JSON_HEX_AMP: i64 = 2;
pub const JSON_HEX_APOS: i64 = 4;
pub const JSON_HEX_QUOT: i64 = 8;
pub const JSON_FORCE_OBJECT: i64 = 16;
pub const JSON_NUMERIC_CHECK: i64 = 32;
pub const JSON_UNESCAPED_SLASHES: i64 = 64;
pub const JSON_PRETTY_PRINT: i64 = 128;
pub const JSON_UNESCAPED_UNICODE: i64 = 256;
pub const JSON_PARTIAL_OUTPUT_ON_ERROR: i64 = 512;
pub const JSON_PRESERVE_ZERO_FRACTION: i64 = 1024;
pub const JSON_UNESCAPED_LINE_TERMINATORS: i64 = 2048;
pub const JSON_INVALID_UTF8_IGNORE: i64 = 1_048_576;
pub const JSON_INVALID_UTF8_SUBSTITUTE: i64 = 2_097_152;
pub const JSON_THROW_ON_ERROR: i64 = 4_194_304;

/// What `json_encode()` gave: the JSON text (`None` for `false`), and the
/// error `json_last_error()` reports after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    pub json: Option<String>,
    pub error: Option<JsonError>,
}

/// Why `json_encode()` threw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeFailure {
    /// `\JsonException` (`JSON_THROW_ON_ERROR`), with the error as its
    /// message and code, and as its previous exception what a
    /// `jsonSerialize()` threw after the error.
    Json {
        error: JsonError,
        previous: Option<Thrown>,
    },
    /// What a `jsonSerialize()` threw.
    Thrown(Thrown),
}

/// Why the encoding stopped early.
enum Stop {
    /// An error that ends it without `JSON_PARTIAL_OUTPUT_ON_ERROR`; it is
    /// in `Encoder::error`.
    Error,
    Thrown(Thrown),
}

struct Encoder {
    flags: i64,
    max_depth: usize,
    depth: usize,
    error: Option<JsonError>,
    out: String,
}

/// A piece of a string: a character, or bytes that are not UTF-8.
enum Piece {
    Char(char),
    Invalid,
}

/// The pieces of `bytes`, as `json_encode()` cuts them.
fn pieces(bytes: &[u8]) -> impl Iterator<Item = Piece> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        let first = *bytes.get(at)?;
        if first < 0x80 {
            at += 1;
            return Some(Piece::Char(char::from(first)));
        }
        let len = match first {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => {
                at += 1;
                return Some(Piece::Invalid);
            }
        };
        let continuation = bytes[at + 1..]
            .iter()
            .take(len - 1)
            .take_while(|b| (0x80..=0xBF).contains(*b))
            .count();
        let piece = &bytes[at..at + 1 + continuation];
        at += piece.len();
        Some(
            std::str::from_utf8(piece)
                .ok()
                .and_then(|s| s.chars().next())
                .map_or(Piece::Invalid, Piece::Char),
        )
    })
}

/// `json_encode($string, $flags)` of a string's characters, quoted; `None`
/// for invalid UTF-8 that the flags neither drop nor replace.
pub(super) fn escape(bytes: &[u8], flags: i64) -> Option<String> {
    let mut out = String::with_capacity(bytes.len() + 2);
    out.push('"');
    for piece in pieces(bytes) {
        let c = match piece {
            Piece::Char(c) => c,
            Piece::Invalid if flags & JSON_INVALID_UTF8_IGNORE != 0 => continue,
            Piece::Invalid if flags & JSON_INVALID_UTF8_SUBSTITUTE != 0 => '\u{FFFD}',
            Piece::Invalid => return None,
        };
        match c {
            '"' if flags & JSON_HEX_QUOT != 0 => out.push_str("\\u0022"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' if flags & JSON_UNESCAPED_SLASHES == 0 => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' if flags & JSON_HEX_TAG != 0 => out.push_str("\\u003C"),
            '>' if flags & JSON_HEX_TAG != 0 => out.push_str("\\u003E"),
            '&' if flags & JSON_HEX_AMP != 0 => out.push_str("\\u0026"),
            '\'' if flags & JSON_HEX_APOS != 0 => out.push_str("\\u0027"),
            '\u{0}'..='\u{1f}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            '\u{2028}' | '\u{2029}'
                if flags & JSON_UNESCAPED_UNICODE == 0
                    || flags & JSON_UNESCAPED_LINE_TERMINATORS == 0 =>
            {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c if !c.is_ascii() && flags & JSON_UNESCAPED_UNICODE == 0 => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    Some(out)
}

impl Encoder {
    fn partial(&self) -> bool {
        self.flags & JSON_PARTIAL_OUTPUT_ON_ERROR != 0
    }

    /// Records an error: without partial output, one that stops ends the
    /// encoding.
    fn fail(&mut self, error: JsonError, stops: bool) -> Result<(), Stop> {
        self.error = Some(error);
        if stops && !self.partial() {
            return Err(Stop::Error);
        }
        Ok(())
    }

    fn newline(&mut self) {
        if self.flags & JSON_PRETTY_PRINT != 0 {
            self.out.push('\n');
            for _ in 0..self.depth {
                self.out.push_str("    ");
            }
        }
    }

    fn float(&mut self, f: f64) -> Result<(), Stop> {
        let Some(mut text) = json_float(f) else {
            self.out.push('0');
            return self.fail(JsonError::InfOrNan, false);
        };
        if self.flags & JSON_PRESERVE_ZERO_FRACTION != 0 && !text.contains(['.', 'e']) {
            text.push_str(".0");
        }
        self.out.push_str(&text);
        Ok(())
    }

    /// A string. An invalid member name is written `""` and does not stop
    /// the encoding; an invalid value is written `null`.
    fn string(&mut self, bytes: &[u8], key: bool) -> Result<(), Stop> {
        if !key && self.flags & JSON_NUMERIC_CHECK != 0 {
            match numeric_value(bytes) {
                Some(Number::Int(i)) => {
                    let _ = write!(self.out, "{i}");
                    return Ok(());
                }
                Some(Number::Float(f)) if f.is_finite() => return self.float(f),
                _ => {}
            }
        }
        if let Some(text) = escape(bytes, self.flags) {
            self.out.push_str(&text);
            Ok(())
        } else {
            self.out.push_str(if key { "\"\"" } else { "null" });
            self.fail(JsonError::Utf8, !key)
        }
    }

    fn value(&mut self, value: &Value) -> Result<(), Stop> {
        match value {
            Value::Null => self.out.push_str("null"),
            Value::Bool(true) => self.out.push_str("true"),
            Value::Bool(false) => self.out.push_str("false"),
            Value::Int(i) => {
                let _ = write!(self.out, "{i}");
            }
            Value::Float(f) => return self.float(*f),
            Value::String(s) => return self.string(s.as_bytes(), false),
            Value::Array(array) => {
                let list = self.flags & JSON_FORCE_OBJECT == 0 && array.is_list();
                return self.members(array, list);
            }
            Value::Object(object) => {
                if let Some(case) = object.downcast_ref::<EnumCase>() {
                    return if let Some(backing) = case.value() {
                        self.value(backing)
                    } else {
                        self.out.push('0');
                        self.fail(JsonError::NonBackedEnum, true)
                    };
                }
                if let Some(serialized) = object.json_serialize() {
                    return self.value(&serialized.map_err(Stop::Thrown)?);
                }
                // a name starting with NUL is a mangled (non-public) one
                let mut properties = PhpArray::new();
                for (name, property) in object.properties().iter() {
                    if !matches!(name, Key::Str(s) if s.as_bytes().first() == Some(&0)) {
                        properties.insert(name.clone(), property.clone());
                    }
                }
                return self.members(&properties, false);
            }
        }
        Ok(())
    }

    /// An array or object: a level deeper.
    fn members(&mut self, members: &PhpArray, list: bool) -> Result<(), Stop> {
        self.depth += 1;
        let (open, close) = if list { ('[', ']') } else { ('{', '}') };
        self.out.push(open);
        for (i, (key, value)) in members.iter().enumerate() {
            if i > 0 {
                self.out.push(',');
            }
            self.newline();
            if !list {
                match key {
                    Key::Int(n) => {
                        let _ = write!(self.out, "\"{n}\"");
                    }
                    Key::Str(s) => self.string(s.as_bytes(), true)?,
                }
                self.out.push(':');
                if self.flags & JSON_PRETTY_PRINT != 0 {
                    self.out.push(' ');
                }
            }
            self.value(value)?;
        }
        // PHP checks the depth once the members are written.
        if self.depth > self.max_depth {
            self.fail(JsonError::Depth, true)?;
        }
        self.depth -= 1;
        if !members.is_empty() {
            self.newline();
        }
        self.out.push(close);
        Ok(())
    }
}

/// `json_encode($value, $flags, $depth)`. The flags are PHP's `JSON_*`
/// bits; those that only `json_decode()` reads are ignored, as PHP does.
///
/// Fails with what a `jsonSerialize()` threw, or with
/// `JSON_THROW_ON_ERROR` (and no `JSON_PARTIAL_OUTPUT_ON_ERROR`) a
/// `\JsonException` for the error, which also wraps what a
/// `jsonSerialize()` threw after an error. In that mode PHP leaves
/// `json_last_error()` as it was, and a success has no `error` here.
pub fn encode(value: &Value, flags: i64, depth: usize) -> Result<Encoded, EncodeFailure> {
    let mut encoder = Encoder {
        flags,
        max_depth: depth,
        depth: 0,
        error: None,
        out: String::new(),
    };
    let partial = encoder.partial();
    let throw = !partial && flags & JSON_THROW_ON_ERROR != 0;
    match encoder.value(value) {
        Err(Stop::Thrown(thrown)) => {
            return Err(match encoder.error {
                Some(error) if throw => EncodeFailure::Json {
                    error,
                    previous: Some(thrown),
                },
                _ => EncodeFailure::Thrown(thrown),
            });
        }
        Ok(()) | Err(Stop::Error) => {}
    }
    match encoder.error {
        Some(error) if throw => Err(EncodeFailure::Json {
            error,
            previous: None,
        }),
        Some(error) if !partial => Ok(Encoded {
            json: None,
            error: Some(error),
        }),
        error => Ok(Encoded {
            json: Some(encoder.out),
            error,
        }),
    }
}

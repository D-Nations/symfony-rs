//! Port of Symfony\Component\HttpFoundation\HeaderUtils.php: `split()`,
//! `combine()`, `toString()`, `quote()` and `unquote()`.
//!
//! `split()`'s pattern is matched by hand, in one pass, as the fixed
//! pattern of GHSA-r925-mgvr-h9cc (8.1.7) matches: a value never ends on
//! whitespace, so nothing is given back. Results are PHP arrays of byte
//! strings, nested once per separator.
//!
//! Not ported: `makeDisposition()` (the pin, v8.1.10, lacks v8.1.11's
//! `/D` anchor on the ASCII check; it waits for the version move, PLAN.md
//! "Decisions for the owner") and `parseQuery()` (it needs `parse_str()`
//! and `http_build_query()`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation HeaderUtils.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::exception::{Class, Error, Result};

pub const DISPOSITION_ATTACHMENT: &str = "attachment";
pub const DISPOSITION_INLINE: &str = "inline";

/// The characters PHP's `trim()` removes by default.
const PHP_TRIM: &[u8] = b" \t\n\r\0\x0B";

/// PCRE's `\s` without the `u` flag.
pub(crate) const fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r')
}

/// One match of `split()`'s pattern: a value run, or a separator with the
/// whitespace around it (`$match[0]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    Value(Vec<u8>),
    Separator(u8, Vec<u8>),
}

impl Token {
    /// `$match[0]`.
    fn text(&self) -> &[u8] {
        match self {
            Self::Value(text) | Self::Separator(_, text) => text,
        }
    }

    /// `$match['separator'] ?? null`.
    fn separator(&self) -> Option<u8> {
        match self {
            Self::Separator(separator, _) => Some(*separator),
            Self::Value(_) => None,
        }
    }
}

/// The matches of `split()`'s pattern over the trimmed header, in order
/// (`preg_match_all(..., trim($header), $matches, \PREG_SET_ORDER)`).
pub(crate) fn tokenize(header: &[u8], separators: &[u8]) -> Vec<Token> {
    let start = header
        .iter()
        .position(|byte| !PHP_TRIM.contains(byte))
        .unwrap_or(header.len());
    let end = header
        .iter()
        .rposition(|byte| !PHP_TRIM.contains(byte))
        .map_or(start, |last| last + 1);
    let text = &header[start..end];
    let is_separator = |byte: u8| separators.contains(&byte);

    let mut tokens = Vec::new();
    let mut i = 0;
    while i < text.len() {
        // A separator, with the whitespace before and after it:
        // `\s*(?<separator>[...])\s*`. The leading `\s*` gives back
        // whitespace until a separator follows, so a whitespace separator
        // inside the run is found from its end.
        let run_end = text[i..]
            .iter()
            .position(|byte| !is_space(*byte))
            .map_or(text.len(), |offset| i + offset);
        let at = if run_end < text.len() && is_separator(text[run_end]) {
            Some(run_end)
        } else {
            (i..run_end).rev().find(|&p| is_separator(text[p]))
        };
        let value_starts = !is_space(text[i]) && !is_separator(text[i]);
        if let (Some(at), false) = (at, value_starts) {
            let mut end = at + 1;
            while end < text.len() && is_space(text[end]) {
                end += 1;
            }
            tokens.push(Token::Separator(text[at], text[i..end].to_vec()));
            i = end;
            continue;
        }
        // A value never starts with whitespace or a separator: no match
        // here, nor anywhere else in this run of whitespace (it holds no
        // separator and ends on none), so go past it. This keeps the scan
        // linear.
        if !value_starts {
            i = run_end.max(i + 1);
            continue;
        }
        let begin = i;
        while i < text.len() {
            let byte = text[i];
            if byte == b'"' {
                // A quoted string, possibly unterminated.
                i += 1;
                while i < text.len() && text[i] != b'"' {
                    i += if text[i] == b'\\' && i + 1 < text.len() {
                        2
                    } else {
                        1
                    };
                }
                i = (i + 1).min(text.len());
            } else if is_separator(byte) {
                break;
            } else if is_space(byte) {
                // Whitespace that is not a separator belongs to the value
                // only when more of the value follows it.
                let run_end = text[i..]
                    .iter()
                    .position(|byte| !is_space(*byte) || is_separator(*byte))
                    .map_or(text.len(), |offset| i + offset);
                if run_end == text.len() || is_separator(text[run_end]) {
                    break;
                }
                i = run_end;
            } else {
                i += 1;
            }
        }
        tokens.push(Token::Value(text[begin..i].to_vec()));
    }
    tokens
}

/// `HeaderUtils::split($header, $separators)`.
pub fn split(header: &[u8], separators: &[u8]) -> Result<PhpArray> {
    if separators.is_empty() {
        return Err(Error::new(
            Class::PhpInvalidArgument,
            "At least one separator must be specified.",
        ));
    }
    Ok(group_parts(&tokenize(header, separators), separators, true))
}

fn string(bytes: Vec<u8>) -> Value {
    Value::String(PhpString::from(bytes))
}

/// `self::groupParts($matches, $separators, $first)`.
///
/// With one separator, a group whose first match unquotes to `''` recurses
/// with none left: PHP reads `$separators[0]` of `''` (an "Uninitialized
/// string offset" warning, not reproduced) and takes the innermost branch.
fn group_parts(matches: &[Token], separators: &[u8], first: bool) -> PhpArray {
    let separator = separators.first().copied();
    let rest = separators.get(1..).unwrap_or_default();

    if rest.is_empty() && !first {
        let mut parts: Vec<Vec<u8>> = vec![Vec::new()];
        for token in matches {
            if parts.len() == 1 && token.separator().is_some() {
                parts.push(Vec::new());
            } else if let Some(part) = parts.last_mut() {
                part.extend(unquote_bytes(token.text()));
            }
        }
        return PhpArray::list(parts.into_iter().map(string));
    }

    let mut groups: Vec<Vec<Token>> = Vec::new();
    let mut current: Option<Vec<Token>> = None;
    for token in matches {
        if separator.is_some() && token.separator() == separator {
            groups.extend(current.take());
        } else {
            current.get_or_insert_with(Vec::new).push(token.clone());
        }
    }
    groups.extend(current);

    let mut parts = Vec::new();
    for group in groups {
        if rest.is_empty() {
            let unquoted = unquote_bytes(group[0].text());
            if !unquoted.is_empty() {
                parts.push(string(unquoted));
                continue;
            }
        }
        let grouped = group_parts(&group, rest, false);
        if !grouped.is_empty() {
            parts.push(Value::Array(grouped));
        }
    }
    PhpArray::list(parts)
}

/// `HeaderUtils::combine($parts)`: `[name, value]` pairs as an
/// associative array, names lower-cased (ASCII, as PHP 8's `strtolower()`),
/// a missing value as `true`.
pub fn combine(parts: &PhpArray) -> Result<PhpArray> {
    let mut assoc = PhpArray::new();
    for (_, part) in parts.iter() {
        let Value::Array(part) = part else {
            return Err(Error::new(
                Class::NotPorted,
                "combine() of a non-array part",
            ));
        };
        let name = part
            .get(&Key::Int(0))
            .and_then(Value::to_php_string)
            .ok_or_else(|| Error::new(Class::NotPorted, "combine() of a part without a name"))?;
        let value = part.get(&Key::Int(1)).cloned().filter(|v| !v.is_null());
        assoc.insert(
            Key::from(PhpString::from(name.as_bytes().to_ascii_lowercase())),
            value.unwrap_or(Value::Bool(true)),
        );
    }
    Ok(assoc)
}

/// `HeaderUtils::toString($assoc, $separator)`: `name=value` entries (a
/// `true` value as the name alone), joined by the separator and a space.
pub fn to_string(assoc: &PhpArray, separator: &[u8]) -> Result<PhpString> {
    let mut parts: Vec<Vec<u8>> = Vec::new();
    for (name, value) in assoc.iter() {
        let name = name.to_value().to_php_string().unwrap_or_default();
        if matches!(value, Value::Bool(true)) {
            parts.push(name.as_bytes().to_vec());
            continue;
        }
        let Some(value) = (!value.is_null()).then(|| value.to_php_string()).flatten() else {
            return Err(Error::new(
                Class::NotPorted,
                format!("toString() of a {} value", value.debug_type()),
            ));
        };
        let mut part = name.as_bytes().to_vec();
        part.push(b'=');
        part.extend_from_slice(quote(value.as_bytes()).as_bytes());
        parts.push(part);
    }
    let glue = [separator, b" "].concat();
    Ok(PhpString::from(parts.join(glue.as_slice())))
}

/// The characters of HTTP's "token".
fn is_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*.^_`|~-".contains(&byte)
}

/// `HeaderUtils::quote($s)`: the string as it is when it is a token, else
/// a quoted string with `"` and `\` escaped. The token check is
/// `/^[...]+$/i` without `D`, so one final `\n` after the token still
/// passes it, as recorded.
pub fn quote(s: &[u8]) -> PhpString {
    let body = s.strip_suffix(b"\n").unwrap_or(s);
    if !body.is_empty() && body.iter().all(|&b| is_token_char(b)) {
        return PhpString::from(s);
    }
    let mut out = Vec::with_capacity(s.len() + 2);
    out.push(b'"');
    for &byte in s {
        if matches!(byte, b'"' | b'\\') {
            out.push(b'\\');
        }
        out.push(byte);
    }
    out.push(b'"');
    PhpString::from(out)
}

/// `HeaderUtils::unquote($s)`.
pub fn unquote(s: &[u8]) -> PhpString {
    PhpString::from(unquote_bytes(s))
}

/// `preg_replace('/\\\\(.)|"/', '$1', $s)`: drops quotes, and keeps the
/// character after each backslash. `.` does not match a newline, so a
/// backslash before `\n` (or at the end) stays.
pub(crate) fn unquote_bytes(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        match text[i] {
            b'\\' if i + 1 < text.len() && text[i + 1] != b'\n' => {
                out.push(text[i + 1]);
                i += 2;
            }
            b'"' => i += 1,
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    out
}

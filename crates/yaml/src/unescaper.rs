//! Port of Symfony\Component\Yaml\Unescaper.php: unescaping rules for
//! single and double-quoted YAML strings.
//!
//! `REGEX_ESCAPED_CHARACTER` (`\\(x[0-9a-fA-F]{2}|u[0-9a-fA-F]{4}|
//! U[0-9a-fA-F]{8}|.)` with `/u`) is matched by hand. As in PCRE, the
//! alternatives are tried in order and `.` is any one character except a
//! line feed; so `\xZZ` matches as `\x`, whose `hexdec("")` gives a NUL
//! byte, and a backslash before a line feed or at the end stays as is.
//! The result is bytes: `\uD800` and code points above U+10FFFF encode
//! as PHP's `utf8chr()` does, which is not valid UTF-8.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Unescaper.php

use php_value::numeric::{Number, hexdec};

use crate::exception::ParseException;

/// Unescapes a single quoted string: `''` is `'`.
pub fn unescape_single_quoted_string(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\'' && value.get(i + 1) == Some(&b'\'') {
            out.push(b'\'');
            i += 2;
        } else {
            out.push(value[i]);
            i += 1;
        }
    }
    out
}

/// Unescapes a double quoted string. `value` is UTF-8, as the parser
/// checks before it gets here.
pub fn unescape_double_quoted_string(value: &str) -> Result<Vec<u8>, ParseException> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let rest = &value[i + 1..];
        let hex =
            |n: usize| rest.len() > n && rest.as_bytes()[1..=n].iter().all(u8::is_ascii_hexdigit);
        let len = match rest.as_bytes().first() {
            Some(b'x') if hex(2) => 4,
            Some(b'u') if hex(4) => 6,
            Some(b'U') if hex(8) => 10,
            Some(b'\n') | None => {
                out.push(b'\\');
                i += 1;
                continue;
            }
            // `.`: one whole character.
            Some(_) => 1 + rest.chars().next().map_or(1, char::len_utf8),
        };
        let escape = &bytes[i..i + len];
        out.extend_from_slice(&unescape_character(escape)?);
        i += len;
    }
    Ok(out)
}

/// `unescapeCharacter()`: `escape` is a backslash and what followed it.
fn unescape_character(escape: &[u8]) -> Result<Vec<u8>, ParseException> {
    let code = |digits: &[u8]| match hexdec(digits) {
        Number::Int(i) => i,
        // At most 8 hex digits: always an int.
        Number::Float(f) => f as i64,
    };
    Ok(match escape[1] {
        b'0' => vec![0x00],
        b'a' => vec![0x07],
        b'b' => vec![0x08],
        b't' | b'\t' => vec![b'\t'],
        b'n' => vec![b'\n'],
        b'v' => vec![0x0b],
        b'f' => vec![0x0c],
        b'r' => vec![b'\r'],
        b'e' => vec![0x1b],
        b' ' => vec![b' '],
        b'"' => vec![b'"'],
        b'/' => vec![b'/'],
        b'\\' => vec![b'\\'],
        // U+0085 NEXT LINE
        b'N' => vec![0xc2, 0x85],
        // U+00A0 NO-BREAK SPACE
        b'_' => vec![0xc2, 0xa0],
        // U+2028 LINE SEPARATOR
        b'L' => vec![0xe2, 0x80, 0xa8],
        // U+2029 PARAGRAPH SEPARATOR
        b'P' => vec![0xe2, 0x80, 0xa9],
        b'x' | b'u' | b'U' => utf8chr(code(&escape[2..])),
        _ => {
            return Err(ParseException::message_only(format!(
                "Found unknown escape character \"{}\".",
                String::from_utf8_lossy(escape)
            )));
        }
    })
}

/// `utf8chr()`: the UTF-8-style bytes of a code point, modulo 0x200000.
fn utf8chr(c: i64) -> Vec<u8> {
    let c = c.rem_euclid(0x0020_0000) as u32;
    let byte = |v: u32| (v & 0xff) as u8;
    if c < 0x80 {
        vec![byte(c)]
    } else if c < 0x800 {
        vec![byte(0xc0 | (c >> 6)), byte(0x80 | (c & 0x3f))]
    } else if c < 0x10000 {
        vec![
            byte(0xe0 | (c >> 12)),
            byte(0x80 | ((c >> 6) & 0x3f)),
            byte(0x80 | (c & 0x3f)),
        ]
    } else {
        vec![
            byte(0xf0 | (c >> 18)),
            byte(0x80 | ((c >> 12) & 0x3f)),
            byte(0x80 | ((c >> 6) & 0x3f)),
            byte(0x80 | (c & 0x3f)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_escapes_need_their_digits() -> Result<(), ParseException> {
        assert_eq!(
            unescape_double_quoted_string("\\x41\\u00e9\\U0001F600")?,
            "Aé😀".as_bytes()
        );
        // `\x` without two hex digits matches the `.` alternative.
        assert_eq!(unescape_double_quoted_string("\\xZZ")?, b"\0ZZ");
        // A backslash before a line feed, or last, is kept.
        assert_eq!(unescape_double_quoted_string("a\\\nb\\")?, b"a\\\nb\\");
        Ok(())
    }

    #[test]
    fn unknown_escapes_fail_with_upstream_message() {
        let error = unescape_double_quoted_string("\\q").err();
        assert_eq!(
            error.map(|e| e.get_message().to_string()),
            Some("Found unknown escape character \"\\q\".".to_owned())
        );
    }

    #[test]
    fn single_quotes_are_doubled() {
        assert_eq!(unescape_single_quoted_string(b"it''s ''''"), b"it's ''");
    }
}

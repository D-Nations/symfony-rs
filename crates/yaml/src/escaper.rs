//! Port of Symfony\Component\Yaml\Escaper.php: escaping rules for
//! double and single-quoted YAML strings.
//!
//! The regular expressions are matched by hand, with PCRE's `/u` classes
//! from php-value (`\s` and `\p{Zs}` include more than ASCII). A string
//! that is not UTF-8 makes PHP's `preg_match()` fail, which these
//! functions report as "not required", as `0 < false` does in PHP.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Escaper.php

use php_value::pcre::{is_space_u, is_zs_u};

/// Mapping for escaping a double quoted string, applied one pair after
/// the other as `str_replace()` does with arrays. The backslash is first.
const ESCAPEES: [&[u8]; 41] = [
    b"\\",
    b"\\\\",
    b"\\\"",
    b"\"",
    b"\x00",
    b"\x01",
    b"\x02",
    b"\x03",
    b"\x04",
    b"\x05",
    b"\x06",
    b"\x07",
    b"\x08",
    b"\x09",
    b"\x0a",
    b"\x0b",
    b"\x0c",
    b"\x0d",
    b"\x0e",
    b"\x0f",
    b"\x10",
    b"\x11",
    b"\x12",
    b"\x13",
    b"\x14",
    b"\x15",
    b"\x16",
    b"\x17",
    b"\x18",
    b"\x19",
    b"\x1a",
    b"\x1b",
    b"\x1c",
    b"\x1d",
    b"\x1e",
    b"\x1f",
    b"\x7f",
    b"\xc2\x85",
    b"\xc2\xa0",
    b"\xe2\x80\xa8",
    b"\xe2\x80\xa9",
];
const ESCAPED: [&[u8]; 41] = [
    b"\\\\", b"\\\"", b"\\\\", b"\\\"", b"\\0", b"\\x01", b"\\x02", b"\\x03", b"\\x04", b"\\x05",
    b"\\x06", b"\\a", b"\\b", b"\\t", b"\\n", b"\\v", b"\\f", b"\\r", b"\\x0e", b"\\x0f", b"\\x10",
    b"\\x11", b"\\x12", b"\\x13", b"\\x14", b"\\x15", b"\\x16", b"\\x17", b"\\x18", b"\\x19",
    b"\\x1a", b"\\e", b"\\x1c", b"\\x1d", b"\\x1e", b"\\x1f", b"\\x7f", b"\\N", b"\\_", b"\\L",
    b"\\P",
];

/// `str_replace($search, $replace, $subject)` for one pair: left to
/// right, without rescanning replaced text.
pub(crate) fn str_replace(subject: &[u8], search: &[u8], replace: &[u8]) -> Vec<u8> {
    if search.is_empty() {
        return subject.to_vec();
    }
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(search) {
            out.extend_from_slice(replace);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// Whether a string needs double quoting: it contains a control
/// character, DEL, NEL, NBSP, LS or PS.
pub fn requires_double_quoting(value: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(value) else {
        return false;
    };
    text.chars().any(|c| {
        matches!(
            c,
            '\u{0}'..='\u{1f}' | '\u{7f}' | '\u{85}' | '\u{a0}' | '\u{2028}' | '\u{2029}'
        )
    })
}

/// Escapes and surrounds a string with double quotes.
pub fn escape_with_double_quotes(value: &[u8]) -> Vec<u8> {
    let mut escaped = value.to_vec();
    for (search, replace) in ESCAPEES.iter().zip(ESCAPED.iter()) {
        escaped = str_replace(&escaped, search, replace);
    }
    let mut out = Vec::with_capacity(escaped.len() + 2);
    out.push(b'"');
    out.extend_from_slice(&escaped);
    out.push(b'"');
    out
}

/// Whether a string needs single quoting: it is a YAML keyword, contains
/// one of `\s ' " : { } [ ] , & * # ?`, or starts with one of
/// `- ? | < > = ! % @` `` ` `` or a space separator.
pub fn requires_single_quoting(value: &[u8]) -> bool {
    let lower = value.to_ascii_lowercase();
    const KEYWORDS: [&[u8]; 13] = [
        b"null", b"~", b"true", b"false", b"y", b"n", b"yes", b"no", b"on", b"off", b".inf",
        b".nan", b"...",
    ];
    if KEYWORDS.contains(&lower.as_slice()) {
        return true;
    }
    let Ok(text) = std::str::from_utf8(value) else {
        return false;
    };
    let special = |c: char| {
        is_space_u(c)
            || matches!(
                c,
                '\'' | '"' | ':' | '{' | '}' | '[' | ']' | ',' | '&' | '*' | '#' | '?'
            )
    };
    if text.chars().any(special) {
        return true;
    }
    text.chars().next().is_some_and(|c| {
        is_zs_u(c) || matches!(c, '-' | '?' | '|' | '<' | '>' | '=' | '!' | '%' | '@' | '`')
    })
}

/// Escapes and surrounds a string with single quotes.
pub fn escape_with_single_quotes(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 2);
    out.push(b'\'');
    out.extend_from_slice(&str_replace(value, b"'", b"''"));
    out.push(b'\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_quoting_escapes_in_upstream_order() {
        assert_eq!(escape_with_double_quotes(b"a\\b\"c"), b"\"a\\\\b\\\"c\"");
        assert_eq!(escape_with_double_quotes(b"\\\""), b"\"\\\\\\\"\"");
        assert_eq!(
            escape_with_double_quotes(b"\0\x07\x1b\xc2\xa0"),
            b"\"\\0\\a\\e\\_\""
        );
    }

    #[test]
    fn quoting_requirements() {
        assert!(requires_double_quoting(b"tab\there"));
        assert!(!requires_double_quoting(b"plain"));
        assert!(requires_single_quoting(b"Yes"));
        assert!(requires_single_quoting(b"a: b"));
        assert!(requires_single_quoting(b"@at"));
        // NEL is \s under /u.
        assert!(requires_single_quoting("a\u{85}b".as_bytes()));
        assert!(!requires_single_quoting(b"plain-text"));
    }
}

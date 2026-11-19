//! `json_encode()` of strings, as recorded from PHP 8.4.27
//! (tests/data/php.json): `"` `\` and `\b \f \n \r \t` escaped by name,
//! other control characters as `\u00XX`, DEL kept, `/` as `\/` unless
//! `JSON_UNESCAPED_SLASHES`, non-ASCII as `\uXXXX` (surrogate pairs above
//! U+FFFF) unless `JSON_UNESCAPED_UNICODE`, U+2028 and U+2029 always
//! escaped, and `false` (here `None`) for invalid UTF-8.
//!
//! Upstream: none

use std::fmt::Write;

/// The `json_encode()` flags that affect strings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsonFlags {
    pub unescaped_slashes: bool,
    pub unescaped_unicode: bool,
}

/// `json_encode($string, $flags)`.
pub fn encode_string(bytes: &[u8], flags: JsonFlags) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' if !flags.unescaped_slashes => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0}'..='\u{1f}' | '\u{2028}' | '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c if !c.is_ascii() && !flags.unescaped_unicode => {
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

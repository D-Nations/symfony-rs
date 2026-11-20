//! PHP built-ins the fixture calls that `php-value` does not have yet:
//! `implode()`, `ucfirst()`, `strtolower()`, `trim()` and
//! `htmlspecialchars($s, ENT_QUOTES, 'UTF-8')`. Written from the PHP manual
//! (no php-src, PLAN.md decision 8); the fixture's recorded responses check
//! the cases they reach. They belong in `php-value` with recorded vectors.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString};

use super::NotPorted;

/// `implode($separator, $array)` for scalar and null elements, each as
/// `(string) $value`. Arrays (`"Array"` with a warning) and objects
/// (`__toString()` or an `Error`) are refused.
pub fn implode(separator: &[u8], array: &PhpArray) -> Result<PhpString, NotPorted> {
    let mut out = Vec::new();
    for (index, value) in array.values().enumerate() {
        if index > 0 {
            out.extend_from_slice(separator);
        }
        let text = value
            .to_php_string()
            .ok_or(NotPorted("implode() of an array or object element"))?;
        out.extend_from_slice(text.as_bytes());
    }
    Ok(PhpString::from(out))
}

/// `ucfirst()`: since PHP 8.0 locale-insensitive, ASCII `a-z` only.
pub fn ucfirst(s: &[u8]) -> Vec<u8> {
    let mut out = s.to_vec();
    if let Some(first) = out.first_mut() {
        first.make_ascii_uppercase();
    }
    out
}

/// `strtolower()`: since PHP 8.2 locale-insensitive, ASCII `A-Z` only.
pub fn strtolower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// `trim()` with its default characters: space, `\t`, `\n`, `\r`, `\0`, `\x0B`.
pub fn trim(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B'])
}

/// `htmlspecialchars($s, ENT_QUOTES, 'UTF-8')`: `&`, `"`, `'`, `<`, `>`
/// escaped (`'` as `&#039;`). Without `ENT_SUBSTITUTE`, invalid UTF-8
/// gives `''`; a `&str` is always valid.
pub fn htmlspecialchars_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

//! PHP built-ins the fixture calls that `php-value` does not have yet:
//! `implode()` and `ucfirst()`. Written from the PHP manual
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

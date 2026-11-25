//! The PHP built-ins `CssSelector` relies on, written from PHP's documented
//! behaviour and checked by the recorded vectors (tests/data/php.json);
//! no PHP source code (PLAN.md decision 8). Strings are bytes, as in PHP.
//!
//! Upstream: none

/// `(bool) $string`: false for `""` and `"0"`.
pub(crate) use php_value::string::truthy;

/// `(bool) $nullableString`.
pub(crate) fn truthy_opt(s: Option<&[u8]>) -> bool {
    s.is_some_and(truthy)
}

/// `$string ?: null`.
pub(crate) fn or_null(s: Option<&[u8]>) -> Option<Vec<u8>> {
    s.filter(|s| truthy(s)).map(<[u8]>::to_vec)
}

/// `strtolower()`: ASCII only, as in PHP 8.
pub(crate) fn strtolower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

/// `trim()` with its default characters (`" \t\n\r\0\x0B"`).
pub(crate) use php_value::string::trim;

/// `substr($string, $start, $length)` for non-negative `$start` and
/// `$length`.
pub(crate) fn substr(s: &[u8], start: usize, length: usize) -> &[u8] {
    let start = start.min(s.len());
    let end = start.saturating_add(length).min(s.len());
    &s[start..end]
}

/// `strpos($haystack, $needle, $offset)` for `$offset <= strlen($haystack)`.
pub(crate) fn strpos(haystack: &[u8], needle: &[u8], offset: usize) -> Option<usize> {
    let rest = haystack.get(offset..)?;
    if needle.is_empty() {
        return Some(offset);
    }
    rest.windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + offset)
}

/// `(string) $int` of an integer result, which PHP turns into a float
/// once it leaves the `int` range (the series arithmetic adds or takes 1).
pub(crate) fn int_string(value: i128) -> Vec<u8> {
    match i64::try_from(value) {
        Ok(v) => v.to_string().into_bytes(),
        Err(_) => php_value::numeric::float_to_string(value as f64).into_bytes(),
    }
}

/// Concatenation of byte strings.
pub(crate) fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

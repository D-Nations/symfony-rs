//! The PHP built-ins `EnvVarProcessor` calls that php-value does not have:
//! `filter_var($value, FILTER_VALIDATE_FLOAT)`, the `(int)` cast of a
//! float, `str_getcsv($string, ',', '"', '')` and `shuffle()`. Written
//! from the PHP manual and from PHP 8.4.27's answers (probed in the
//! `php:8.4.27-cli-bookworm` image, and recorded by
//! `oracle/recorders/di_env_var_processor.php` into
//! `tests/data/env_var_processor.json`); no PHP source code.
//!
//! What was observed:
//!
//! - `FILTER_VALIDATE_FLOAT` ignores spaces, tabs, `\n`, `\r` and `\v`
//!   around the value (not `\f` or NUL), as the int filter does. It takes
//!   an optional sign, digits with at most one `.` (at least one digit),
//!   and an optional exponent `e`/`E`, sign and digits. A number without
//!   `.` or exponent that fits in an int is read as that int (`"-0"` is
//!   `0.0`, `"-0.0"` is `-0.0`). An infinite result fails, and so does a
//!   zero result when an exponent other than 0 was written (`"0e5"`,
//!   `"1e-400"`; `"0e0"` is `0.0`).
//! - `(int)` of a float outside the int range wraps modulo 2^64; NAN and
//!   infinities are 0.
//! - `str_getcsv()` with an empty escape: the input is one record (line
//!   breaks do not end it). A record that is only a line break is
//!   `[null]`. Fields are split at `,`. A field whose first byte after
//!   leading C-locale whitespace (space, `\t`, `\n`, `\v`, `\f`, `\r`) is
//!   `"` is enclosed: `""` inside is `"`, the next lone `"` closes it and
//!   whatever follows up to the next `,` is appended as is; without a
//!   closing `"` the field runs to the end of the input, line breaks
//!   included. Otherwise the field is its bytes, leading whitespace kept.
//!   Trailing `\r` and `\n` bytes of a field outside the enclosure are
//!   dropped.
//!
//! Upstream: none

use std::hash::{BuildHasher, Hasher, RandomState};

use php_value::{PhpArray, Value};

/// The whitespace `FILTER_VALIDATE_FLOAT` ignores around a value.
const fn is_filter_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0B')
}

/// `filter_var($value, FILTER_VALIDATE_FLOAT)`: the float, or `None` for
/// `false`. Arrays and objects other than `\Stringable` ones fail.
pub(crate) fn validate_float(value: &Value) -> Option<f64> {
    let text = value.to_php_string_or_stringable()?;
    let text = text.as_bytes();
    let start = text
        .iter()
        .position(|&b| !is_filter_space(b))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|&b| !is_filter_space(b))
        .map_or(start, |last| last + 1);
    let s = &text[start..end];

    let mut i = usize::from(matches!(s.first(), Some(b'+' | b'-')));
    let digits = |from: usize| s[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    let int_digits = digits(i);
    i += int_digits;
    let mut fraction = false;
    let mut frac_digits = 0;
    if s.get(i) == Some(&b'.') {
        fraction = true;
        i += 1;
        frac_digits = digits(i);
        i += frac_digits;
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    let mut exponent = None;
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(s.get(i + 1), Some(b'+' | b'-')));
        let n = digits(i + 1 + sign);
        if n == 0 {
            return None;
        }
        exponent = Some(&s[i + 1 + sign..i + 1 + sign + n]);
        i += 1 + sign + n;
    }
    if i != s.len() {
        return None;
    }
    let text = std::str::from_utf8(s).ok()?;
    if !fraction
        && exponent.is_none()
        && let Ok(int) = text.parse::<i64>()
    {
        #[expect(clippy::cast_precision_loss, reason = "PHP's int to float conversion")]
        return Some(int as f64);
    }
    let float: f64 = text.parse().ok()?;
    if float.is_infinite() {
        return None;
    }
    let nonzero_exponent = exponent.is_some_and(|e| e.iter().any(|&b| b != b'0'));
    if float == 0.0 && nonzero_exponent {
        return None;
    }
    Some(float)
}

/// `(int) $float`: truncated, wrapping modulo 2^64 outside the int range,
/// 0 for NAN and infinities.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "PHP's float to int conversion"
)]
pub(crate) fn float_to_int(float: f64) -> i64 {
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    const TWO_POW_64: f64 = 18_446_744_073_709_551_616.0;
    if !float.is_finite() {
        return 0;
    }
    if (-TWO_POW_63..TWO_POW_63).contains(&float) {
        return float as i64;
    }
    let mut wrapped = float.trunc() % TWO_POW_64;
    if wrapped < 0.0 {
        wrapped += TWO_POW_64;
    }
    (wrapped as u64) as i64
}

/// C-locale `isspace()`.
const fn is_c_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r')
}

fn without_line_breaks(mut bytes: &[u8]) -> &[u8] {
    while let [rest @ .., b'\r' | b'\n'] = bytes {
        bytes = rest;
    }
    bytes
}

/// `str_getcsv($string, ',', '"', '')` for a non-empty string.
pub(crate) fn str_getcsv(input: &[u8]) -> PhpArray {
    let record = input
        .strip_suffix(b"\r\n")
        .or_else(|| input.strip_suffix(b"\n"))
        .or_else(|| input.strip_suffix(b"\r"))
        .unwrap_or(input);
    if record.is_empty() {
        return PhpArray::list([Value::Null]);
    }
    let mut fields = Vec::new();
    let mut at = 0;
    loop {
        let start = at;
        let blank = input[at..].iter().take_while(|&&b| is_c_space(b)).count();
        let mut field = Vec::new();
        if input.get(at + blank) == Some(&b'"') {
            at += blank + 1;
            let mut closed = false;
            while at < input.len() {
                if input[at] == b'"' {
                    if input.get(at + 1) == Some(&b'"') {
                        field.push(b'"');
                        at += 2;
                        continue;
                    }
                    at += 1;
                    closed = true;
                    break;
                }
                field.push(input[at]);
                at += 1;
            }
            if closed {
                let tail_end = input[at..]
                    .iter()
                    .position(|&b| b == b',')
                    .map_or(input.len(), |p| at + p);
                field.extend_from_slice(without_line_breaks(&input[at..tail_end]));
                at = tail_end;
            }
        } else {
            let end = input[start..]
                .iter()
                .position(|&b| b == b',')
                .map_or(input.len(), |p| start + p);
            field.extend_from_slice(without_line_breaks(&input[start..end]));
            at = end;
        }
        fields.push(Value::String(field.into()));
        if at >= input.len() {
            break;
        }
        // The delimiter.
        at += 1;
        if at == input.len() {
            fields.push(Value::from(""));
            break;
        }
    }
    PhpArray::list(fields)
}

/// `shuffle($array)`: the values in a random order, as a list. PHP draws
/// from its Mt19937 generator, which `srand()` seeds; here the order comes
/// from the standard library's random hasher keys, so a seeded order is
/// not reproduced.
pub(crate) fn shuffle(array: &PhpArray) -> PhpArray {
    let mut values: Vec<Value> = array.values().cloned().collect();
    let state = RandomState::new();
    for i in (1..values.len()).rev() {
        let mut hasher = state.build_hasher();
        hasher.write_usize(i);
        let bound = u64::try_from(i + 1).unwrap_or(u64::MAX);
        let j = usize::try_from(hasher.finish() % bound).unwrap_or(0);
        values.swap(i, j);
    }
    PhpArray::list(values)
}

//! PHP's string/number conversions, written from PHP 8.4's recorded
//! behaviour (oracle/recorders/php_value.php -> tests/data/php.json):
//!
//! - A numeric string is optional whitespace (` \t\n\r\v\f`), an optional
//!   sign, digits with an optional fraction (or a fraction alone), an
//!   optional exponent, and optional whitespace. `is_numeric()` wants the
//!   whole string to be one; `(int)` and `(float)` read its longest
//!   numeric prefix and ignore the rest (`"12abc"` is 12).
//! - `(int)` of an integer prefix saturates at the int limits; of a float
//!   prefix, it truncates and saturates, except that an infinite value
//!   gives 0.
//! - `(string)` of a float keeps 14 significant digits (`precision`);
//!   `var_export()` and `json_encode()` keep the shortest digits that read
//!   back (`serialize_precision = -1`). All three switch to exponent form
//!   below 1e-4 and from 10^precision (14, or 17 for the shortest form).
//! - `hexdec()` / `octdec()` skip characters that are not digits of their
//!   base and continue in float once the int overflows.
//!
//! Upstream: none

/// An `int|float` result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(i) => i as f64,
            Self::Float(f) => f,
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// The longest numeric prefix after leading whitespace: `(start, end,
/// is_integer)` byte offsets of the number itself.
fn numeric_prefix(s: &[u8]) -> Option<(usize, usize, bool)> {
    let start = s.iter().take_while(|&&b| is_space(b)).count();
    let mut i = start;
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_digits = s[i..].iter().take_while(|b| b.is_ascii_digit()).count();
    i += int_digits;
    let mut integer = true;
    let mut frac_digits = 0;
    if s.get(i) == Some(&b'.') {
        frac_digits = s[i + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if int_digits > 0 || frac_digits > 0 {
            i += 1 + frac_digits;
            integer = false;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(s.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_digits = s[j..].iter().take_while(|b| b.is_ascii_digit()).count();
        if exp_digits > 0 {
            i = j + exp_digits;
            integer = false;
        }
    }
    Some((start, i, integer))
}

/// `is_numeric($string)`.
pub fn is_numeric(s: &[u8]) -> bool {
    match numeric_prefix(s) {
        Some((_, end, _)) => s[end..].iter().all(|&b| is_space(b)),
        None => false,
    }
}

/// The number a numeric string (`is_numeric()`) holds: an int when it is
/// written as one and fits, else a float; `None` for any other string.
pub fn numeric_value(s: &[u8]) -> Option<Number> {
    let (start, end, integer) = numeric_prefix(s)?;
    if !s[end..].iter().all(|&b| is_space(b)) {
        return None;
    }
    let number = &s[start..end];
    if integer && let Some(int) = checked_int(number) {
        return Some(Number::Int(int));
    }
    Some(Number::Float(parse_float(number)))
}

/// An optionally signed run of digits as an int, `None` when it overflows.
fn checked_int(number: &[u8]) -> Option<i64> {
    let (negative, digits) = match number.first() {
        Some(b'-') => (true, &number[1..]),
        Some(b'+') => (false, &number[1..]),
        _ => (false, number),
    };
    digits.iter().try_fold(0i64, |value, &d| {
        let d = i64::from(d - b'0');
        let value = value.checked_mul(10)?;
        if negative {
            value.checked_sub(d)
        } else {
            value.checked_add(d)
        }
    })
}

fn parse_float(digits: &[u8]) -> f64 {
    // The prefix is ASCII digits, sign, point and exponent, which Rust
    // parses as the same decimal number.
    std::str::from_utf8(digits)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// `(float) $string`.
pub fn string_to_float(s: &[u8]) -> f64 {
    match numeric_prefix(s) {
        Some((start, end, _)) => parse_float(&s[start..end]),
        None => 0.0,
    }
}

/// `(int) $string`.
pub fn string_to_int(s: &[u8]) -> i64 {
    let Some((start, end, integer)) = numeric_prefix(s) else {
        return 0;
    };
    let number = &s[start..end];
    if integer {
        let (negative, digits) = match number.first() {
            Some(b'-') => (true, &number[1..]),
            Some(b'+') => (false, &number[1..]),
            _ => (false, number),
        };
        let mut value: i64 = 0;
        for &d in digits {
            let d = i64::from(d - b'0');
            let next = value.checked_mul(10).and_then(|v| {
                if negative {
                    v.checked_sub(d)
                } else {
                    v.checked_add(d)
                }
            });
            match next {
                Some(v) => value = v,
                None => return if negative { i64::MIN } else { i64::MAX },
            }
        }
        return value;
    }
    let float = parse_float(number);
    if float.is_infinite() || float.is_nan() {
        return 0;
    }
    // `as` truncates and saturates.
    float as i64
}

/// The decimal digits (no leading or trailing zeros) and the exponent of
/// the first digit, for a finite non-zero `value`; `significant` digits,
/// or the shortest that read back when `None`.
fn decimal_digits(value: f64, significant: Option<usize>) -> (String, i32) {
    let text = match significant {
        Some(n) => format!("{:.*e}", n.saturating_sub(1), value.abs()),
        None => format!("{:e}", value.abs()),
    };
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    (digits, exponent)
}

/// The layout shared by PHP's float formatting: plain unless the exponent
/// is below -4 or at least `precision`, else `d.dddE+X` with at least one
/// fraction digit.
fn layout(value: f64, digits: &str, exponent: i32, precision: i32, e: char) -> String {
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    if exponent < -4 || exponent >= precision {
        let (first, rest) = digits.split_at(1);
        out.push_str(first);
        out.push('.');
        out.push_str(if rest.is_empty() { "0" } else { rest });
        out.push(e);
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
    } else if exponent < 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
        out.push_str(digits);
    } else {
        let int_len = exponent as usize + 1;
        if digits.len() <= int_len {
            out.push_str(digits);
            out.extend(std::iter::repeat_n('0', int_len - digits.len()));
        } else {
            out.push_str(&digits[..int_len]);
            out.push('.');
            out.push_str(&digits[int_len..]);
        }
    }
    out
}

fn special(value: f64) -> Option<&'static str> {
    if value.is_nan() {
        Some("NAN")
    } else if value.is_infinite() {
        Some(if value > 0.0 { "INF" } else { "-INF" })
    } else {
        None
    }
}

/// `(string) $float`, with the default `precision = 14`.
pub fn float_to_string(value: f64) -> String {
    if let Some(s) = special(value) {
        return s.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let (digits, exponent) = decimal_digits(value, Some(14));
    layout(value, &digits, exponent, 14, 'E')
}

/// `var_export($float, true)`, with `serialize_precision = -1`.
pub fn var_export_float(value: f64) -> String {
    if let Some(s) = special(value) {
        return s.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_owned();
    }
    let (digits, exponent) = decimal_digits(value, None);
    let mut out = layout(value, &digits, exponent, 17, 'E');
    if !out.contains(['.', 'E']) {
        out.push_str(".0");
    }
    out
}

/// `json_encode($float)` for a finite float (INF and NaN fail in PHP).
pub fn json_float(value: f64) -> Option<String> {
    if special(value).is_some() {
        return None;
    }
    if value == 0.0 {
        return Some(if value.is_sign_negative() { "-0" } else { "0" }.to_owned());
    }
    let (digits, exponent) = decimal_digits(value, None);
    Some(layout(value, &digits, exponent, 17, 'e'))
}

fn radix(s: &[u8], base: u32) -> Number {
    let mut value = Number::Int(0);
    for &b in s {
        let Some(d) = char::from(b).to_digit(base) else {
            continue;
        };
        value = match value {
            Number::Int(v) => match v
                .checked_mul(i64::from(base))
                .and_then(|v| v.checked_add(i64::from(d)))
            {
                Some(next) => Number::Int(next),
                // Exact until here; continue in float from the exact value.
                None => Number::Float(v as f64 * f64::from(base) + f64::from(d)),
            },
            Number::Float(f) => Number::Float(f * f64::from(base) + f64::from(d)),
        };
    }
    value
}

/// A float in `serialize()`: the shortest digits, exponent form from
/// 10^17 and below 1e-4 with `E`, no `.0` added; `INF`, `-INF`, `NAN`.
pub fn serialize_float(value: f64) -> String {
    if let Some(s) = special(value) {
        return s.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let (digits, exponent) = decimal_digits(value, None);
    layout(value, &digits, exponent, 17, 'E')
}

/// The int key of a float array key: truncated toward zero; NAN and
/// infinities give 0.
pub fn float_key(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

/// `hexdec($string)`.
pub fn hexdec(s: &[u8]) -> Number {
    radix(s, 16)
}

/// `octdec($string)`.
pub fn octdec(s: &[u8]) -> Number {
    radix(s, 8)
}

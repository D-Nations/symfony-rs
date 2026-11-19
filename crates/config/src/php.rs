//! The PHP built-ins the Definition code relies on, written from PHP's
//! documentation and checked against values recorded in the oracle
//! (tests/data/php.json, `scripts/record.sh config`); no PHP source code
//! (PLAN.md decision 8):
//!
//! - truthiness (`if ($value)`, `!$value`);
//! - `levenshtein()` with unit costs, on bytes;
//! - the loose comparison `<` and `sort()`'s default order (PHP 8: a
//!   number and a numeric string compare as numbers, a number and a
//!   non-numeric string as strings, `null` and `bool` against anything as
//!   bools except `null` against a string, which compares `""`);
//! - `strtr($string, $pairs)`;
//! - `json_encode()` of scalars and of the empty values VariableNode
//!   reports, and `var_export()` of enum cases.
//!
//! Upstream: none

use std::cmp::Ordering;

use php_value::{
    Key, PhpArray, PhpString, Value,
    json::{JsonFlags, encode_string},
    numeric::{float_to_string, is_numeric, json_float, string_to_float},
    object::EnumCase,
};

/// `(bool) $value`.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !(s.is_empty() || s.as_bytes() == b"0"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// A key as the string PHP gives it in a string context.
pub fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}

/// `sprintf('%s', $value)` / `(string)` for scalars and null; arrays give
/// "Array" (with a warning in PHP) and objects their class name, which
/// only error paths reach.
pub fn to_bytes(value: &Value) -> Vec<u8> {
    match value {
        Value::Array(_) => b"Array".to_vec(),
        Value::Object(o) => o.class().as_bytes().to_vec(),
        other => other
            .to_php_string()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default(),
    }
}

/// A value's type as PHP's TypeError messages name it ("given",
/// "returned"): `true` and `false` for bools, the class for objects.
pub fn error_type_name(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        other => other.debug_type(),
    }
}

/// `levenshtein($a, $b)` with insertion, replacement and deletion costs 1.
pub fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let replace = previous[j] + usize::from(ca != cb);
            let insert = current[j] + 1;
            let delete = previous[j + 1] + 1;
            current[j + 1] = replace.min(insert).min(delete);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

enum Num {
    Int(i64),
    Float(f64),
}

fn compare_numbers(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Some(x.cmp(&y)),
        (Num::Int(x), Num::Float(y)) => (x as f64).partial_cmp(&y),
        (Num::Float(x), Num::Int(y)) => x.partial_cmp(&(y as f64)),
        (Num::Float(x), Num::Float(y)) => x.partial_cmp(&y),
    }
}

/// A numeric string as the number it is: an int when it is written as
/// one and fits, else a float.
fn numeric_string(s: &[u8]) -> Num {
    let trimmed = s.trim_ascii();
    let digits = trimmed.strip_prefix(b"+").unwrap_or(trimmed);
    let unsigned = digits.strip_prefix(b"-").unwrap_or(digits);
    if !unsigned.is_empty()
        && unsigned.iter().all(u8::is_ascii_digit)
        && let Some(i) = std::str::from_utf8(digits)
            .ok()
            .and_then(|t| t.parse().ok())
    {
        return Num::Int(i);
    }
    Num::Float(string_to_float(s))
}

/// `$a <=> $b` for scalars and null, or `None` when PHP's answer is
/// "uncomparable" (NAN, arrays, objects): then `<` and `>` are false.
pub fn compare(a: &Value, b: &Value) -> Option<Ordering> {
    use Value::*;
    match (a, b) {
        (Null, String(s)) => Some(compare_strings(b"", s)),
        (String(s), Null) => Some(compare_strings(s, b"")),
        (Null | Bool(_), _) | (_, Null | Bool(_)) => Some(truthy(a).cmp(&truthy(b))),
        (Int(x), Int(y)) => Some(x.cmp(y)),
        (Int(x), Float(y)) => compare_numbers(Num::Int(*x), Num::Float(*y)),
        (Float(x), Int(y)) => compare_numbers(Num::Float(*x), Num::Int(*y)),
        (Float(x), Float(y)) => x.partial_cmp(y),
        (Int(_) | Float(_), String(s)) => {
            if is_numeric(s) {
                compare_numbers(number(a), numeric_string(s))
            } else {
                Some(compare_strings(&to_bytes(a), s))
            }
        }
        (String(s), Int(_) | Float(_)) => {
            if is_numeric(s) {
                compare_numbers(numeric_string(s), number(b))
            } else {
                Some(compare_strings(s, &to_bytes(b)))
            }
        }
        (String(x), String(y)) => Some(compare_strings(x, y)),
        _ => None,
    }
}

fn number(value: &Value) -> Num {
    match value {
        Value::Int(i) => Num::Int(*i),
        Value::Float(f) => Num::Float(*f),
        _ => Num::Int(0),
    }
}

/// Two strings: as numbers when both are numeric, else byte by byte.
fn compare_strings(a: &[u8], b: &[u8]) -> Ordering {
    if is_numeric(a)
        && is_numeric(b)
        && let Some(o) = compare_numbers(numeric_string(a), numeric_string(b))
    {
        return o;
    }
    a.cmp(b)
}

/// `$a < $b`.
pub fn less(a: &Value, b: &Value) -> bool {
    compare(a, b) == Some(Ordering::Less)
}

/// `$a > $b`.
pub fn greater(a: &Value, b: &Value) -> bool {
    compare(a, b) == Some(Ordering::Greater)
}

/// `sort($keys)` with the default flags (stable, as PHP 8's sort is).
pub fn sort_keys(keys: &mut [Key]) {
    keys.sort_by(|a, b| compare(&a.to_value(), &b.to_value()).unwrap_or(Ordering::Equal));
}

/// `strtr($subject, $pairs)`: at each position the longest matching key
/// is replaced, and replaced text is not searched again.
pub fn strtr(subject: &[u8], pairs: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        let found = pairs
            .iter()
            .filter(|(from, _)| !from.is_empty() && subject[i..].starts_with(from))
            .max_by_key(|(from, _)| from.len());
        match found {
            Some((from, to)) => {
                out.extend_from_slice(to);
                i += from.len();
            }
            None => {
                out.push(subject[i]);
                i += 1;
            }
        }
    }
    out
}

/// `json_encode($value, $flags)` of a scalar or null; `None` where PHP
/// returns `false` (invalid UTF-8, INF, NAN, or a value of another kind).
/// `preserve_zero_fraction` is `JSON_PRESERVE_ZERO_FRACTION`.
pub fn json_encode_scalar(
    value: &Value,
    flags: JsonFlags,
    preserve_zero_fraction: bool,
) -> Option<Vec<u8>> {
    Some(match value {
        Value::Null => b"null".to_vec(),
        Value::Bool(true) => b"true".to_vec(),
        Value::Bool(false) => b"false".to_vec(),
        Value::Int(i) => i.to_string().into_bytes(),
        Value::Float(f) => {
            let mut s = json_float(*f)?;
            if preserve_zero_fraction && !s.contains(['.', 'e', 'E']) {
                s.push_str(".0");
            }
            s.into_bytes()
        }
        Value::String(s) => encode_string(s, flags)?.into_bytes(),
        Value::Array(a) if a.is_empty() => b"[]".to_vec(),
        _ => return None,
    })
}

/// `var_export($case, true)` of an enum case: `\Class::Case`.
pub fn var_export_enum(case: &EnumCase, class: &str) -> String {
    format!("\\{}::{}", class, case.name())
}

/// `var_export($float, true)`.
pub fn var_export_float(value: f64) -> String {
    php_value::numeric::var_export_float(value)
}

/// `(string) $float`.
pub fn float_string(value: f64) -> String {
    float_to_string(value)
}

/// An array with the given string keys, in order.
pub fn keys_of(array: &PhpArray) -> Vec<Key> {
    array.keys().cloned().collect()
}

/// Builds a message from literal text and bytes.
#[derive(Default)]
pub struct Msg(Vec<u8>);

impl Msg {
    pub fn new(text: &str) -> Self {
        Self(text.as_bytes().to_vec())
    }

    /// Appends literal text.
    pub fn s(mut self, text: &str) -> Self {
        self.0.extend_from_slice(text.as_bytes());
        self
    }

    /// Appends bytes.
    pub fn b(mut self, bytes: &[u8]) -> Self {
        self.0.extend_from_slice(bytes);
        self
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl From<Msg> for Vec<u8> {
    fn from(m: Msg) -> Self {
        m.0
    }
}

/// `implode($glue, $pieces)`.
pub fn implode(glue: &[u8], pieces: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, piece) in pieces.into_iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(glue);
        }
        out.extend_from_slice(&piece);
    }
    out
}

/// `str_contains($haystack, $needle)`: true for an empty needle.
pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// A PhpString from bytes.
pub fn string(bytes: impl Into<Vec<u8>>) -> PhpString {
    PhpString::from(bytes.into())
}

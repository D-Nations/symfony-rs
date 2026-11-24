//! PHP 8's loose equality (`==`), written from the PHP manual's
//! "Comparison with Various Types" table and checked against PHP 8.4.27's
//! answers (oracle/recorders/php_value_loose_equals.php ->
//! tests/data/loose_equals.json):
//!
//! - a bool or null against anything else compares truthiness, except
//!   that null against a string is `''` against that string;
//! - two strings compare as numbers when both are numeric strings, else
//!   as bytes; two integer strings of which one overflows an int compare
//!   as bytes;
//! - an int or float against a string compares as numbers when the string
//!   is numeric, else the number's string form against the string (NAN
//!   equals no string);
//! - ints and floats compare as numbers (NAN equals nothing);
//! - arrays are equal when they hold the same keys with loosely equal
//!   values, in any order; an array equals no scalar;
//! - an object equals itself, or an object of the same class whose
//!   properties are loosely equal (for two `\Stringable` objects, whose
//!   non-public state is not visible here, their string forms too); a
//!   `\Stringable` object against a string compares its string form as a
//!   string; an object against an int or float counts as
//!   1 (PHP warns).
//!
//! Upstream: none

use std::sync::Arc;

use crate::{
    array::PhpArray,
    numeric::{Number, float_to_string, numeric_value},
    value::Value,
};

/// `(bool) $value`.
fn truthy(value: &Value) -> bool {
    value.is_truthy()
}

fn numbers_equal(a: Number, b: Number) -> bool {
    match (a, b) {
        (Number::Int(a), Number::Int(b)) => a == b,
        (a, b) => a.as_f64() == b.as_f64(),
    }
}

/// A numeric string against another, as numbers; but two integer strings
/// (no point or exponent) of which one does not fit an int compare as
/// bytes, so that distinct big integers are not equal through float
/// rounding.
fn numeric_strings_equal(a: &[u8], b: &[u8]) -> Option<bool> {
    let (x, y) = (numeric_value(a)?, numeric_value(b)?);
    let integer = |s: &[u8]| s.iter().all(|c| !matches!(c, b'.' | b'e' | b'E'));
    if integer(a) && integer(b) && (matches!(x, Number::Float(_)) || matches!(y, Number::Float(_)))
    {
        return Some(a == b);
    }
    Some(numbers_equal(x, y))
}

fn strings_equal(a: &[u8], b: &[u8]) -> bool {
    numeric_strings_equal(a, b).unwrap_or(a == b)
}

fn number_against_string(number: Number, s: &[u8]) -> bool {
    match numeric_value(s) {
        Some(value) => numbers_equal(number, value),
        // NAN equals no string, not even "NAN".
        None if matches!(number, Number::Float(f) if f.is_nan()) => false,
        None => {
            let text = match number {
                Number::Int(i) => i.to_string(),
                Number::Float(f) => float_to_string(f),
            };
            text.as_bytes() == s
        }
    }
}

fn arrays_equal(a: &PhpArray, b: &PhpArray) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(key, value)| b.get(key).is_some_and(|other| loose_equals(value, other)))
}

/// `$a == $b`.
pub fn loose_equals(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Null, String(s)) | (String(s), Null) => s.is_empty(),
        (Bool(_) | Null, _) | (_, Bool(_) | Null) => truthy(a) == truthy(b),
        (Int(x), Int(y)) => x == y,
        (Int(_) | Float(_), Int(_) | Float(_)) => numbers_equal(number(a), number(b)),
        (String(x), String(y)) => strings_equal(x.as_bytes(), y.as_bytes()),
        (Int(_) | Float(_), String(s)) | (String(s), Int(_) | Float(_)) => {
            let n = if let String(_) = a {
                number(b)
            } else {
                number(a)
            };
            number_against_string(n, s.as_bytes())
        }
        (Array(x), Array(y)) => arrays_equal(x, y),
        (Array(_), _) | (_, Array(_)) => false,
        (Object(x), Object(y)) => {
            std::ptr::addr_eq(Arc::as_ptr(x), Arc::as_ptr(y))
                || (x.class() == y.class()
                    && arrays_equal(&x.properties(), &y.properties())
                    && hidden_state_equal(x, y))
        }
        (Object(o), String(s)) | (String(s), Object(o)) => o
            .to_php_string()
            .is_some_and(|text| strings_equal(text.as_bytes(), s.as_bytes())),
        (Object(_), _) => numbers_equal(Number::Int(1), number(b)),
        (_, Object(_)) => numbers_equal(number(a), Number::Int(1)),
    }
}

/// PHP compares every property of two objects of one class, the
/// non-public ones too, which [`crate::PhpObject`] does not expose. Two
/// `\Stringable` objects stand in for that state with their string forms.
fn hidden_state_equal(x: &crate::Object, y: &crate::Object) -> bool {
    match (x.to_php_string(), y.to_php_string()) {
        (Some(a), Some(b)) => strings_equal(a.as_bytes(), b.as_bytes()),
        _ => true,
    }
}

/// An int or float value as a number.
fn number(value: &Value) -> Number {
    match value {
        Value::Float(f) => Number::Float(*f),
        Value::Int(i) => Number::Int(*i),
        _ => Number::Int(0),
    }
}

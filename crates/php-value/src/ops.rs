//! PHP 8's operators on values: the arithmetic and bitwise operators,
//! concatenation, `<=>`, and the `(int)`, `(float)` and `(string)` casts,
//! written from the PHP manual ("Arithmetic Operators", "Comparison
//! Operators", "Type Juggling") and PHP 8.4.27's answers (the cases in
//! `tests/ops.rs` were run in the oracle image):
//!
//! - an arithmetic operand is converted to a number: null is 0, a bool 0
//!   or 1, a numeric string its value, a leading-numeric string its prefix
//!   (PHP warns); any other string, an array (except `array + array`) and
//!   an object throw `TypeError: Unsupported operand types: <left> <op>
//!   <right>`;
//! - an int result that overflows becomes a float; `/` gives an int only
//!   when the division is exact; `%` works on ints (a float operand is
//!   truncated); a zero divisor throws `DivisionByZeroError`;
//! - unary minus and plus are `$a * -1` and `$a * 1`;
//! - `<=>`: null or a bool against anything compares truthiness (null
//!   against a string is `""` against it); numbers and numeric strings
//!   compare as numbers, a number against a non-numeric string compares
//!   its string form bytewise; arrays compare by size, then key by key (a
//!   key missing on the right makes them uncomparable, which reads as
//!   greater); an array is greater than anything else;
//! - `(int)` of a float outside the int range wraps modulo 2^64; NAN and
//!   the infinities give 0.
//!
//! PHP's warnings are not reported.
//!
//! Upstream: none

#![allow(clippy::many_single_char_names, reason = "operands are a and b, as in the PHP manual")]

use std::cmp::Ordering;

use crate::{
    array::PhpArray,
    numeric::{Number, float_to_string, leading_numeric_value, numeric_value, string_to_int},
    string::PhpString,
    value::{Thrown, Value},
};

/// The type name PHP's operator errors use.
fn operand_type(value: &Value) -> String {
    match value {
        Value::Bool(_) => "bool".to_owned(),
        other => other.debug_type(),
    }
}

fn unsupported(a: &Value, op: &str, b: &Value) -> Thrown {
    Thrown::new(
        "TypeError",
        format!(
            "Unsupported operand types: {} {op} {}",
            operand_type(a),
            operand_type(b)
        ),
        0,
    )
}

/// An arithmetic operand as a number; `None` where PHP throws.
fn operand(value: &Value) -> Option<Number> {
    Some(match value {
        Value::Null | Value::Bool(false) => Number::Int(0),
        Value::Bool(true) => Number::Int(1),
        Value::Int(i) => Number::Int(*i),
        Value::Float(f) => Number::Float(*f),
        Value::String(s) => leading_numeric_value(s.as_bytes())?,
        Value::Array(_) | Value::Object(_) => return None,
    })
}

fn operands(a: &Value, op: &str, b: &Value) -> Result<(Number, Number), Thrown> {
    match (operand(a), operand(b)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(unsupported(a, op, b)),
    }
}

fn arithmetic(
    a: &Value,
    op: &str,
    b: &Value,
    int: fn(i64, i64) -> Option<i64>,
    float: fn(f64, f64) -> f64,
) -> Result<Value, Thrown> {
    let (x, y) = operands(a, op, b)?;
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) => match int(x, y) {
            Some(r) => Value::Int(r),
            None => Value::Float(float(Number::Int(x).as_f64(), Number::Int(y).as_f64())),
        },
        (x, y) => Value::Float(float(x.as_f64(), y.as_f64())),
    })
}

/// `$a + $b` (`array + array` is the union).
pub fn add(a: &Value, b: &Value) -> Result<Value, Thrown> {
    if let (Value::Array(x), Value::Array(y)) = (a, b) {
        let mut union = x.clone();
        union.union(y);
        return Ok(Value::Array(union));
    }
    arithmetic(a, "+", b, i64::checked_add, |x, y| x + y)
}

/// `$a - $b`.
pub fn sub(a: &Value, b: &Value) -> Result<Value, Thrown> {
    arithmetic(a, "-", b, i64::checked_sub, |x, y| x - y)
}

/// `$a * $b`.
pub fn mul(a: &Value, b: &Value) -> Result<Value, Thrown> {
    arithmetic(a, "*", b, i64::checked_mul, |x, y| x * y)
}

/// `$a / $b`.
pub fn div(a: &Value, b: &Value) -> Result<Value, Thrown> {
    let (x, y) = operands(a, "/", b)?;
    if y.as_f64() == 0.0 {
        return Err(Thrown::new("DivisionByZeroError", "Division by zero", 0));
    }
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) if x.checked_rem(y) == Some(0) => {
            x.checked_div(y).map_or_else(
                || Value::Float(Number::Int(x).as_f64() / Number::Int(y).as_f64()),
                Value::Int,
            )
        }
        (x, y) => Value::Float(x.as_f64() / y.as_f64()),
    })
}

/// `$a % $b`.
pub fn modulo(a: &Value, b: &Value) -> Result<Value, Thrown> {
    let (x, y) = operands(a, "%", b)?;
    let (x, y) = (number_to_int(x), number_to_int(y));
    if y == 0 {
        return Err(Thrown::new("DivisionByZeroError", "Modulo by zero", 0));
    }
    // PHP_INT_MIN % -1 is 0.
    Ok(Value::Int(x.checked_rem(y).unwrap_or(0)))
}

/// `$a ** $b`.
pub fn pow(a: &Value, b: &Value) -> Result<Value, Thrown> {
    let (x, y) = operands(a, "**", b)?;
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) if y >= 0 => {
            let result = u32::try_from(y).ok().and_then(|e| x.checked_pow(e));
            result.map_or_else(
                || Value::Float(powf(Number::Int(x).as_f64(), Number::Int(y).as_f64())),
                Value::Int,
            )
        }
        (x, y) => Value::Float(powf(x.as_f64(), y.as_f64())),
    })
}

fn powf(x: f64, y: f64) -> f64 {
    x.powf(y)
}

/// `-$a` (PHP computes `$a * -1`).
pub fn neg(a: &Value) -> Result<Value, Thrown> {
    mul(a, &Value::Int(-1))
}

/// `+$a` (PHP computes `$a * 1`).
pub fn pos(a: &Value) -> Result<Value, Thrown> {
    mul(a, &Value::Int(1))
}

fn bitwise(
    a: &Value,
    op: &str,
    b: &Value,
    int: fn(i64, i64) -> i64,
    byte: fn(u8, u8) -> u8,
    longest: bool,
) -> Result<Value, Thrown> {
    if let (Value::String(x), Value::String(y)) = (a, b) {
        let (x, y) = (x.as_bytes(), y.as_bytes());
        let len = if longest {
            x.len().max(y.len())
        } else {
            x.len().min(y.len())
        };
        let bytes: Vec<u8> = (0..len)
            .map(|i| match (x.get(i), y.get(i)) {
                (Some(&p), Some(&q)) => byte(p, q),
                (Some(&p), None) | (None, Some(&p)) => p,
                (None, None) => 0,
            })
            .collect();
        return Ok(Value::String(PhpString::from(bytes)));
    }
    let (x, y) = operands(a, op, b)?;
    Ok(Value::Int(int(number_to_int(x), number_to_int(y))))
}

/// `$a & $b` (two strings: bytewise, the shorter length).
pub fn bit_and(a: &Value, b: &Value) -> Result<Value, Thrown> {
    bitwise(a, "&", b, |x, y| x & y, |x, y| x & y, false)
}

/// `$a | $b` (two strings: bytewise, the longer length).
pub fn bit_or(a: &Value, b: &Value) -> Result<Value, Thrown> {
    bitwise(a, "|", b, |x, y| x | y, |x, y| x | y, true)
}

/// `$a ^ $b` (two strings: bytewise, the shorter length).
pub fn bit_xor(a: &Value, b: &Value) -> Result<Value, Thrown> {
    bitwise(a, "^", b, |x, y| x ^ y, |x, y| x ^ y, false)
}

/// `(int)` of a float: truncated, wrapped modulo 2^64 outside the int
/// range; 0 for NAN and the infinities.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "PHP's float to int conversion wraps modulo 2^64"
)]
pub fn float_to_int(f: f64) -> i64 {
    const TWO_64: f64 = 18_446_744_073_709_551_616.0;
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    if !f.is_finite() {
        return 0;
    }
    let t = f.trunc();
    if (-TWO_63..TWO_63).contains(&t) {
        return t as i64;
    }
    let mut m = t % TWO_64;
    if m < 0.0 {
        m += TWO_64;
    }
    // m is in [0, 2^64): reinterpret as two's complement.
    (m as u64).cast_signed()
}

fn number_to_int(n: Number) -> i64 {
    match n {
        Number::Int(i) => i,
        Number::Float(f) => float_to_int(f),
    }
}

/// `(int) $value`.
pub fn to_int(value: &Value) -> i64 {
    match value {
        Value::Null | Value::Bool(false) => 0,
        Value::Bool(true) | Value::Object(_) => 1,
        Value::Int(i) => *i,
        Value::Float(f) => float_to_int(*f),
        Value::String(s) => string_to_int(s.as_bytes()),
        Value::Array(a) => i64::from(!a.is_empty()),
    }
}

/// `(float) $value`.
pub fn to_float(value: &Value) -> f64 {
    match value {
        Value::Float(f) => *f,
        Value::String(s) => crate::numeric::string_to_float(s.as_bytes()),
        other => Number::Int(to_int(other)).as_f64(),
    }
}

/// `(string) $value`: an array is `"Array"` (PHP warns), an object needs
/// `__toString()`.
pub fn to_string(value: &Value) -> Result<PhpString, Thrown> {
    match value {
        Value::Array(_) => Ok(PhpString::from("Array")),
        Value::Object(o) => o.to_php_string().ok_or_else(|| {
            Thrown::new(
                "Error",
                format!(
                    "Object of class {} could not be converted to string",
                    o.class()
                ),
                0,
            )
        }),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// `$a . $b`.
pub fn concat(a: &Value, b: &Value) -> Result<Value, Thrown> {
    let mut bytes = to_string(a)?.as_bytes().to_vec();
    bytes.extend_from_slice(to_string(b)?.as_bytes());
    Ok(Value::String(PhpString::from(bytes)))
}

fn number_cmp(a: Number, b: Number) -> Ordering {
    match (a, b) {
        (Number::Int(x), Number::Int(y)) => x.cmp(&y),
        (x, y) => {
            let (x, y) = (x.as_f64(), y.as_f64());
            // NAN is uncomparable: PHP answers 1.
            x.partial_cmp(&y).unwrap_or(Ordering::Greater)
        }
    }
}

fn number_text(n: Number) -> String {
    match n {
        Number::Int(i) => i.to_string(),
        Number::Float(f) => float_to_string(f),
    }
}

/// A number against a string under PHP 8's rules.
fn number_string_cmp(n: Number, s: &[u8]) -> Ordering {
    match numeric_value(s) {
        Some(m) => number_cmp(n, m),
        None => number_text(n).as_bytes().cmp(s),
    }
}

fn strings_cmp(a: &[u8], b: &[u8]) -> Ordering {
    match (numeric_value(a), numeric_value(b)) {
        (Some(x), Some(y)) => number_cmp(x, y),
        _ => a.cmp(b),
    }
}

fn arrays_cmp(a: &PhpArray, b: &PhpArray) -> Ordering {
    match a.len().cmp(&b.len()) {
        Ordering::Equal => {}
        other => return other,
    }
    for (key, value) in a.iter() {
        let Some(other) = b.get(key) else {
            return Ordering::Greater;
        };
        match compare(value, other) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// `$a <=> $b`. An uncomparable pair reads as greater (PHP returns 1).
pub fn compare(a: &Value, b: &Value) -> Ordering {
    use Value::{Array, Bool, Float, Int, Null, Object, String};
    match (a, b) {
        (Null, Null) => Ordering::Equal,
        (Null, String(s)) => b"".as_slice().cmp(s.as_bytes()),
        (String(s), Null) => s.as_bytes().cmp(b""),
        (Null | Bool(_), _) | (_, Null | Bool(_)) => a.is_truthy().cmp(&b.is_truthy()),
        (Int(x), Int(y)) => x.cmp(y),
        (Int(_) | Float(_), Int(_) | Float(_)) => number_cmp(
            operand(a).unwrap_or(Number::Int(0)),
            operand(b).unwrap_or(Number::Int(0)),
        ),
        (Int(_) | Float(_), String(s)) => {
            number_string_cmp(operand(a).unwrap_or(Number::Int(0)), s.as_bytes())
        }
        (String(s), Int(_) | Float(_)) => {
            number_string_cmp(operand(b).unwrap_or(Number::Int(0)), s.as_bytes()).reverse()
        }
        (String(x), String(y)) => strings_cmp(x.as_bytes(), y.as_bytes()),
        (Array(x), Array(y)) => arrays_cmp(x, y),
        (Array(_), _) => Ordering::Greater,
        (_, Array(_)) => Ordering::Less,
        (Object(x), Object(y)) => {
            if std::ptr::addr_eq(std::sync::Arc::as_ptr(x), std::sync::Arc::as_ptr(y)) {
                Ordering::Equal
            } else if x.class() == y.class() {
                arrays_cmp(&x.properties(), &y.properties())
            } else {
                Ordering::Greater
            }
        }
        (Object(o), String(s)) => match o.to_php_string() {
            Some(text) => strings_cmp(text.as_bytes(), s.as_bytes()),
            None => Ordering::Greater,
        },
        (String(s), Object(o)) => match o.to_php_string() {
            Some(text) => strings_cmp(s.as_bytes(), text.as_bytes()),
            None => Ordering::Less,
        },
        // An object counts as 1 against a number (PHP warns).
        (Object(_), _) => number_cmp(Number::Int(1), operand(b).unwrap_or(Number::Int(0))),
        (_, Object(_)) => number_cmp(operand(a).unwrap_or(Number::Int(0)), Number::Int(1)),
    }
}

/// `$a <=> $b` as PHP's int.
pub fn spaceship(a: &Value, b: &Value) -> i64 {
    match compare(a, b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// `$a < $b`; an uncomparable pair is not less either way.
pub fn less(a: &Value, b: &Value) -> bool {
    compare(a, b) == Ordering::Less
}

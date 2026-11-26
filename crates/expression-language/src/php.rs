//! The PHP operators and functions that the nodes run when they evaluate
//! an expression, written from the PHP manual and checked against PHP
//! 8.4.27's answers (`oracle/recorders/expression_language.php` ->
//! `tests/data/php.json`). php-value gives the values, `==`, `===` and
//! the string/number conversions; this module adds what php-value does
//! not have yet: arithmetic, bitwise and comparison operators, `range()`,
//! `min()`/`max()`, the string functions the operators call, and the
//! escaping functions (`stripcslashes()`, `addcslashes()`,
//! `var_export()`). No PHP source code is used (PLAN.md decision 8).
//!
//! PHP's warnings and deprecations (a leading-numeric string in
//! arithmetic, a float key that loses its fraction, an undefined array
//! key) are not reported; the value PHP goes on with is returned.
//!
//! Upstream: none

use std::cmp::Ordering;

use php_value::{
    Key, Number, PhpArray, PhpObject, PhpString, Value,
    compare::loose_equals,
    numeric::{
        float_to_string, is_numeric, leading_numeric_value, numeric_value, var_export_float,
    },
    object::EnumCase,
};

use crate::syntax_error::{Class, Error};

/// `stripcslashes($string)`: C escapes `\a \b \f \n \r \t \v`, octal
/// (`\0` to `\777`, one to three digits, the low byte kept) and hex
/// (`\x` and one or two digits); any other escaped byte stands for
/// itself; a final lone backslash is dropped.
pub fn stripcslashes(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] != b'\\' {
            out.push(s[i]);
            i += 1;
            continue;
        }
        i += 1;
        let Some(&c) = s.get(i) else { break };
        match c {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'a' => out.push(0x07),
            b'v' => out.push(0x0b),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0c),
            b'x' if s.get(i + 1).is_some_and(u8::is_ascii_hexdigit) => {
                let mut value = 0u32;
                let mut j = i + 1;
                while j < s.len() && j < i + 3 && s[j].is_ascii_hexdigit() {
                    value = value * 16 + hex_digit(s[j]);
                    j += 1;
                }
                out.push(low_byte(value));
                i = j;
                continue;
            }
            b'0'..=b'7' => {
                let mut value = 0u32;
                let mut j = i;
                while j < s.len() && j < i + 3 && matches!(s[j], b'0'..=b'7') {
                    value = value * 8 + u32::from(s[j] - b'0');
                    j += 1;
                }
                out.push(low_byte(value));
                i = j;
                continue;
            }
            other => out.push(other),
        }
        i += 1;
    }
    out
}

fn hex_digit(b: u8) -> u32 {
    char::from(b).to_digit(16).unwrap_or(0)
}

fn low_byte(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

/// `addcslashes($string, $characters)` for a list of single bytes.
pub fn addcslashes(s: &[u8], characters: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for &b in s {
        if !characters.contains(&b) {
            out.push(b);
            continue;
        }
        out.push(b'\\');
        match b {
            0x07 => out.push(b'a'),
            0x08 => out.push(b'b'),
            b'\t' => out.push(b't'),
            b'\n' => out.push(b'n'),
            0x0b => out.push(b'v'),
            0x0c => out.push(b'f'),
            b'\r' => out.push(b'r'),
            b if !(32..=126).contains(&b) => out.extend_from_slice(format!("{b:03o}").as_bytes()),
            b => out.push(b),
        }
    }
    out
}

/// `var_export($value, true)`.
pub fn var_export(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    export(value, 0, &mut out);
    out
}

fn export_string(s: &[u8], out: &mut Vec<u8>) {
    // NUL bytes leave the quotes: 'a' . "\0" . 'b'
    out.push(b'\'');
    for &b in s {
        match b {
            0 => out.extend_from_slice(b"' . \"\\0\" . '"),
            b'\'' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b => out.push(b),
        }
    }
    out.push(b'\'');
}

fn export(value: &Value, indent: usize, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"NULL"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Int(i) => out.extend_from_slice(crate::compiler::int_export(*i).as_bytes()),
        Value::Float(f) => out.extend_from_slice(var_export_float(*f).as_bytes()),
        Value::String(s) => export_string(s.as_bytes(), out),
        Value::Array(a) => {
            out.extend_from_slice(b"array (\n");
            for (k, v) in a.iter() {
                out.extend(std::iter::repeat_n(b' ', indent + 2));
                match k {
                    Key::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
                    Key::Str(s) => export_string(s.as_bytes(), out),
                }
                out.extend_from_slice(b" => ");
                if matches!(v, Value::Array(_) | Value::Object(_)) {
                    out.push(b'\n');
                    out.extend(std::iter::repeat_n(b' ', indent + 2));
                }
                export(v, indent + 2, out);
                out.extend_from_slice(b",\n");
            }
            out.extend(std::iter::repeat_n(b' ', indent));
            out.push(b')');
        }
        Value::Object(o) => {
            if let Some(case) = o.downcast_ref::<EnumCase>() {
                out.push(b'\\');
                out.extend_from_slice(o.class().as_bytes());
                out.extend_from_slice(b"::");
                out.extend_from_slice(case.name().as_bytes());
            } else {
                out.push(b'\\');
                out.extend_from_slice(o.class().as_bytes());
                out.extend_from_slice(b"::__set_state(");
                export(&Value::Array(o.properties()), indent, out);
                out.push(b')');
            }
        }
    }
}

/// `zend_zval_value_name()`: the type name, with booleans by value, as
/// argument type errors name a value.
pub fn value_name(value: &Value) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        other => other.debug_type(),
    }
}

/// The type name of an operand in "Unsupported operand types" messages.
fn operand_type(value: &Value) -> String {
    value.debug_type()
}

fn unsupported(a: &Value, op: &str, b: &Value) -> Error {
    Error::type_error(format!(
        "Unsupported operand types: {} {op} {}",
        operand_type(a),
        operand_type(b)
    ))
}

/// The number an arithmetic operand stands for, or `None` (PHP's
/// "Unsupported operand types").
fn arithmetic_operand(value: &Value) -> Option<Number> {
    match value {
        Value::Null => Some(Number::Int(0)),
        Value::Bool(b) => Some(Number::Int(i64::from(*b))),
        Value::Int(i) => Some(Number::Int(*i)),
        Value::Float(f) => Some(Number::Float(*f)),
        Value::String(s) => leading_numeric_value(s.as_bytes()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn operands(a: &Value, op: &str, b: &Value) -> Result<(Number, Number), Error> {
    match (arithmetic_operand(a), arithmetic_operand(b)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(unsupported(a, op, b)),
    }
}

/// `$a + $b`.
pub fn add(a: &Value, b: &Value) -> Result<Value, Error> {
    if let (Value::Array(x), Value::Array(y)) = (a, b) {
        let mut sum = x.clone();
        sum.union(y);
        return Ok(Value::Array(sum));
    }
    let (x, y) = operands(a, "+", b)?;
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) => x
            .checked_add(y)
            .map_or_else(|| Value::Float(as_float(x) + as_float(y)), Value::Int),
        (x, y) => Value::Float(x.as_f64() + y.as_f64()),
    })
}

/// `$a - $b`.
pub fn sub(a: &Value, b: &Value) -> Result<Value, Error> {
    let (x, y) = operands(a, "-", b)?;
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) => x
            .checked_sub(y)
            .map_or_else(|| Value::Float(as_float(x) - as_float(y)), Value::Int),
        (x, y) => Value::Float(x.as_f64() - y.as_f64()),
    })
}

/// `$a * $b`.
pub fn mul(a: &Value, b: &Value) -> Result<Value, Error> {
    let (x, y) = operands(a, "*", b)?;
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) => x
            .checked_mul(y)
            .map_or_else(|| Value::Float(as_float(x) * as_float(y)), Value::Int),
        (x, y) => Value::Float(x.as_f64() * y.as_f64()),
    })
}

#[expect(clippy::cast_precision_loss, reason = "PHP's int to float conversion")]
fn as_float(i: i64) -> f64 {
    i as f64
}

/// `$a / $b`.
pub fn div(a: &Value, b: &Value) -> Result<Value, Error> {
    let (x, y) = operands(a, "/", b)?;
    if y.as_f64() == 0.0 {
        return Err(Error::new(Class::DivisionByZeroError, "Division by zero"));
    }
    Ok(match (x, y) {
        (Number::Int(x), Number::Int(y)) if y != 0 && x.checked_rem(y) == Some(0) => x
            .checked_div(y)
            .map_or_else(|| Value::Float(as_float(x) / as_float(y)), Value::Int),
        (x, y) => Value::Float(x.as_f64() / y.as_f64()),
    })
}

/// `(int)` of a float, as PHP converts an operand: truncated toward zero
/// when in range; out of range, the value modulo 2^64 (as PHP 8 does on
/// 64-bit platforms); 0 for infinities and NAN.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "PHP's float to int conversion"
)]
pub fn float_to_int(f: f64) -> i64 {
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    if !f.is_finite() {
        return 0;
    }
    if (-TWO_63..TWO_63).contains(&f) {
        return f.trunc() as i64;
    }
    // modular arithmetic on the truncated value
    let t = f.trunc();
    let two_64 = TWO_63 * 2.0;
    let m = t.rem_euclid(two_64);
    (m as u64) as i64
}

fn number_to_int(n: Number) -> i64 {
    match n {
        Number::Int(i) => i,
        Number::Float(f) => float_to_int(f),
    }
}

/// `$a % $b`.
pub fn modulo(a: &Value, b: &Value) -> Result<Value, Error> {
    let (x, y) = operands(a, "%", b)?;
    let (x, y) = (number_to_int(x), number_to_int(y));
    if y == 0 {
        return Err(Error::new(Class::DivisionByZeroError, "Modulo by zero"));
    }
    Ok(Value::Int(x.wrapping_rem(y)))
}

/// `pow($base, $exp)` (and `**`).
pub fn pow(a: &Value, b: &Value) -> Result<Value, Error> {
    let (x, y) = operands(a, "**", b)?;
    Ok(match (x, y) {
        (Number::Int(base), Number::Int(exp)) if exp >= 0 => int_pow(base, exp),
        (x, y) => Value::Float(float_pow(x.as_f64(), y.as_f64())),
    })
}

/// `pow_function_base()` for ints: square and multiply, and on overflow
/// the float product times the rest of the power.
fn int_pow(base: i64, exp: i64) -> Value {
    if exp == 0 {
        return Value::Int(1);
    }
    if base == 0 {
        return Value::Int(0);
    }
    let (mut l1, mut l2, mut i) = (1i64, base, exp);
    while i >= 1 {
        if i % 2 == 1 {
            i -= 1;
            match l1.checked_mul(l2) {
                Some(v) => l1 = v,
                None => {
                    return Value::Float(
                        as_float(l1) * as_float(l2) * float_pow(as_float(l2), as_float(i)),
                    );
                }
            }
        } else {
            i /= 2;
            match l2.checked_mul(l2) {
                Some(v) => l2 = v,
                None => {
                    return Value::Float(
                        as_float(l1) * float_pow(as_float(l2) * as_float(l2), as_float(i)),
                    );
                }
            }
        }
    }
    Value::Int(l1)
}

fn float_pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

/// An operand of `|`, `&`, `^`, `<<` and `>>` as an int.
fn int_operand(value: &Value) -> Option<i64> {
    arithmetic_operand(value).map(number_to_int)
}

fn int_operands(a: &Value, op: &str, b: &Value) -> Result<(i64, i64), Error> {
    match (int_operand(a), int_operand(b)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(unsupported(a, op, b)),
    }
}

/// `$a | $b`, `$a & $b`, `$a ^ $b`; two strings combine byte by byte.
pub fn bitwise(a: &Value, op: &str, b: &Value) -> Result<Value, Error> {
    if let (Value::String(x), Value::String(y)) = (a, b) {
        let (x, y) = (x.as_bytes(), y.as_bytes());
        let bytes: Vec<u8> = match op {
            "|" => {
                let (long, short) = if x.len() >= y.len() { (x, y) } else { (y, x) };
                long.iter()
                    .enumerate()
                    .map(|(i, &c)| c | short.get(i).copied().unwrap_or(0))
                    .collect()
            }
            "&" => x.iter().zip(y).map(|(p, q)| p & q).collect(),
            _ => x.iter().zip(y).map(|(p, q)| p ^ q).collect(),
        };
        return Ok(Value::String(PhpString::from(bytes)));
    }
    let (x, y) = int_operands(a, op, b)?;
    Ok(Value::Int(match op {
        "|" => x | y,
        "&" => x & y,
        _ => x ^ y,
    }))
}

/// `$a << $b` and `$a >> $b`.
pub fn shift(a: &Value, op: &str, b: &Value) -> Result<Value, Error> {
    let (x, y) = int_operands(a, op, b)?;
    if y < 0 {
        return Err(Error::new(
            Class::ArithmeticError,
            "Bit shift by negative number",
        ));
    }
    Ok(Value::Int(if op == "<<" {
        u32::try_from(y)
            .ok()
            .filter(|&s| s < 64)
            .map_or(0, |s| x.wrapping_shl(s))
    } else {
        u32::try_from(y)
            .ok()
            .filter(|&s| s < 64)
            .map_or(if x < 0 { -1 } else { 0 }, |s| x >> s)
    }))
}

/// `-$a` (PHP multiplies by -1).
pub fn negate(a: &Value) -> Result<Value, Error> {
    mul(a, &Value::Int(-1))
}

/// `+$a` (PHP multiplies by 1).
pub fn plus(a: &Value) -> Result<Value, Error> {
    mul(a, &Value::Int(1))
}

/// `~$a`.
pub fn bitwise_not(a: &Value) -> Result<Value, Error> {
    Ok(match a {
        Value::Int(i) => Value::Int(!i),
        Value::Float(f) => Value::Int(!float_to_int(*f)),
        Value::String(s) => Value::String(PhpString::from(
            s.as_bytes().iter().map(|b| !b).collect::<Vec<_>>(),
        )),
        other => {
            return Err(Error::type_error(format!(
                "Cannot perform bitwise not on {}",
                value_name(other)
            )));
        }
    })
}

/// `(string) $value` as the `.` operator converts it: arrays are
/// `"Array"` (PHP warns), objects need `__toString()`.
pub fn to_string(value: &Value) -> Result<PhpString, Error> {
    match value {
        Value::Array(_) => Ok(PhpString::from("Array")),
        Value::Object(o) => o.to_php_string().ok_or_else(|| {
            Error::new(
                Class::Error,
                format!(
                    "Object of class {} could not be converted to string",
                    php_value::value::debug_class_name(o.class())
                ),
            )
        }),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// `$a . $b`.
pub fn concat(a: &Value, b: &Value) -> Result<Value, Error> {
    let mut out = to_string(a)?.as_bytes().to_vec();
    out.extend_from_slice(to_string(b)?.as_bytes());
    Ok(Value::String(PhpString::from(out)))
}

/// A value passed to an internal function's `string` parameter, without
/// `strict_types`: scalars and null convert (null is deprecated),
/// `\Stringable` objects give their string, other values are a
/// `TypeError`.
pub fn string_argument(
    function: &str,
    position: usize,
    parameter: &str,
    value: &Value,
) -> Result<PhpString, Error> {
    match value {
        Value::Array(_) => Err(argument_type_error(
            function, position, parameter, "string", value,
        )),
        Value::Object(o) => o
            .to_php_string()
            .ok_or_else(|| argument_type_error(function, position, parameter, "string", value)),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// `f(): Argument #n ($name) must be of type T, U given`.
pub fn argument_type_error(
    function: &str,
    position: usize,
    parameter: &str,
    expected: &str,
    value: &Value,
) -> Error {
    Error::type_error(format!(
        "{function}(): Argument #{position} (${parameter}) must be of type {expected}, {} given",
        value_name(value)
    ))
}

/// `str_contains()`, `str_starts_with()` and `str_ends_with()`.
pub fn string_test(function: &str, haystack: &Value, needle: &Value) -> Result<Value, Error> {
    let h = string_argument(function, 1, "haystack", haystack)?;
    let n = string_argument(function, 2, "needle", needle)?;
    let (h, n) = (h.as_bytes(), n.as_bytes());
    Ok(Value::Bool(match function {
        "str_contains" => n.is_empty() || h.windows(n.len()).any(|w| w == n),
        "str_starts_with" => h.starts_with(n),
        _ => h.ends_with(n),
    }))
}

/// `in_array($needle, $haystack, true)`.
pub fn in_array_strict(needle: &Value, haystack: &Value) -> Result<bool, Error> {
    match haystack {
        Value::Array(a) => Ok(a.values().any(|v| v.identical(needle))),
        other => Err(argument_type_error(
            "in_array", 2, "haystack", "array", other,
        )),
    }
}

/// The three-way comparison under PHP 8's rules (`<=>`, and what `<`,
/// `<=`, `>`, `>=` use): -1, 0 or 1. Values that do not compare (arrays
/// with different keys, objects of different classes, enum cases, NAN)
/// give 1, so that `<` and `>` are both false.
pub fn compare(a: &Value, b: &Value) -> Result<i32, Error> {
    use Value::{Array, Bool, Float, Int, Null, Object, String};
    let nan = |v: &Value| matches!(v, Float(f) if f.is_nan());
    Ok(match (a, b) {
        (Int(x), Int(y)) => ordering(x.cmp(y)),
        (Int(_) | Float(_), Int(_) | Float(_)) => {
            float_compare(number(a).as_f64(), number(b).as_f64())
        }
        (Array(x), Array(y)) => compare_arrays(x, y)?,
        (Null, Null) => 0,
        (Object(x), Object(y)) => {
            if a.identical(b) {
                0
            } else if is_enum(a) || is_enum(b) || x.class() != y.class() {
                1
            } else {
                compare_arrays(&x.properties(), &y.properties())?
            }
        }
        // null against an object compares as false against true
        (Null, Object(_)) => -1,
        (Object(_), Null) => 1,
        (Object(o), other) => compare_object(o.as_ref(), a, other, true)?,
        (other, Object(o)) => compare_object(o.as_ref(), b, other, false)?,
        (Null, Bool(y)) => ordering(false.cmp(y)),
        (Bool(x), Null) => ordering(x.cmp(&false)),
        (Bool(x), _) => ordering(x.cmp(&b.is_truthy())),
        (_, Bool(y)) => ordering(a.is_truthy().cmp(y)),
        (Null, String(s)) => {
            if s.is_empty() {
                0
            } else {
                -1
            }
        }
        (String(s), Null) => i32::from(!s.is_empty()),
        (Null, _) => {
            if b.is_truthy() {
                -1
            } else {
                0
            }
        }
        (_, Null) => i32::from(a.is_truthy()),
        (String(x), String(y)) => compare_strings(x.as_bytes(), y.as_bytes()),
        // a NAN does not compare, on either side
        (Int(_) | Float(_), String(_)) | (String(_), Int(_) | Float(_)) if nan(a) || nan(b) => 1,
        (Int(_) | Float(_), String(s)) => compare_number_string(number(a), s.as_bytes()),
        (String(s), Int(_) | Float(_)) => -compare_number_string(number(b), s.as_bytes()),
        (Array(_), _) => 1,
        (_, Array(_)) => -1,
    })
}

fn is_enum(value: &Value) -> bool {
    matches!(value, Value::Object(o) if o.downcast_ref::<EnumCase>().is_some())
}

/// `zend_std_compare_objects()` with one object: the object is cast to
/// the other operand's type. A cast that fails makes the object the
/// greater, except to int or float, where it counts as 1 (PHP warns).
/// Enum cases do not compare.
fn compare_object(
    object: &dyn PhpObject,
    object_value: &Value,
    other: &Value,
    object_lhs: bool,
) -> Result<i32, Error> {
    if is_enum(object_value) {
        return Ok(1);
    }
    let casted = match other {
        Value::String(_) => object.to_php_string().map(Value::String),
        Value::Bool(_) => Some(Value::Bool(true)),
        Value::Int(_) => Some(Value::Int(1)),
        Value::Float(_) => Some(Value::Float(1.0)),
        _ => None,
    };
    let Some(casted) = casted else {
        return Ok(if object_lhs { 1 } else { -1 });
    };
    if object_lhs {
        compare(&casted, other)
    } else {
        compare(other, &casted)
    }
}

fn ordering(o: Ordering) -> i32 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

#[expect(clippy::float_cmp, reason = "PHP's float comparison")]
fn float_compare(x: f64, y: f64) -> i32 {
    if x == y {
        0
    } else if x < y {
        -1
    } else {
        1
    }
}

fn number(value: &Value) -> Number {
    match value {
        Value::Int(i) => Number::Int(*i),
        Value::Float(f) => Number::Float(*f),
        _ => Number::Int(0),
    }
}

fn compare_numbers(x: Number, y: Number) -> i32 {
    match (x, y) {
        (Number::Int(x), Number::Int(y)) => ordering(x.cmp(&y)),
        (x, y) => float_compare(x.as_f64(), y.as_f64()),
    }
}

fn binary_compare(x: &[u8], y: &[u8]) -> i32 {
    ordering(x.cmp(y))
}

/// Two strings: as numbers when both are numeric, else bytewise.
fn compare_strings(x: &[u8], y: &[u8]) -> i32 {
    match (numeric_value(x), numeric_value(y)) {
        (Some(a), Some(b)) => {
            // two integer strings that overflow to the same float compare
            // as strings
            let integer = |s: &[u8]| s.iter().all(|c| !matches!(c, b'.' | b'e' | b'E'));
            if let (Number::Float(fa), Number::Float(fb)) = (a, b)
                && integer(x)
                && integer(y)
                && float_compare(fa, fb) == 0
            {
                return binary_compare(x, y);
            }
            compare_numbers(a, b)
        }
        _ => binary_compare(x, y),
    }
}

fn number_string(n: Number) -> String {
    match n {
        Number::Int(i) => i.to_string(),
        Number::Float(f) => float_to_string(f),
    }
}

fn compare_number_string(n: Number, s: &[u8]) -> i32 {
    if let Some(m) = numeric_value(s) {
        return compare_numbers(n, m);
    }
    if let Number::Float(f) = n
        && f.is_nan()
    {
        return 1;
    }
    binary_compare(number_string(n).as_bytes(), s)
}

fn compare_arrays(x: &PhpArray, y: &PhpArray) -> Result<i32, Error> {
    if x.len() != y.len() {
        return Ok(ordering(x.len().cmp(&y.len())));
    }
    for (k, v) in x.iter() {
        let Some(w) = y.get(k) else { return Ok(1) };
        let c = compare(v, w)?;
        if c != 0 {
            return Ok(c);
        }
    }
    Ok(0)
}

/// `$a < $b`.
pub fn less(a: &Value, b: &Value) -> Result<bool, Error> {
    Ok(compare(a, b)? < 0)
}

/// `$a <= $b`.
pub fn less_or_equal(a: &Value, b: &Value) -> Result<bool, Error> {
    Ok(compare(a, b)? <= 0)
}

/// `$a > $b`, which PHP runs as `$b < $a`.
pub fn greater(a: &Value, b: &Value) -> Result<bool, Error> {
    less(b, a)
}

/// `$a >= $b`, which PHP runs as `$b <= $a`.
pub fn greater_or_equal(a: &Value, b: &Value) -> Result<bool, Error> {
    less_or_equal(b, a)
}

/// `$a == $b`.
pub fn equals(a: &Value, b: &Value) -> bool {
    // an enum case is equal only to itself
    if is_enum(a) || is_enum(b) {
        return a.identical(b);
    }
    loose_equals(a, b)
}

/// The most elements `range()` builds here. PHP builds more, up to its
/// memory limit (128 MB by default, a fatal error beyond it); this port
/// refuses larger ranges instead of allocating them.
pub const MAX_RANGE: usize = 1 << 20;

fn range_too_large() -> Error {
    Error::new(
        Class::Unsupported,
        format!("range() of more than {MAX_RANGE} elements"),
    )
}

/// A `range()` bound: the value it stands for.
enum Bound {
    Int(i64),
    Float(f64),
    Byte(u8),
}

/// `range($start, $end)` (step 1).
pub fn range(start: &Value, end: &Value) -> Result<Value, Error> {
    let bound = |value: &Value, position: usize, name: &str| -> Result<Value, Error> {
        match value {
            Value::Array(_) | Value::Object(_) => Err(argument_type_error(
                "range",
                position,
                name,
                "string|int|float",
                value,
            )),
            Value::Null => Ok(Value::Int(0)),
            Value::Bool(b) => Ok(Value::Int(i64::from(*b))),
            other => Ok(other.clone()),
        }
    };
    let start = bound(start, 1, "start")?;
    let end = bound(end, 2, "end")?;
    let byte_bound = |s: &PhpString| !s.is_empty() && (s.len() == 1 || !is_numeric(s.as_bytes()));
    let (low, high) = match (&start, &end) {
        (Value::String(x), Value::String(y)) if byte_bound(x) && byte_bound(y) => {
            (Bound::Byte(x.as_bytes()[0]), Bound::Byte(y.as_bytes()[0]))
        }
        _ => (range_number(&start), range_number(&end)),
    };
    let mut out = PhpArray::new();
    match (low, high) {
        (Bound::Byte(x), Bound::Byte(y)) => {
            let bytes: Vec<u8> = if x <= y {
                (x..=y).collect()
            } else {
                (y..=x).rev().collect()
            };
            for b in bytes {
                push(&mut out, Value::String(PhpString::from(vec![b])));
            }
        }
        (Bound::Int(x), Bound::Int(y)) => {
            let count = x.abs_diff(y);
            if count >= MAX_RANGE as u64 {
                return Err(range_too_large());
            }
            let mut v = x;
            loop {
                push(&mut out, Value::Int(v));
                if v == y {
                    break;
                }
                v = if x < y { v + 1 } else { v - 1 };
            }
        }
        (low, high) => {
            let x = bound_float(&low);
            let y = bound_float(&high);
            if !x.is_finite() || !y.is_finite() {
                return Err(Error::new(
                    Class::ValueError,
                    format!(
                        "range(): Argument #{} (${}) must be a finite number, INF provided",
                        if x.is_finite() { 2 } else { 1 },
                        if x.is_finite() { "end" } else { "start" }
                    ),
                ));
            }
            let span = (x - y).abs();
            if span > 0.0 && span < 1.0 {
                return Err(Error::new(
                    Class::ValueError,
                    "range(): Argument #3 ($step) must be less than the range spanned by argument #1 ($start) and argument #2 ($end)",
                ));
            }
            if span >= as_float(i64::try_from(MAX_RANGE).unwrap_or(i64::MAX)) {
                return Err(range_too_large());
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "span is below MAX_RANGE"
            )]
            let steps = span.floor() as u64;
            for i in 0..=steps {
                let offset = as_float(i64::try_from(i).unwrap_or(0));
                push(
                    &mut out,
                    Value::Float(if x <= y { x + offset } else { x - offset }),
                );
            }
        }
    }
    Ok(Value::Array(out))
}

fn push(array: &mut PhpArray, value: Value) {
    let index = i64::try_from(array.len()).unwrap_or(i64::MAX);
    array.insert(index, value);
}

fn range_number(value: &Value) -> Bound {
    match value {
        Value::Int(i) => Bound::Int(*i),
        Value::Float(f) => Bound::Float(*f),
        Value::String(s) => match leading_numeric_value(s.as_bytes()) {
            Some(Number::Int(i)) => Bound::Int(i),
            Some(Number::Float(f)) => Bound::Float(f),
            None => Bound::Int(0),
        },
        _ => Bound::Int(0),
    }
}

fn bound_float(b: &Bound) -> f64 {
    match b {
        Bound::Int(i) => as_float(*i),
        Bound::Float(f) => *f,
        Bound::Byte(b) => f64::from(*b),
    }
}

/// `min(...)` and `max(...)`.
pub fn min_max(function: &str, arguments: &[Value]) -> Result<Value, Error> {
    let values: Vec<Value> = match arguments {
        [] => {
            return Err(Error::new(
                Class::ArgumentCountError,
                format!("{function}() expects at least 1 argument, 0 given"),
            ));
        }
        [Value::Array(a)] => {
            if a.is_empty() {
                return Err(Error::new(
                    Class::ValueError,
                    format!("{function}(): Argument #1 ($value) must contain at least one element"),
                ));
            }
            a.values().cloned().collect()
        }
        [other] => {
            return Err(argument_type_error(function, 1, "value", "array", other));
        }
        many => many.to_vec(),
    };
    let mut best = values[0].clone();
    for v in &values[1..] {
        let better = if function == "min" {
            less(v, &best)?
        } else {
            less(&best, v)?
        };
        if better {
            best = v.clone();
        }
    }
    Ok(best)
}

/// `strtoupper()` and `strtolower()` (ASCII only, as PHP 8.2+).
pub fn change_case(function: &str, arguments: &[Value]) -> Result<Value, Error> {
    let [value] = arguments else {
        return Err(Error::new(
            Class::ArgumentCountError,
            format!(
                "{function}() expects exactly 1 argument, {} given",
                arguments.len()
            ),
        ));
    };
    let s = string_argument(function, 1, "string", value)?;
    let bytes: Vec<u8> = if function == "strtoupper" {
        s.as_bytes().to_ascii_uppercase()
    } else {
        s.as_bytes().to_ascii_lowercase()
    };
    Ok(Value::String(PhpString::from(bytes)))
}

/// `$array[$key]` for reading: the key PHP uses.
pub fn read_key(key: &Value, container: &str) -> Result<Key, Error> {
    Key::from_value(key).ok_or_else(|| {
        Error::type_error(format!(
            "Cannot access offset of type {} on {container}",
            key.debug_type()
        ))
    })
}

/// `$array[$key] = ...`: the key PHP uses.
pub fn write_key(key: &Value) -> Result<Key, Error> {
    Key::from_value(key).ok_or_else(|| {
        Error::type_error(format!(
            "Cannot access offset of type {} on array",
            key.debug_type()
        ))
    })
}

/// `isset($string[$offset]) ? $string[$offset] : null`, as `??` reads a
/// string offset.
pub fn string_offset_or_null(s: &[u8], offset: &Value) -> Value {
    let index = match offset {
        Value::Int(i) => Some(*i),
        Value::String(t) => match Key::from_bytes(t.as_bytes()) {
            Key::Int(i) => Some(i),
            Key::Str(_) => None,
        },
        Value::Float(f) => Some(float_to_int(*f)),
        Value::Bool(b) => Some(i64::from(*b)),
        _ => None,
    };
    let len = i64::try_from(s.len()).unwrap_or(i64::MAX);
    match index {
        Some(i) if i >= 0 && i < len => usize::try_from(i)
            .ok()
            .map_or(Value::Null, |i| Value::String(PhpString::from(vec![s[i]]))),
        Some(i) if i < 0 && -i <= len => usize::try_from(len + i)
            .ok()
            .map_or(Value::Null, |i| Value::String(PhpString::from(vec![s[i]]))),
        _ => Value::Null,
    }
}

/// The key `isset($array[$key])` (and `??`) looks up.
pub fn isset_key(key: &Value) -> Result<Key, Error> {
    Key::from_value(key).ok_or_else(|| {
        Error::type_error(format!(
            "Cannot access offset of type {} in isset or empty",
            key.debug_type()
        ))
    })
}

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
//! - `json_encode()` of scalars and of the empty values `VariableNode`
//!   reports, and `var_export()` of enum cases;
//! - for `XmlUtils::phpize()` and `loadFile()` (checked against
//!   `tests/data/xml_utils.json`, `scripts/record.sh config-xml-utils`):
//!   `trim()` with its default characters, `ctype_digit()`,
//!   `intval($string, 8)` (a C `strtol()`: leading octal digits, saturating),
//!   `decoct()` of a non-negative int, and `bindec()` (characters other than
//!   `0` and `1` skipped, continuing in float once the int overflows).
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
    value.is_truthy()
}

/// A key as the string PHP gives it in a string context.
pub fn key_bytes(key: &Key) -> Vec<u8> {
    key.bytes().into_owned()
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

/// A value's type as PHP's `TypeError` messages name it ("given",
/// "returned"): `true` and `false` for bools, the class for objects.
pub fn error_type_name(value: &Value) -> String {
    match value {
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        other => other.debug_type(),
    }
}

/// `levenshtein($a, $b)` with insertion, replacement and deletion costs 1.
pub use php_value::string::levenshtein;

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
    use Value::{Bool, Float, Int, Null, String};
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
        if let Some((from, to)) = found {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
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

/// A `PhpString` from bytes.
pub fn string(bytes: impl Into<Vec<u8>>) -> PhpString {
    PhpString::from(bytes.into())
}

/// Whether a float is in the int range, so PHP's coercive typing makes
/// it an int (truncated) where a union also takes a string.
pub fn float_fits_int(f: f64) -> bool {
    (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&f)
}

/// A class name as PHP compares it: without a leading backslash and
/// ASCII-lowercased.
pub fn class_name_key(class: &str) -> String {
    class
        .strip_prefix('\\')
        .unwrap_or(class)
        .to_ascii_lowercase()
}

/// `json_encode($value)` with no flags; `None` where PHP returns `false`
/// (invalid UTF-8, INF or NAN, a unit enum). A list is a JSON array,
/// another array or an object's properties a JSON object, and a backed
/// enum case its value.
pub fn json_encode(value: &Value) -> Option<Vec<u8>> {
    json_encode_with(value, JsonFlags::default(), false)
}

/// `json_encode($value, $flags)` with `JSON_UNESCAPED_SLASHES` /
/// `JSON_UNESCAPED_UNICODE` (`flags`) and `JSON_PRESERVE_ZERO_FRACTION`
/// (`preserve_zero_fraction`); otherwise as [`json_encode`].
pub fn json_encode_with(
    value: &Value,
    flags: JsonFlags,
    preserve_zero_fraction: bool,
) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let pairs = |out: &mut Vec<u8>, array: &PhpArray| -> Option<()> {
        out.push(b'{');
        for (i, (k, v)) in array.iter().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            out.extend_from_slice(encode_string(&key_bytes(k), flags)?.as_bytes());
            out.push(b':');
            out.extend_from_slice(&json_encode_with(v, flags, preserve_zero_fraction)?);
        }
        out.push(b'}');
        Some(())
    };
    match value {
        Value::Array(array) if !array.is_empty() && array.is_list() => {
            out.push(b'[');
            for (i, v) in array.values().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(&json_encode_with(v, flags, preserve_zero_fraction)?);
            }
            out.push(b']');
        }
        Value::Array(array) if !array.is_empty() => pairs(&mut out, array)?,
        Value::Object(object) => match object.downcast_ref::<EnumCase>() {
            Some(case) => {
                return case
                    .value()
                    .and_then(|v| json_encode_with(v, flags, preserve_zero_fraction));
            }
            None => pairs(&mut out, &object.properties())?,
        },
        scalar => return json_encode_scalar(scalar, flags, preserve_zero_fraction),
    }
    Some(out)
}

/// `(string) $value` as concatenation and `implode()` do it: an array
/// gives "Array" (PHP also warns), an object PHP's `\Error`; no object
/// here is Stringable.
pub fn string_value(value: &Value) -> Result<Vec<u8>, crate::Error> {
    match value {
        Value::Object(o) => Err(crate::Error::new(
            crate::Class::Error,
            format!(
                "Object of class {} could not be converted to string",
                o.class()
            ),
        )),
        other => Ok(to_bytes(other)),
    }
}

fn format_error(class: &str, message: impl Into<Vec<u8>>) -> crate::Error {
    crate::Error::new(
        crate::Class::Other {
            class: class.to_owned(),
            exception: false,
        },
        message,
    )
}

/// `sprintf($format, ...$args)`, as PHP's documentation describes it:
/// `%[argnum$][flags][width][.precision]specifier` with the flags `-`,
/// `+`, space, `0` and `'char`. The specifiers `s`, `d`, `u`, `c`, `x`,
/// `X`, `o`, `b`, `e`, `E`, `f`, `F` and `%` are written (a float's
/// precision is capped at 53 digits, without PHP's notice); `g`, `G`,
/// `h`, `H` and `*` widths are refused with an `\Error` naming them,
/// never approximated. Left-justified `d` and `u` pad with spaces where
/// `0` is asked, as PHP does. Too few arguments is PHP's
/// `ArgumentCountError`, a bad specifier its `ValueError`.
pub fn sprintf(format: &[u8], args: &[Value]) -> Result<Vec<u8>, crate::Error> {
    struct Spec {
        arg: usize,
        left: bool,
        plus: bool,
        pad: u8,
        width: usize,
        precision: Option<usize>,
        conversion: u8,
    }
    let mut pieces: Vec<Result<Vec<u8>, Spec>> = Vec::new();
    let mut literal = Vec::new();
    let mut next_arg = 0usize;
    let mut max_arg = 0usize;
    let mut i = 0;
    let digits = |format: &[u8], mut i: usize| -> (usize, Option<usize>) {
        let start = i;
        let mut n: usize = 0;
        while let Some(d) = format.get(i).filter(|b| b.is_ascii_digit()) {
            n = n.saturating_mul(10).saturating_add(usize::from(d - b'0'));
            i += 1;
        }
        (i, (i > start).then_some(n))
    };
    while i < format.len() {
        let b = format[i];
        i += 1;
        if b != b'%' {
            literal.push(b);
            continue;
        }
        if format.get(i) == Some(&b'%') {
            literal.push(b'%');
            i += 1;
            continue;
        }
        pieces.push(Ok(std::mem::take(&mut literal)));
        // An argnum is digits followed by "$".
        let mut arg = None;
        let (end, n) = digits(format, i);
        if let Some(n) = n
            && format.get(end) == Some(&b'$')
        {
            if n == 0 || n >= 2_147_483_647 {
                return Err(format_error(
                    "ValueError",
                    "Argument number specifier must be greater than zero and less than 2147483647",
                ));
            }
            arg = Some(n - 1);
            i = end + 1;
        }
        let (mut left, mut plus, mut pad) = (false, false, b' ');
        loop {
            match format.get(i) {
                Some(b'-') => left = true,
                Some(b'+') => plus = true,
                Some(b' ') => pad = b' ',
                Some(b'0') => pad = b'0',
                Some(b'\'') => match format.get(i + 1) {
                    Some(&c) => {
                        pad = c;
                        i += 1;
                    }
                    None => {
                        return Err(format_error("ValueError", "Missing padding character"));
                    }
                },
                _ => break,
            }
            i += 1;
        }
        if format.get(i) == Some(&b'*') {
            return Err(format_error(
                "Error",
                "sprintf(): a \"*\" width or precision is not supported by this port",
            ));
        }
        let (end, width) = digits(format, i);
        i = end;
        let mut precision = None;
        if format.get(i) == Some(&b'.') {
            if format.get(i + 1) == Some(&b'*') {
                return Err(format_error(
                    "Error",
                    "sprintf(): a \"*\" width or precision is not supported by this port",
                ));
            }
            let (end, p) = digits(format, i + 1);
            precision = Some(p.unwrap_or(0));
            i = end;
        }
        let Some(&conversion) = format.get(i) else {
            return Err(format_error(
                "ValueError",
                "Missing format specifier at end of string",
            ));
        };
        i += 1;
        match conversion {
            b's' | b'd' | b'u' | b'c' | b'x' | b'X' | b'o' | b'b' | b'e' | b'E' | b'f' | b'F' => {}
            b'g' | b'G' | b'h' | b'H' => {
                return Err(format_error(
                    "Error",
                    format!(
                        "sprintf(): the \"{}\" specifier is not supported by this port",
                        conversion as char
                    ),
                ));
            }
            other => {
                return Err(format_error(
                    "ValueError",
                    format!("Unknown format specifier \"{}\"", other as char),
                ));
            }
        }
        let arg = arg.unwrap_or_else(|| {
            next_arg += 1;
            next_arg - 1
        });
        max_arg = max_arg.max(arg + 1);
        pieces.push(Err(Spec {
            arg,
            left,
            plus,
            pad,
            width: width.unwrap_or(0),
            precision,
            conversion,
        }));
    }
    pieces.push(Ok(literal));
    if max_arg > args.len() {
        return Err(format_error(
            "ArgumentCountError",
            format!(
                "{} arguments are required, {} given",
                max_arg + 1,
                args.len() + 1
            ),
        ));
    }

    let mut out = Vec::new();
    for piece in pieces {
        let spec = match piece {
            Ok(text) => {
                out.extend_from_slice(&text);
                continue;
            }
            Err(spec) => spec,
        };
        let value = &args[spec.arg];
        let int = || match value {
            Value::String(s) => php_value::numeric::string_to_int(s),
            Value::Bool(b) => i64::from(*b),
            Value::Int(n) => *n,
            Value::Float(f) => *f as i64,
            Value::Null => 0,
            _ => 1,
        };
        let float = || match value {
            Value::String(s) => php_value::numeric::string_to_float(s),
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Int(n) => *n as f64,
            Value::Float(f) => *f,
            Value::Null => 0.0,
            _ => 1.0,
        };
        let float_sign = |f: f64| -> &'static [u8] {
            if f < 0.0 {
                b"-"
            } else if spec.plus {
                b"+"
            } else {
                b""
            }
        };
        let (sign, mut body): (&[u8], Vec<u8>) = match spec.conversion {
            b's' => {
                let mut s = to_bytes(value);
                if let Some(p) = spec.precision {
                    s.truncate(p);
                }
                (b"", s)
            }
            b'd' => {
                let n = int();
                let sign: &[u8] = if n < 0 {
                    b"-"
                } else if spec.plus {
                    b"+"
                } else {
                    b""
                };
                (sign, n.unsigned_abs().to_string().into_bytes())
            }
            b'u' => (b"", (int() as u64).to_string().into_bytes()),
            b'c' => {
                out.push(int() as u8);
                continue;
            }
            b'x' => (b"", format!("{:x}", int() as u64).into_bytes()),
            b'X' => (b"", format!("{:X}", int() as u64).into_bytes()),
            b'o' => (b"", format!("{:o}", int() as u64).into_bytes()),
            b'f' | b'F' => {
                let f = float();
                let p = spec.precision.unwrap_or(6).min(53);
                (float_sign(f), format!("{:.p$}", f.abs()).into_bytes())
            }
            b'e' | b'E' => {
                let f = float();
                let p = spec.precision.unwrap_or(6).min(53);
                let text = format!("{:.p$e}", f.abs());
                let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
                let (exp_sign, digits) = match exponent.strip_prefix('-') {
                    Some(d) => ("-", d),
                    None => ("+", exponent),
                };
                let e = if spec.conversion == b'E' { 'E' } else { 'e' };
                (
                    float_sign(f),
                    format!("{mantissa}{e}{exp_sign}{digits}").into_bytes(),
                )
            }
            _ => (b"", format!("{:b}", int() as u64).into_bytes()),
        };
        let len = sign.len() + body.len();
        if len >= spec.width {
            out.extend_from_slice(sign);
            out.append(&mut body);
        } else {
            let pad = if spec.left && spec.pad == b'0' && matches!(spec.conversion, b'd' | b'u') {
                b' '
            } else {
                spec.pad
            };
            let fill = vec![pad; spec.width - len];
            if spec.left {
                out.extend_from_slice(sign);
                out.append(&mut body);
                out.extend_from_slice(&fill);
            } else if pad == b'0' {
                out.extend_from_slice(sign);
                out.extend_from_slice(&fill);
                out.append(&mut body);
            } else {
                out.extend_from_slice(&fill);
                out.extend_from_slice(sign);
                out.append(&mut body);
            }
        }
    }
    Ok(out)
}

/// `trim($string)`: strips `" \t\n\r\0\x0B"` from both ends.
pub use php_value::string::trim;

/// `ctype_digit($string)`: non-empty and only ASCII digits.
pub fn ctype_digit(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

/// `intval($string, 8)` of a string of an optional `-` and digits: the
/// leading octal digits, saturating at the int limits.
pub fn intval_octal(s: &[u8]) -> i64 {
    let (negative, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        _ => (false, s),
    };
    let mut value: i64 = 0;
    for &b in digits.iter().take_while(|b| (b'0'..=b'7').contains(b)) {
        let digit = i64::from(b - b'0');
        let next = value.checked_mul(8).and_then(|v| {
            if negative {
                v.checked_sub(digit)
            } else {
                v.checked_add(digit)
            }
        });
        match next {
            Some(v) => value = v,
            None => return if negative { i64::MIN } else { i64::MAX },
        }
    }
    value
}

/// `decoct($int)` of a non-negative int.
pub fn decoct(value: i64) -> String {
    format!("{value:o}")
}

/// `bindec($string)`.
pub fn bindec(s: &[u8]) -> php_value::Number {
    let mut int: i64 = 0;
    let mut float: Option<f64> = None;
    for &b in s.iter().filter(|b| matches!(b, b'0' | b'1')) {
        let digit = i64::from(b - b'0');
        match float {
            Some(f) => float = Some(f * 2.0 + digit as f64),
            None => match int.checked_mul(2).and_then(|v| v.checked_add(digit)) {
                Some(v) => int = v,
                None => float = Some(int as f64 * 2.0 + digit as f64),
            },
        }
    }
    match float {
        Some(f) => php_value::Number::Float(f),
        None => php_value::Number::Int(int),
    }
}

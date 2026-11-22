//! `filter_var()`, for the filters Symfony's `ParameterBag::filter()` and
//! `InputBag::filter()` are used with: `FILTER_UNSAFE_RAW` (the default),
//! `FILTER_VALIDATE_INT`, `FILTER_VALIDATE_BOOL`,
//! `FILTER_SANITIZE_NUMBER_INT` and `FILTER_CALLBACK`. Written from the
//! PHP manual and from 8.4.27's answers recorded by
//! oracle/recorders/filter_var.php (tests/data/filter.json), no PHP
//! source.
//!
//! What was recorded:
//!
//! - A scalar is filtered as its string form (`null` and `false` as `''`,
//!   floats as PHP prints them); a `\Stringable` object as its
//!   `__toString()`; any other object fails.
//! - An array fails unless `FILTER_REQUIRE_ARRAY` or `FILTER_FORCE_ARRAY`
//!   is set (and `FILTER_REQUIRE_SCALAR` is not); then every element is
//!   filtered, at any depth, keys kept. `FILTER_CALLBACK` without a
//!   `flags` entry also walks arrays. A scalar fails under
//!   `FILTER_REQUIRE_ARRAY`, and is wrapped in a list under
//!   `FILTER_FORCE_ARRAY`.
//! - A failed value (the whole input, or one element) is the `default`
//!   option when one is given, else `null` under `FILTER_NULL_ON_FAILURE`,
//!   else `false`. A wrong shape (an array where a scalar is required, or
//!   the reverse) ignores `default`.
//! - `FILTER_VALIDATE_INT` and `FILTER_VALIDATE_BOOL` ignore spaces, tabs,
//!   `\n`, `\r` and `\v` around the value (not `\f` or NUL).
//!
//! Any other filter, flag or callback form is refused with
//! [`Unsupported`], never approximated.
//!
//! Upstream: none

use crate::{
    array::{Key, PhpArray},
    object::Closure,
    string::PhpString,
    value::Value,
};

// The constants' values, as PHP 8.4 defines them (checked against the
// recording).
pub const FILTER_FLAG_NONE: i64 = 0;
pub const FILTER_REQUIRE_SCALAR: i64 = 33_554_432;
pub const FILTER_REQUIRE_ARRAY: i64 = 16_777_216;
pub const FILTER_FORCE_ARRAY: i64 = 67_108_864;
pub const FILTER_NULL_ON_FAILURE: i64 = 134_217_728;
pub const FILTER_VALIDATE_INT: i64 = 257;
pub const FILTER_VALIDATE_BOOLEAN: i64 = 258;
pub const FILTER_VALIDATE_BOOL: i64 = 258;
pub const FILTER_VALIDATE_FLOAT: i64 = 259;
pub const FILTER_VALIDATE_REGEXP: i64 = 272;
pub const FILTER_VALIDATE_DOMAIN: i64 = 277;
pub const FILTER_VALIDATE_URL: i64 = 273;
pub const FILTER_VALIDATE_EMAIL: i64 = 274;
pub const FILTER_VALIDATE_IP: i64 = 275;
pub const FILTER_VALIDATE_MAC: i64 = 276;
pub const FILTER_DEFAULT: i64 = 516;
pub const FILTER_UNSAFE_RAW: i64 = 516;
pub const FILTER_SANITIZE_STRING: i64 = 513;
pub const FILTER_SANITIZE_STRIPPED: i64 = 513;
pub const FILTER_SANITIZE_ENCODED: i64 = 514;
pub const FILTER_SANITIZE_SPECIAL_CHARS: i64 = 515;
pub const FILTER_SANITIZE_FULL_SPECIAL_CHARS: i64 = 522;
pub const FILTER_SANITIZE_EMAIL: i64 = 517;
pub const FILTER_SANITIZE_URL: i64 = 518;
pub const FILTER_SANITIZE_NUMBER_INT: i64 = 519;
pub const FILTER_SANITIZE_NUMBER_FLOAT: i64 = 520;
pub const FILTER_SANITIZE_ADD_SLASHES: i64 = 523;
pub const FILTER_CALLBACK: i64 = 1024;
pub const FILTER_FLAG_ALLOW_OCTAL: i64 = 1;
pub const FILTER_FLAG_ALLOW_HEX: i64 = 2;
pub const FILTER_FLAG_STRIP_LOW: i64 = 4;
pub const FILTER_FLAG_STRIP_HIGH: i64 = 8;
pub const FILTER_FLAG_STRIP_BACKTICK: i64 = 512;
pub const FILTER_FLAG_ENCODE_LOW: i64 = 16;
pub const FILTER_FLAG_ENCODE_HIGH: i64 = 32;
pub const FILTER_FLAG_ENCODE_AMP: i64 = 64;
pub const FILTER_FLAG_NO_ENCODE_QUOTES: i64 = 128;
pub const FILTER_FLAG_EMPTY_STRING_NULL: i64 = 256;
pub const FILTER_FLAG_ALLOW_FRACTION: i64 = 4096;
pub const FILTER_FLAG_ALLOW_THOUSAND: i64 = 8192;
pub const FILTER_FLAG_ALLOW_SCIENTIFIC: i64 = 16384;
pub const FILTER_FLAG_PATH_REQUIRED: i64 = 262_144;
pub const FILTER_FLAG_QUERY_REQUIRED: i64 = 524_288;
pub const FILTER_FLAG_IPV4: i64 = 1_048_576;
pub const FILTER_FLAG_IPV6: i64 = 2_097_152;
pub const FILTER_FLAG_NO_RES_RANGE: i64 = 4_194_304;
pub const FILTER_FLAG_NO_PRIV_RANGE: i64 = 8_388_608;
pub const FILTER_FLAG_GLOBAL_RANGE: i64 = 268_435_456;
pub const FILTER_FLAG_HOSTNAME: i64 = 1_048_576;
pub const FILTER_FLAG_EMAIL_UNICODE: i64 = 1_048_576;

/// Every filter constant with its name, as `get_defined_constants()`
/// lists the extension's (without the `INPUT_*` ones).
pub const CONSTANTS: &[(&str, i64)] = &[
    ("FILTER_FLAG_NONE", FILTER_FLAG_NONE),
    ("FILTER_REQUIRE_SCALAR", FILTER_REQUIRE_SCALAR),
    ("FILTER_REQUIRE_ARRAY", FILTER_REQUIRE_ARRAY),
    ("FILTER_FORCE_ARRAY", FILTER_FORCE_ARRAY),
    ("FILTER_NULL_ON_FAILURE", FILTER_NULL_ON_FAILURE),
    ("FILTER_VALIDATE_INT", FILTER_VALIDATE_INT),
    ("FILTER_VALIDATE_BOOLEAN", FILTER_VALIDATE_BOOLEAN),
    ("FILTER_VALIDATE_BOOL", FILTER_VALIDATE_BOOL),
    ("FILTER_VALIDATE_FLOAT", FILTER_VALIDATE_FLOAT),
    ("FILTER_VALIDATE_REGEXP", FILTER_VALIDATE_REGEXP),
    ("FILTER_VALIDATE_DOMAIN", FILTER_VALIDATE_DOMAIN),
    ("FILTER_VALIDATE_URL", FILTER_VALIDATE_URL),
    ("FILTER_VALIDATE_EMAIL", FILTER_VALIDATE_EMAIL),
    ("FILTER_VALIDATE_IP", FILTER_VALIDATE_IP),
    ("FILTER_VALIDATE_MAC", FILTER_VALIDATE_MAC),
    ("FILTER_DEFAULT", FILTER_DEFAULT),
    ("FILTER_UNSAFE_RAW", FILTER_UNSAFE_RAW),
    ("FILTER_SANITIZE_STRING", FILTER_SANITIZE_STRING),
    ("FILTER_SANITIZE_STRIPPED", FILTER_SANITIZE_STRIPPED),
    ("FILTER_SANITIZE_ENCODED", FILTER_SANITIZE_ENCODED),
    (
        "FILTER_SANITIZE_SPECIAL_CHARS",
        FILTER_SANITIZE_SPECIAL_CHARS,
    ),
    (
        "FILTER_SANITIZE_FULL_SPECIAL_CHARS",
        FILTER_SANITIZE_FULL_SPECIAL_CHARS,
    ),
    ("FILTER_SANITIZE_EMAIL", FILTER_SANITIZE_EMAIL),
    ("FILTER_SANITIZE_URL", FILTER_SANITIZE_URL),
    ("FILTER_SANITIZE_NUMBER_INT", FILTER_SANITIZE_NUMBER_INT),
    ("FILTER_SANITIZE_NUMBER_FLOAT", FILTER_SANITIZE_NUMBER_FLOAT),
    ("FILTER_SANITIZE_ADD_SLASHES", FILTER_SANITIZE_ADD_SLASHES),
    ("FILTER_CALLBACK", FILTER_CALLBACK),
    ("FILTER_FLAG_ALLOW_OCTAL", FILTER_FLAG_ALLOW_OCTAL),
    ("FILTER_FLAG_ALLOW_HEX", FILTER_FLAG_ALLOW_HEX),
    ("FILTER_FLAG_STRIP_LOW", FILTER_FLAG_STRIP_LOW),
    ("FILTER_FLAG_STRIP_HIGH", FILTER_FLAG_STRIP_HIGH),
    ("FILTER_FLAG_STRIP_BACKTICK", FILTER_FLAG_STRIP_BACKTICK),
    ("FILTER_FLAG_ENCODE_LOW", FILTER_FLAG_ENCODE_LOW),
    ("FILTER_FLAG_ENCODE_HIGH", FILTER_FLAG_ENCODE_HIGH),
    ("FILTER_FLAG_ENCODE_AMP", FILTER_FLAG_ENCODE_AMP),
    ("FILTER_FLAG_NO_ENCODE_QUOTES", FILTER_FLAG_NO_ENCODE_QUOTES),
    (
        "FILTER_FLAG_EMPTY_STRING_NULL",
        FILTER_FLAG_EMPTY_STRING_NULL,
    ),
    ("FILTER_FLAG_ALLOW_FRACTION", FILTER_FLAG_ALLOW_FRACTION),
    ("FILTER_FLAG_ALLOW_THOUSAND", FILTER_FLAG_ALLOW_THOUSAND),
    ("FILTER_FLAG_ALLOW_SCIENTIFIC", FILTER_FLAG_ALLOW_SCIENTIFIC),
    ("FILTER_FLAG_PATH_REQUIRED", FILTER_FLAG_PATH_REQUIRED),
    ("FILTER_FLAG_QUERY_REQUIRED", FILTER_FLAG_QUERY_REQUIRED),
    ("FILTER_FLAG_IPV4", FILTER_FLAG_IPV4),
    ("FILTER_FLAG_IPV6", FILTER_FLAG_IPV6),
    ("FILTER_FLAG_NO_RES_RANGE", FILTER_FLAG_NO_RES_RANGE),
    ("FILTER_FLAG_NO_PRIV_RANGE", FILTER_FLAG_NO_PRIV_RANGE),
    ("FILTER_FLAG_GLOBAL_RANGE", FILTER_FLAG_GLOBAL_RANGE),
    ("FILTER_FLAG_HOSTNAME", FILTER_FLAG_HOSTNAME),
    ("FILTER_FLAG_EMAIL_UNICODE", FILTER_FLAG_EMAIL_UNICODE),
];

/// The flags that choose how arrays and failures are handled, which every
/// filter accepts.
const SHAPE_FLAGS: i64 =
    FILTER_REQUIRE_SCALAR | FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY | FILTER_NULL_ON_FAILURE;

/// A filter, flag or option this port does not reproduce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "filter_var() is not ported for {}", self.0)
    }
}

impl std::error::Error for Unsupported {}

/// The filters this port reproduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    UnsafeRaw,
    ValidateInt,
    ValidateBool,
    SanitizeNumberInt,
    Callback,
}

impl Filter {
    fn of(id: i64) -> Result<Self, Unsupported> {
        Ok(match id {
            FILTER_UNSAFE_RAW => Self::UnsafeRaw,
            FILTER_VALIDATE_INT => Self::ValidateInt,
            FILTER_VALIDATE_BOOL => Self::ValidateBool,
            FILTER_SANITIZE_NUMBER_INT => Self::SanitizeNumberInt,
            FILTER_CALLBACK => Self::Callback,
            other => {
                let name = CONSTANTS
                    .iter()
                    .find(|(name, value)| *value == other && !name.starts_with("FILTER_FLAG"))
                    .map_or_else(
                        || format!("filter id {other}"),
                        |(name, _)| (*name).to_owned(),
                    );
                return Err(Unsupported(name));
            }
        })
    }

    /// The filter's own flags this port reproduces.
    const fn own_flags(self) -> i64 {
        match self {
            Self::ValidateInt => FILTER_FLAG_ALLOW_OCTAL | FILTER_FLAG_ALLOW_HEX,
            Self::UnsafeRaw | Self::SanitizeNumberInt => FILTER_FLAG_STRIP_LOW,
            Self::ValidateBool | Self::Callback => 0,
        }
    }
}

/// `filter_var()`'s options, read once.
struct Options {
    filter: Filter,
    flags: i64,
    /// The options array had a `flags` entry (or the options were flags).
    has_flags: bool,
    default: Option<Value>,
    min_range: Option<i64>,
    max_range: Option<i64>,
    callback: Option<Closure>,
}

/// `(int)` of a range option: an int, a float truncated, or a numeric
/// string (recorded: `'5'` and `12.5` bound as 5 and 12).
fn option_int(value: &Value, name: &str) -> Result<i64, Unsupported> {
    match value {
        Value::Int(i) => Ok(*i),
        Value::Float(f) if f.is_finite() => Ok(*f as i64),
        Value::String(s) if crate::numeric::is_numeric(s.as_bytes()) => {
            Ok(crate::numeric::string_to_int(s.as_bytes()))
        }
        other => Err(Unsupported(format!(
            "a {} {name} option",
            other.debug_type()
        ))),
    }
}

impl Options {
    fn read(filter: Filter, options: &Value) -> Result<Self, Unsupported> {
        let mut read = Self {
            filter,
            flags: 0,
            has_flags: false,
            default: None,
            min_range: None,
            max_range: None,
            callback: None,
        };
        match options {
            Value::Int(flags) => {
                read.flags = *flags;
                read.has_flags = true;
            }
            Value::Array(array) => {
                if let Some(flags) = array.get(&Key::from("flags")) {
                    read.flags = match flags {
                        Value::Int(i) => *i,
                        other => {
                            return Err(Unsupported(format!("{} flags", other.debug_type())));
                        }
                    };
                    read.has_flags = true;
                }
                match (filter, array.get(&Key::from("options"))) {
                    (Filter::Callback, Some(Value::Object(object))) => {
                        read.callback = object.downcast_ref::<Closure>().cloned();
                    }
                    (Filter::Callback, _) => {}
                    (_, Some(Value::Array(options))) => read.read_options(options)?,
                    (_, None) => {}
                    (_, Some(other)) => {
                        return Err(Unsupported(format!("{} options", other.debug_type())));
                    }
                }
            }
            Value::Null => {}
            other => return Err(Unsupported(format!("{} options", other.debug_type()))),
        }
        if filter == Filter::Callback && read.callback.is_none() {
            return Err(Unsupported("FILTER_CALLBACK without a Closure".to_owned()));
        }
        let known = SHAPE_FLAGS | filter.own_flags();
        if read.flags & !known != 0 {
            return Err(Unsupported(format!("flags {}", read.flags & !known)));
        }
        Ok(read)
    }

    fn read_options(&mut self, options: &PhpArray) -> Result<(), Unsupported> {
        for (key, value) in options.iter() {
            match key.as_str().map(PhpString::as_bytes) {
                Some(b"default") => self.default = Some(value.clone()),
                Some(b"min_range") if self.filter == Filter::ValidateInt => {
                    self.min_range = Some(option_int(value, "min_range")?);
                }
                Some(b"max_range") if self.filter == Filter::ValidateInt => {
                    self.max_range = Some(option_int(value, "max_range")?);
                }
                _ => {
                    return Err(Unsupported(format!(
                        "the {:?} option",
                        key.to_value().to_php_string().unwrap_or_default()
                    )));
                }
            }
        }
        Ok(())
    }

    fn null_on_failure(&self) -> bool {
        self.flags & FILTER_NULL_ON_FAILURE != 0
    }

    /// A value of the wrong shape.
    fn wrong_shape(&self) -> Value {
        if self.null_on_failure() {
            Value::Null
        } else {
            Value::Bool(false)
        }
    }

    /// A value the filter rejected.
    fn failed(&self) -> Value {
        match &self.default {
            Some(default) => default.clone(),
            None => self.wrong_shape(),
        }
    }
}

/// `filter_var($value, $filter, $options)`. `options` is PHP's argument:
/// an int of flags, or an array with `flags` and `options` entries.
pub fn filter_var(value: &Value, filter: i64, options: &Value) -> Result<Value, Unsupported> {
    let options = Options::read(Filter::of(filter)?, options)?;
    let flags = options.flags;
    Ok(match value {
        Value::Array(array) => {
            let walk = flags & FILTER_REQUIRE_SCALAR == 0
                && (flags & (FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY) != 0
                    || (options.filter == Filter::Callback && !options.has_flags));
            if walk {
                Value::Array(walk_array(array, &options))
            } else {
                options.wrong_shape()
            }
        }
        scalar => {
            if flags & FILTER_REQUIRE_ARRAY != 0 {
                options.wrong_shape()
            } else if flags & FILTER_FORCE_ARRAY != 0 {
                Value::Array(PhpArray::list([filter_scalar(scalar, &options)]))
            } else {
                filter_scalar(scalar, &options)
            }
        }
    })
}

fn walk_array(array: &PhpArray, options: &Options) -> PhpArray {
    let mut out = PhpArray::new();
    for (key, item) in array.iter() {
        let filtered = match item {
            Value::Array(inner) => Value::Array(walk_array(inner, options)),
            scalar => filter_scalar(scalar, options),
        };
        out.insert(key.clone(), filtered);
    }
    out
}

/// One value that is not an array.
fn filter_scalar(value: &Value, options: &Options) -> Value {
    let Some(text) = value.to_php_string_or_stringable() else {
        return options.failed();
    };
    let bytes = text.as_bytes();
    let result = match options.filter {
        Filter::UnsafeRaw => Some(Value::String(strip_low(bytes, options.flags))),
        Filter::SanitizeNumberInt => {
            let kept: Vec<u8> = strip_low(bytes, options.flags)
                .as_bytes()
                .iter()
                .copied()
                .filter(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-'))
                .collect();
            Some(Value::String(PhpString::from(kept)))
        }
        Filter::ValidateInt => validate_int(trim(bytes), options).map(Value::Int),
        // Without FILTER_NULL_ON_FAILURE, `false` is also the failure
        // value, so a valid false gives the default (recorded).
        Filter::ValidateBool => validate_bool(trim(bytes))
            .filter(|&b| b || options.default.is_none() || options.null_on_failure())
            .map(Value::Bool),
        Filter::Callback => options
            .callback
            .as_ref()
            .map(|callback| callback.call(&[Value::String(text.clone())])),
    };
    result.unwrap_or_else(|| options.failed())
}

fn strip_low(bytes: &[u8], flags: i64) -> PhpString {
    if flags & FILTER_FLAG_STRIP_LOW == 0 {
        return PhpString::from(bytes);
    }
    PhpString::from(
        bytes
            .iter()
            .copied()
            .filter(|&b| b >= 32)
            .collect::<Vec<u8>>(),
    )
}

/// The whitespace the validating filters ignore around a value.
const fn is_filter_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0B')
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|&b| !is_filter_space(b))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|&b| !is_filter_space(b))
        .map_or(start, |last| last + 1);
    &bytes[start..end]
}

fn validate_bool(bytes: &[u8]) -> Option<bool> {
    match bytes.to_ascii_lowercase().as_slice() {
        b"1" | b"true" | b"on" | b"yes" => Some(true),
        b"" | b"0" | b"false" | b"off" | b"no" => Some(false),
        _ => None,
    }
}

fn validate_int(bytes: &[u8], options: &Options) -> Option<i64> {
    let flags = options.flags;
    let value = if flags & FILTER_FLAG_ALLOW_HEX != 0
        && bytes.len() > 2
        && bytes[0] == b'0'
        && matches!(bytes[1], b'x' | b'X')
    {
        unsigned(&bytes[2..], 16)?
    } else if flags & FILTER_FLAG_ALLOW_OCTAL != 0 && bytes.len() > 1 && bytes[0] == b'0' {
        let digits = match bytes[1] {
            b'o' | b'O' => &bytes[2..],
            _ => &bytes[1..],
        };
        unsigned(digits, 8)?
    } else {
        decimal(bytes)?
    };
    let in_range = options.min_range.is_none_or(|min| value >= min)
        && options.max_range.is_none_or(|max| value <= max);
    in_range.then_some(value)
}

/// Digits of `radix` with no sign, read as an unsigned 64-bit number and
/// taken as an int (so `0xffffffffffffffff` is `-1`, as recorded).
fn unsigned(digits: &[u8], radix: u32) -> Option<i64> {
    if digits.is_empty() {
        return None;
    }
    let mut value: u64 = 0;
    for &b in digits {
        let digit = char::from(b).to_digit(radix)?;
        value = value
            .checked_mul(u64::from(radix))?
            .checked_add(u64::from(digit))?;
    }
    Some(value as i64)
}

/// An optional sign, then `0` or digits without a leading zero.
fn decimal(bytes: &[u8]) -> Option<i64> {
    let (negative, digits) = match bytes.first() {
        Some(b'-') => (true, &bytes[1..]),
        Some(b'+') => (false, &bytes[1..]),
        _ => (false, bytes),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if digits[0] == b'0' && digits.len() > 1 {
        return None;
    }
    let mut value: i64 = 0;
    for &b in digits {
        let digit = i64::from(b - b'0');
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(digit)?
        } else {
            value.checked_add(digit)?
        };
    }
    Some(value)
}

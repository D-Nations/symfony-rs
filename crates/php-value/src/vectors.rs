//! Reading values recorded from PHP (feature `vectors`), and PHPUnit's
//! `assertEquals` comparison, for tests that check Rust against the
//! oracle. The format is oracle/recorders/lib/typed.php's:
//! `["null"]`, `["bool", b]`, `["int", i]`, `["float", "<hex bits>"]`,
//! `["string", "<base64>"]`, `["array", [[key, value], ...]]`,
//! `["object", class, [[name, value], ...]]`, `["enum", class, case,
//! value|null]`, `["datetime", "Y-m-d\TH:i:s.uP", zone, U]`,
//! `["arrayobject", [[key, value], ...]]`, `["tagged", tag, value]` (decoded as an object of class
//! `Symfony\Component\Yaml\Tag\TaggedValue`), and `["throws", class,
//! message]` for an exception.
//!
//! Upstream: none

use std::sync::Arc;

use serde_json::Value as Json;

use crate::{
    array::{Key, PhpArray},
    datetime::{DateTimeImmutable, Fields, TimeZone},
    object::{ArrayObject, EnumCase, GenericObject, StdClass},
    string::PhpString,
    value::Value,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A recorded call: its value, or the exception it threw.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Value(Value),
    Throws { class: String, message: String },
}

fn text(json: &Json) -> Result<&str, Error> {
    json.as_str()
        .ok_or_else(|| format!("expected a string, got {json}").into())
}

/// Decodes the recorders' base64.
pub fn base64(text: &str) -> Result<Vec<u8>, Error> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = ALPHABET.iter().position(|&a| a == c).ok_or("bad base64")? as u32;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn pairs(json: &Json) -> Result<PhpArray, Error> {
    let mut array = PhpArray::new();
    for pair in json.as_array().ok_or("expected pairs")? {
        let key = match decode(&pair[0])? {
            Value::Int(i) => Key::Int(i),
            Value::String(s) => Key::from(s),
            other => return Err(format!("bad key {other:?}").into()),
        };
        array.insert(key, decode(&pair[1])?);
    }
    Ok(array)
}

fn datetime(full: &str, zone: &str) -> Result<DateTimeImmutable, Error> {
    // "-0001-12-10T00:00:00.000000+00:00": the offset is the last 6.
    let (local, offset) = full.split_at(full.len() - 6);
    let negative_year = local.starts_with('-');
    let local = local.trim_start_matches('-');
    let num = |s: &str| s.parse::<i64>().map_err(|e| Error::from(e.to_string()));
    let (date, time) = local.split_once('T').ok_or("no T")?;
    let mut d = date.split('-');
    let (y, m, day) = (
        d.next().ok_or("y")?,
        d.next().ok_or("m")?,
        d.next().ok_or("d")?,
    );
    let (hms, micro) = time.split_once('.').ok_or("no fraction")?;
    let mut t = hms.split(':');
    let fields = Fields {
        year: if negative_year { -num(y)? } else { num(y)? },
        month: num(m)?,
        day: num(day)?,
        hour: num(t.next().ok_or("h")?)?,
        minute: num(t.next().ok_or("i")?)?,
        second: num(t.next().ok_or("s")?)?,
        microsecond: micro.parse()?,
    };
    let seconds = (num(&offset[1..3])? * 3600 + num(&offset[4..6])? * 60) as i32;
    let seconds = if offset.starts_with('-') {
        -seconds
    } else {
        seconds
    };
    let zone = match zone {
        "UTC" => TimeZone::Utc,
        "Z" => TimeZone::Zulu,
        _ => TimeZone::Offset(seconds),
    };
    Ok(DateTimeImmutable::from_fields(fields, zone))
}

/// Decodes one recorded value.
pub fn decode(json: &Json) -> Result<Value, Error> {
    let kind = text(&json[0])?;
    Ok(match kind {
        "null" => Value::Null,
        "bool" => Value::Bool(json[1].as_bool().ok_or("bool")?),
        "int" => Value::Int(json[1].as_i64().ok_or("int")?),
        "float" => Value::Float(f64::from_bits(u64::from_str_radix(text(&json[1])?, 16)?)),
        "string" => Value::String(PhpString::from(base64(text(&json[1])?)?)),
        "array" => Value::Array(pairs(&json[1])?),
        "object" => {
            let class = text(&json[1])?;
            let properties = pairs(&json[2])?;
            if class == StdClass::CLASS {
                StdClass::object(properties)
            } else {
                Value::Object(Arc::new(GenericObject::new(class, properties)))
            }
        }
        "arrayobject" => Value::Object(Arc::new(ArrayObject::new(pairs(&json[1])?))),
        "enum" => {
            let backing = if json[3].is_null() {
                None
            } else {
                Some(decode(&json[3])?)
            };
            Value::Object(EnumCase::new(text(&json[1])?, text(&json[2])?, backing).object())
        }
        "datetime" => Value::Object(Arc::new(datetime(text(&json[1])?, text(&json[2])?)?)),
        "tagged" => {
            let mut properties = PhpArray::new();
            properties.insert("tag", Value::from(text(&json[1])?));
            properties.insert("value", decode(&json[2])?);
            Value::Object(Arc::new(GenericObject::new(
                "Symfony\\Component\\Yaml\\Tag\\TaggedValue",
                properties,
            )))
        }
        other => return Err(format!("cannot decode {other}").into()),
    })
}

/// Decodes a recorded call.
pub fn outcome(json: &Json) -> Result<Outcome, Error> {
    if json[0] == "throws" {
        return Ok(Outcome::Throws {
            class: text(&json[1])?.to_owned(),
            message: text(&json[2])?.to_owned(),
        });
    }
    Ok(Outcome::Value(decode(json)?))
}

fn to_bool(value: &Value) -> bool {
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

/// PHPUnit 11's `assertEquals($expected, $actual)` for these value kinds:
/// arrays by key regardless of order, objects by class and properties,
/// dates by instant, numbers by value (NAN equals NAN, as for PHPUnit's
/// float comparator), strings against scalars as strings, and `null` /
/// `bool` loosely.
pub fn phpunit_equals(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Array(e), Value::Array(a)) => {
            e.len() == a.len()
                && e.iter()
                    .all(|(k, v)| a.get(k).is_some_and(|w| phpunit_equals(v, w)))
        }
        (Value::Object(e), Value::Object(a)) => {
            if e.class() != a.class() {
                return false;
            }
            if let (Some(x), Some(y)) = (
                e.downcast_ref::<DateTimeImmutable>(),
                a.downcast_ref::<DateTimeImmutable>(),
            ) {
                return x.timestamp() == y.timestamp() && x.microsecond() == y.microsecond();
            }
            phpunit_equals(&Value::Array(e.properties()), &Value::Array(a.properties()))
        }
        (Value::Object(_), _)
        | (_, Value::Object(_))
        | (Value::Array(_), _)
        | (_, Value::Array(_)) => false,
        (Value::Null | Value::Bool(_), _) | (_, Value::Null | Value::Bool(_)) => {
            to_bool(expected) == to_bool(actual)
        }
        (Value::String(_), _) | (_, Value::String(_)) => {
            expected.to_php_string() == actual.to_php_string()
        }
        (e, a) => {
            let (x, y) = (number(e), number(a));
            (x.is_nan() && y.is_nan()) || x == y
        }
    }
}

fn number(value: &Value) -> f64 {
    match value {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        _ => f64::NAN,
    }
}

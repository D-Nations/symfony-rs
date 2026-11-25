//! PHP 8.4.27's behaviour recorded by `oracle/recorders/php_value.php`
//! (`scripts/record.sh php-value`).
//!
//! Upstream: none
#![allow(
    clippy::cast_possible_truncation,
    reason = "the tests build bytes and code points from known small values"
)]

use php_value::{
    Key, PhpArray, Value,
    json::{JsonFlags, encode_string},
    numeric::{
        Number, float_to_string, hexdec, is_numeric, json_float, octdec, string_to_float,
        string_to_int, var_export_float,
    },
    pcre::{is_space_u, is_zs_u},
};
use serde_json::Value as Json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn vectors() -> Result<Json, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(include_str!("data/php.json"))?)
}

fn rows<'a>(json: &'a Json, key: &str) -> Result<&'a Vec<Json>, Box<dyn std::error::Error>> {
    json[key]
        .as_array()
        .ok_or_else(|| format!("no {key}").into())
}

fn float(bits: &Json) -> Result<f64, Box<dyn std::error::Error>> {
    let hex = bits.as_str().ok_or("bits are not a string")?;
    Ok(f64::from_bits(u64::from_str_radix(hex, 16)?))
}

fn base64(text: &Json) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // A small decoder: the vectors are the only base64 these tests read.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let text = text.as_str().ok_or("not base64")?;
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
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

/// A recorded `[type, value]`.
fn typed(json: &Json) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(match json[0].as_str().ok_or("no type")? {
        "null" => Value::Null,
        "bool" => Value::Bool(json[1].as_bool().ok_or("not a bool")?),
        "int" => Value::Int(json[1].as_i64().ok_or("not an int")?),
        "float" => Value::Float(float(&json[1])?),
        "string" => Value::string(base64(&json[1])?),
        other => return Err(format!("unexpected type {other}").into()),
    })
}

fn number(value: Number) -> Value {
    match value {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

#[test]
fn array_keys_match_php() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "keys")? {
        let expected = typed(&row[1])?;
        let key = match &row[0] {
            Json::String(_) => Key::from_bytes(&base64(&row[0])?),
            other => match typed(other)? {
                Value::Bool(b) => Key::from(b),
                Value::Null => Key::from(""),
                v => return Err(format!("unexpected key input {v:?}").into()),
            },
        };
        assert_eq!(key.to_value(), expected, "key of {:?}", row[0]);
    }
    Ok(())
}

#[test]
fn string_casts_and_is_numeric_match_php() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "casts")? {
        let input = base64(&row[0])?;
        let shown = String::from_utf8_lossy(&input).into_owned();
        assert_eq!(
            Some(string_to_int(&input)),
            row[1].as_i64(),
            "(int) {shown:?}"
        );
        let expected = float(&row[2])?;
        assert_eq!(
            string_to_float(&input).to_bits(),
            expected.to_bits(),
            "(float) {shown:?}"
        );
        assert_eq!(
            Some(is_numeric(&input)),
            row[3].as_bool(),
            "is_numeric({shown:?})"
        );
    }
    Ok(())
}

fn build(label: &str) -> Result<PhpArray, php_value::NextElementOccupied> {
    let mut a = PhpArray::new();
    match label {
        "empty" => {}
        "after 5" => a.insert(5, "a".into()),
        "after -5" => a.insert(-5, "a".into()),
        "after -5 and 2" => {
            a.insert(-5, "a".into());
            a.insert(2, "b".into());
        }
        "after string key" => a.insert("k", "a".into()),
        "after unset of the last" => {
            a = php_value::php_array!["a", "b", "c"];
            a.remove(&Key::Int(2));
        }
        "after \"7\"" => a.insert("7", "a".into()),
        "after PHP_INT_MAX" => a.insert(i64::MAX, "a".into()),
        "overwrite keeps place" => {
            a.insert("a", 1.into());
            a.insert("b", 2.into());
            a.insert("a", 3.into());
            return Ok(a);
        }
        other => panic!("unknown scenario {other}"),
    }
    a.push("x".into())?;
    Ok(a)
}

#[test]
fn appending_follows_php_next_index() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "appends")? {
        let label = row[0].as_str().ok_or("no label")?;
        match (&row[1], build(label)) {
            (Json::Array(keys), Ok(array)) => {
                let expected = keys.iter().map(typed).collect::<Result<Vec<_>, _>>()?;
                let actual: Vec<Value> = array.keys().map(Key::to_value).collect();
                assert_eq!(actual, expected, "{label}");
            }
            (Json::String(message), Err(error)) => {
                assert_eq!(*message, format!("Error: {error}"), "{label}");
            }
            (expected, actual) => panic!("{label}: PHP {expected:?}, Rust {actual:?}"),
        }
    }
    Ok(())
}

#[test]
fn float_to_string_forms_match_php() -> TestResult {
    let json = vectors()?;
    let cases = rows(&json, "float_strings")?;
    for row in cases {
        let f = float(&row[0])?;
        assert_eq!(
            float_to_string(f),
            row[1].as_str().ok_or("no cast")?,
            "(string) {f:?}"
        );
        assert_eq!(
            var_export_float(f),
            row[2].as_str().ok_or("no export")?,
            "var_export({f:?})"
        );
        let json_form = row[3].as_str().map(str::to_owned);
        assert_eq!(json_float(f), json_form, "json_encode({f:?})");
    }
    assert!(cases.len() > 300);
    Ok(())
}

#[test]
fn hexdec_and_octdec_match_php() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "radix")? {
        let input = row[0].as_str().ok_or("no input")?;
        assert_eq!(
            number(hexdec(input.as_bytes())),
            typed(&row[1])?,
            "hexdec({input:?})"
        );
        assert_eq!(
            number(octdec(input.as_bytes())),
            typed(&row[2])?,
            "octdec({input:?})"
        );
    }
    Ok(())
}

#[test]
fn pcre_space_classes_match_php() -> TestResult {
    let json = vectors()?;
    for (key, class) in [
        ("pcre_space_u", is_space_u as fn(char) -> bool),
        ("pcre_zs_u", is_zs_u),
    ] {
        let expected: Vec<u32> = rows(&json, key)?
            .iter()
            .filter_map(Json::as_u64)
            .map(|c| c as u32)
            .collect();
        let actual: Vec<u32> = (0..=0x0010_FFFF_u32)
            .filter_map(char::from_u32)
            .filter(|&c| class(c))
            .map(u32::from)
            .collect();
        assert_eq!(actual, expected, "{key}");
    }
    Ok(())
}

#[test]
fn json_encode_of_strings_matches_php() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "json_strings")? {
        let input = base64(&row[0])?;
        for (i, (slashes, unicode)) in [(false, false), (true, false), (false, true), (true, true)]
            .into_iter()
            .enumerate()
        {
            let flags = JsonFlags {
                unescaped_slashes: slashes,
                unescaped_unicode: unicode,
            };
            let expected = row[i + 1].as_str().map(str::to_owned);
            assert_eq!(
                encode_string(&input, flags),
                expected,
                "{input:?} with {flags:?}"
            );
        }
    }
    Ok(())
}

fn recorder_classes() -> (
    php_value::serialize::Classes,
    php_value::constants::Registry,
) {
    use php_value::{constants::Registry, object::EnumCase, serialize::Classes};
    let mut classes = Classes::new();
    classes.allow_with_defaults(
        "RecorderAllowed",
        php_value::php_array!["a" => "foo", "b" => 1],
    );
    let mut constants = Registry::with_core();
    constants.define_enum(
        "RecorderSuit",
        vec![
            EnumCase::new("RecorderSuit", "Hearts", Some("H".into())),
            EnumCase::new("RecorderSuit", "Spades", Some("S".into())),
        ],
    );
    constants.define_enum(
        "RecorderUnit",
        vec![EnumCase::new("RecorderUnit", "One", None)],
    );
    (classes, constants)
}

#[test]
fn serialize_matches_php() -> TestResult {
    let json = vectors()?;
    for row in rows(&json, "serialize")? {
        let value = php_value::vectors::decode(&row[0]).map_err(|e| e.to_string())?;
        let expected = base64(&row[1])?;
        let actual = php_value::serialize::serialize(&value);
        assert_eq!(
            actual.as_bytes(),
            expected.as_slice(),
            "serialize({value:?})"
        );
    }
    Ok(())
}

#[test]
fn unserialize_matches_php_with_an_allow_list() -> TestResult {
    let json = vectors()?;
    let (classes, constants) = recorder_classes();
    for row in rows(&json, "unserialize")? {
        let input = base64(&row[0])?;
        let shown = String::from_utf8_lossy(&input).into_owned();
        let actual = php_value::serialize::unserialize(&input, &classes, &constants);
        if row[1][0] == "false" {
            assert!(
                actual.is_none(),
                "unserialize({shown:?}) should fail, got {actual:?}"
            );
            continue;
        }
        let expected = php_value::vectors::decode(&row[1]).map_err(|e| e.to_string())?;
        let actual = actual.ok_or_else(|| format!("unserialize({shown:?}) failed"))?;
        assert_eq!(actual.debug_type(), expected.debug_type(), "{shown:?}");
        assert!(
            php_value::vectors::phpunit_equals(&expected, &actual),
            "unserialize({shown:?}): {actual:?} vs {expected:?}"
        );
    }
    Ok(())
}

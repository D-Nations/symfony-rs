//! `json_encode()` and `json_decode()` to objects against PHP 8.4.27's
//! answers recorded by `oracle/recorders/php_value_json.php`
//! (`scripts/record.sh php-value-json`).
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{
    PhpArray, PhpObject, PhpString, Thrown, Value,
    json::{EncodeFailure, JSON_HEX_TAG, JsonError, decode_object, encode},
    object::StdClass,
    vectors,
};
use serde_json::Value as Json;

/// The recorder's `JsonSerializableValue`: `jsonSerialize()` returns `v`.
#[derive(Debug)]
struct SerializableValue(PhpArray);

impl PhpObject for SerializableValue {
    fn class(&self) -> &'static str {
        "JsonSerializableValue"
    }

    fn properties(&self) -> PhpArray {
        self.0.clone()
    }

    fn json_serialize(&self) -> Option<Result<Value, Thrown>> {
        Some(Ok(self.0.get(&"v".into()).cloned().unwrap_or_default()))
    }
}

/// The recorder's `JsonSerializableThrows`: `jsonSerialize()` throws
/// `\Exception($message)`.
#[derive(Debug)]
struct SerializableThrows(PhpArray);

impl PhpObject for SerializableThrows {
    fn class(&self) -> &'static str {
        "JsonSerializableThrows"
    }

    fn properties(&self) -> PhpArray {
        self.0.clone()
    }

    fn json_serialize(&self) -> Option<Result<Value, Thrown>> {
        let message = self
            .0
            .get(&"message".into())
            .and_then(Value::to_php_string)
            .unwrap_or_default();
        Some(Err(Thrown::new("Exception", message, 0)))
    }
}

/// The recorded value with the recorder's serializable classes in place.
fn prepare(value: Value) -> Value {
    let map = |array: &PhpArray| {
        let mut out = PhpArray::new();
        for (k, v) in array.iter() {
            out.insert(k.clone(), prepare(v.clone()));
        }
        out
    };
    match value {
        Value::Array(array) => Value::Array(map(&array)),
        Value::Object(object) => match object.class() {
            "JsonSerializableValue" => {
                Value::Object(Arc::new(SerializableValue(map(&object.properties()))))
            }
            "JsonSerializableThrows" => {
                Value::Object(Arc::new(SerializableThrows(object.properties())))
            }
            StdClass::CLASS => StdClass::object(map(&object.properties())),
            "ArrayObject" => Value::Object(Arc::new(php_value::object::ArrayObject::new(map(
                &object.properties(),
            )))),
            _ => Value::Object(object),
        },
        other => other,
    }
}

/// Reads the vector file, whose values nest past `serde_json`'s default
/// limit.
fn data() -> Result<Json, vectors::Error> {
    let mut reader = serde_json::Deserializer::from_str(include_str!("data/json_encode.json"));
    reader.disable_recursion_limit();
    match reader.into_iter::<Json>().next() {
        Some(Ok(data)) => Ok(data),
        Some(Err(e)) => Err(e.into()),
        None => Err("empty".into()),
    }
}

fn encode_cases(data: &Json) -> Result<(), vectors::Error> {
    let cases = data["encode"].as_array().ok_or("encode")?;
    assert!(cases.len() >= 15_000, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in cases {
        let value = prepare(vectors::decode(&case[0])?);
        let flags = case[1].as_i64().ok_or("flags")?;
        let depth = usize::try_from(case[2].as_i64().ok_or("depth")?)?;
        let got = encode(&value, flags, depth);
        let expected = &case[3];
        let ok = match (expected[0].as_str().ok_or("kind")?, &got) {
            ("json", Ok(encoded)) => {
                let text = vectors::base64(expected[1].as_str().ok_or("text")?)?;
                let code = expected[2].as_i64();
                encoded.json.as_deref().map(str::as_bytes) == Some(&text[..])
                    && encoded.error.map(JsonError::code) == code.filter(|&c| c != 0)
            }
            ("false", Ok(encoded)) => {
                let code = expected[1].as_i64().ok_or("code")?;
                let message = expected[2].as_str().ok_or("message")?;
                encoded.json.is_none()
                    && encoded
                        .error
                        .is_some_and(|e| e.code() == code && e.message() == message)
            }
            ("throws", Err(EncodeFailure::Json { error, previous })) => {
                expected[1] == "JsonException"
                    && expected[2].as_i64() == Some(error.code())
                    && expected[3].as_str() == Some(error.message())
                    && match previous {
                        None => expected[4].is_null(),
                        Some(thrown) => {
                            expected[4][0].as_str() == Some(thrown.class.as_str())
                                && expected[4][1].as_str().map(str::as_bytes)
                                    == Some(thrown.message.as_bytes())
                        }
                    }
            }
            ("throws", Err(EncodeFailure::Thrown(thrown))) => {
                expected[1].as_str() == Some(thrown.class.as_str())
                    && expected[2].as_i64() == Some(thrown.code)
                    && expected[3].as_str().map(str::as_bytes) == Some(thrown.message.as_bytes())
                    && expected[4].is_null()
            }
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "{:?} flags {flags} depth {depth}: PHP {}, Rust {:?}",
                value,
                expected.to_string().chars().take(120).collect::<String>(),
                got
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    Ok(())
}

/// `===` for decoded values, with floats compared by their bits and
/// objects by class and properties.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((k, v), (l, w))| k == l && same(v, w))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.class() == y.class()
                && same(&Value::Array(x.properties()), &Value::Array(y.properties()))
        }
        _ => a == b,
    }
}

fn decode_cases(data: &Json) -> Result<(), vectors::Error> {
    let cases = data["decode"].as_array().ok_or("decode")?;
    assert!(cases.len() >= 8_000, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in cases {
        let input = vectors::base64(case[0].as_str().ok_or("input")?)?;
        let bigint = case[1].as_bool().ok_or("flag")?;
        let got = decode_object(&input, 512, bigint);
        let result = &case[2];
        let ok = if result[0] == "ok" {
            let expected = vectors::decode(&result[1])?;
            matches!(&got, Ok(v) if same(v, &expected))
        } else {
            let code = result[1].as_i64().ok_or("code")?;
            let message = result[2].as_str().ok_or("message")?;
            matches!(got, Err(e) if e.code() == code && e.message() == message)
        };
        if !ok {
            failures.push(format!(
                "{:?} (bigint {bigint}): PHP {}, Rust {}",
                String::from_utf8_lossy(&input)
                    .chars()
                    .take(80)
                    .collect::<String>(),
                result.to_string().chars().take(80).collect::<String>(),
                format!("{got:?}").chars().take(80).collect::<String>()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    Ok(())
}

/// Runs a replay on a thread with room for the 513-level vectors.
fn deep(f: fn(&Json) -> Result<(), vectors::Error>) -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let data = data().map_err(|e| e.to_string())?;
            f(&data).map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the replay panicked".to_owned())?
}

#[test]
fn json_encode_matches_php() -> Result<(), String> {
    deep(encode_cases)
}

#[test]
fn json_decode_to_objects_matches_php() -> Result<(), String> {
    deep(decode_cases)
}

#[test]
fn json_encode_hex_tag() {
    let encoded = encode(&Value::string("<a>"), JSON_HEX_TAG, 512);
    assert_eq!(
        Ok(Some("\"\\u003Ca\\u003E\"".to_owned())),
        encoded.map(|e| e.json)
    );
    assert_eq!(
        PhpString::from("{}"),
        PhpString::from(
            encode(&StdClass::object(PhpArray::new()), 0, 512)
                .ok()
                .and_then(|e| e.json)
                .unwrap_or_default()
        )
    );
}

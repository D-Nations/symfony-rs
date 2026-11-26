//! Reading `tests/data/php.json` (`oracle/recorders/expression_language.php`):
//! values with the recorder's classes, outcomes, and PHP's `===`.
//!
//! Upstream: none

use std::sync::{Arc, OnceLock};

use php_value::{
    Key, PhpArray, PhpObject, PhpString, Value,
    constants::Registry,
    object::{ArrayObject, EnumCase, StdClass},
    vectors,
};
use serde_json::Value as Json;
use symfony_expression_language::{Error, ExpressionLanguage, MethodObject, php};

pub fn data() -> Json {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/php.json"))
        .expect("tests/data/php.json");
    serde_json::from_str(&text).expect("JSON")
}

pub fn b64(json: &Json) -> Vec<u8> {
    vectors::base64(json.as_str().expect("base64 text")).expect("base64")
}

pub fn text(json: &Json) -> String {
    String::from_utf8_lossy(&b64(json)).into_owned()
}

/// The recorder's `ElObj`: properties a, b, c and methods `name()`,
/// `nothing()`, `add($x, $y)` and `self()`.
pub fn el_obj(properties: &PhpArray) -> Value {
    let me: Arc<OnceLock<Value>> = Arc::new(OnceLock::new());
    let me2 = Arc::clone(&me);
    let mut object = MethodObject::new("ElObj")
        .method("name", |_| Ok(Value::from("el")))
        .method("nothing", |_| Ok(Value::Null))
        .method("add", |args| {
            php::add(
                args.first().unwrap_or(&Value::Null),
                args.get(1).unwrap_or(&Value::Null),
            )
        })
        .method("self", move |_| {
            Ok(me2.get().cloned().unwrap_or(Value::Null))
        });
    for (k, v) in properties.iter() {
        object = object.property(
            &k.bytes().iter().map(|&b| b as char).collect::<String>(),
            v.clone(),
        );
    }
    let value = object.into_value();
    let _ = me.set(value.clone());
    value
}

/// A recorded value; `ElObj` objects become `MethodObject`s.
pub fn decode(json: &Json) -> Value {
    match json[0].as_str().unwrap_or("") {
        "array" => {
            let mut array = PhpArray::new();
            for pair in json[1].as_array().expect("pairs") {
                let key = Key::from_value(&decode(&pair[0])).expect("key");
                array.insert(key, decode(&pair[1]));
            }
            Value::Array(array)
        }
        "object" => {
            let class = json[1].as_str().expect("class");
            let mut properties = PhpArray::new();
            for pair in json[2].as_array().expect("pairs") {
                let key = Key::from_value(&decode(&pair[0])).expect("key");
                properties.insert(key, decode(&pair[1]));
            }
            match class {
                "ElObj" => el_obj(&properties),
                "stdClass" => StdClass::object(properties),
                _ => Value::Object(Arc::new(php_value::object::GenericObject::new(
                    class, properties,
                ))),
            }
        }
        "arrayobject" => {
            let Value::Array(storage) = decode(&serde_json::json!(["array", json[1]])) else {
                unreachable!()
            };
            Value::Object(Arc::new(ArrayObject::new(storage)))
        }
        // the recorder's `Text` keeps its text in a property, which
        // comparisons read
        "stringable" => {
            let text = b64(&json[1]);
            MethodObject::new("Text")
                .property("text", PhpString::from(text.clone()))
                .stringable(text)
                .into_value()
        }
        _ => vectors::decode(json).expect("value"),
    }
}

/// PHP's `===` for recorded values: objects by class and properties
/// (enum cases by case), floats by bits or both NAN.
pub fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(x), Value::Float(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((k1, v1), (k2, v2))| k1 == k2 && same(v1, v2))
        }
        (Value::Object(x), Value::Object(y)) => {
            if let (Some(a), Some(b)) = (x.downcast_ref::<EnumCase>(), y.downcast_ref::<EnumCase>())
            {
                return a.class() == b.class() && a.name() == b.name();
            }
            x.class() == y.class()
                && x.to_php_string() == y.to_php_string()
                && same(&Value::Array(x.properties()), &Value::Array(y.properties()))
        }
        _ => expected.identical(actual),
    }
}

/// A recorded outcome against ours: `None` when they agree, else a
/// description.
pub fn check(expected: &Json, actual: &Result<Value, Error>) -> Option<String> {
    match (expected[0].as_str(), actual) {
        (Some("throws"), Err(e)) => {
            let class = expected[1].as_str().unwrap_or("");
            let message = without_file(&b64(&expected[2]));
            let ours = e.message().as_bytes();
            let message_ok = message == ours
                || (message.starts_with(ours)
                    && message[ours.len()..].starts_with(b", called in "));
            if e.class().name() == class && message_ok {
                None
            } else {
                Some(format!(
                    "PHP threw {class}: {}; we threw {e}",
                    String::from_utf8_lossy(&message)
                ))
            }
        }
        (Some("throws"), Ok(v)) => Some(format!(
            "PHP threw {}: {}; we returned {v:?}",
            expected[1].as_str().unwrap_or(""),
            text(&expected[2])
        )),
        (_, Err(e)) => Some(format!("PHP returned {expected}; we threw {e}")),
        (_, Ok(v)) => {
            let value = decode(expected);
            if same(&value, v) {
                None
            } else {
                Some(format!("PHP returned {value:?}; we returned {v:?}"))
            }
        }
    }
}

/// PHP's message without the " in FILE on line N" that names where a
/// closure was called from: the recorder's paths are not this port's.
fn without_file(message: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(message);
    let Some(start) = text.find(" passed in ") else {
        return message.to_vec();
    };
    let rest = &text[start + " passed".len()..];
    let Some(line) = rest.find(" on line ") else {
        return message.to_vec();
    };
    let after = &rest[line + " on line ".len()..];
    let digits = after.bytes().take_while(u8::is_ascii_digit).count();
    format!("{} passed{}", &text[..start], &after[digits..]).into_bytes()
}

/// A string outcome (compile, dump, `__toString()`) against ours.
pub fn check_text(expected: &Json, actual: &Result<PhpString, Error>) -> Option<String> {
    check(expected, &actual.clone().map(Value::String))
}

/// The constants and enums the recorder defines.
pub fn registry() -> Registry {
    let mut registry = Registry::with_core();
    registry.define("EL_CONST", Value::Int(42));
    registry.define("PHP_VERSION", Value::from("8.4.27"));
    registry.define_enum(
        "ElEnum",
        vec![
            EnumCase::new("ElEnum", "One", None),
            EnumCase::new("ElEnum", "Two", None),
        ],
    );
    registry.define_enum(
        "ElBacked",
        vec![EnumCase::new("ElBacked", "Bar", Some(Value::from("bar")))],
    );
    registry
}

pub fn language() -> ExpressionLanguage {
    ExpressionLanguage::with_constants(None, registry())
}

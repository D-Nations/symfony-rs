//! Shared helpers for the Finder tests: provider rows recorded from
//! upstream (tests/data/providers.json, `scripts/record.sh finder`) and
//! PHP vectors (tests/data/php.json).
//!
//! Upstream: none
#![allow(dead_code)]

pub mod fixture;

use php_value::{Value, vectors::decode};
use serde_json::Value as Json;

pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub fn providers_json() -> Json {
    serde_json::from_str(include_str!("../data/providers.json"))
        .unwrap_or_else(|e| panic!("providers.json: {e}"))
}

pub fn php_json() -> Json {
    serde_json::from_str(include_str!("../data/php.json"))
        .unwrap_or_else(|e| panic!("php.json: {e}"))
}

/// The provider rows of `class::test`: (row name, arguments).
pub fn provider(class: &str, test: &str) -> Vec<(String, Vec<Value>)> {
    let json = providers_json();
    json[class][test]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no rows for {class}::{test}"))
        .iter()
        .map(|row| {
            let name = row[0].as_str().unwrap_or_default().to_string();
            let args = row[1]
                .as_array()
                .map(|args| {
                    args.iter()
                        .map(|a| decode(a).unwrap_or_else(|e| panic!("{class}::{test}: {e}")))
                        .collect()
                })
                .unwrap_or_default();
            (name, args)
        })
        .collect()
}

/// The directory upstream's providers put fixtures under.
pub fn recorded_tmp_dir() -> String {
    providers_json()["tmpDir"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// A string argument.
pub fn bytes(v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Int(i) => i.to_string().into_bytes(),
        other => panic!("not a string: {other:?}"),
    }
}

pub fn string(v: &Value) -> String {
    String::from_utf8(bytes(v)).unwrap_or_else(|e| panic!("not UTF-8: {e}"))
}

/// A list of strings, or one string as a one-element list (`(array)`).
pub fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.values().map(string).collect(),
        Value::String(_) => vec![string(v)],
        other => panic!("not a list: {other:?}"),
    }
}

pub fn b64(json: &Json) -> Vec<u8> {
    php_value::vectors::base64(json.as_str().unwrap_or_default())
        .unwrap_or_else(|e| panic!("base64: {e}"))
}

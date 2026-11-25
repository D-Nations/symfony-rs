//! Reads tests/data/php.json (`scripts/record.sh doctrine-dbal`): the
//! upstream data providers, and the parser and expansion vectors.
//!
//! Upstream: none
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use doctrine_dbal::{ArrayParameterType, ParameterType, WrapperParameterType};
use php_value::vectors::{base64, decode};
use php_value::{Key, PhpArray, Value};
use serde_json::Value as Json;

pub fn data() -> &'static Json {
    static DATA: OnceLock<Json> = OnceLock::new();
    DATA.get_or_init(|| {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/php.json"))
                .expect("tests/data/php.json (scripts/record.sh doctrine-dbal)");
        serde_json::from_str(&text).expect("valid JSON")
    })
}

/// The rows of a data provider: (row name, typed JSON arguments).
pub fn provider(class: &str, method: &str) -> Vec<(String, Vec<Json>)> {
    let rows = data()["providers"][class][method]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no provider for {class}::{method}"));
    rows.iter()
        .map(|row| {
            let name = row[0].as_str().expect("row name").to_owned();
            let args = row[1].as_array().expect("arguments").clone();
            (name, args)
        })
        .collect()
}

pub fn bytes(json: &Json) -> Vec<u8> {
    assert_eq!(json[0], "string", "not a string: {json}");
    base64(json[1].as_str().expect("base64")).expect("base64")
}

pub fn bool(json: &Json) -> bool {
    assert_eq!(json[0], "bool", "not a bool: {json}");
    json[1].as_bool().expect("bool")
}

pub fn array(json: &Json) -> PhpArray {
    match decode(json).expect("typed value") {
        Value::Array(array) => array,
        other => panic!("not an array: {other:?}"),
    }
}

/// A recorded `string|ParameterType|ArrayParameterType`.
pub fn wrapper_type(json: &Json) -> WrapperParameterType {
    match json[0].as_str() {
        Some("string") => WrapperParameterType::Name(bytes(json).into()),
        Some("enum") => {
            let case = json[2].as_str().expect("case");
            match json[1].as_str() {
                Some("Doctrine\\DBAL\\ParameterType") => {
                    WrapperParameterType::Parameter(ParameterType::from_name(case).expect("case"))
                }
                Some("Doctrine\\DBAL\\ArrayParameterType") => {
                    WrapperParameterType::Array(ArrayParameterType::from_name(case).expect("case"))
                }
                other => panic!("unknown enum {other:?}"),
            }
        }
        _ => panic!("not a parameter type: {json}"),
    }
}

/// A recorded array of types, by key.
pub fn types(json: &Json) -> BTreeMap<Key, WrapperParameterType> {
    assert_eq!(json[0], "array", "not an array: {json}");
    json[1]
        .as_array()
        .expect("pairs")
        .iter()
        .map(|pair| {
            let key = match decode(&pair[0]).expect("key") {
                Value::Int(i) => Key::Int(i),
                Value::String(s) => Key::Str(s),
                other => panic!("bad key {other:?}"),
            };
            (key, wrapper_type(&pair[1]))
        })
        .collect()
}

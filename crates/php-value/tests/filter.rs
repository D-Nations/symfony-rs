//! `filter_var()` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/filter_var.php` (`scripts/record.sh php-value`).
//!
//! Upstream: none

use php_value::{
    Key, PhpArray, PhpString, Value,
    filter::{CONSTANTS, filter_var},
    object::Closure,
    vectors::{self, Outcome},
};
use serde_json::Value as Json;

type TestResult = Result<(), vectors::Error>;

fn data() -> Result<Json, vectors::Error> {
    Ok(serde_json::from_str(include_str!("data/filter.json"))?)
}

/// The recorder's named callbacks.
fn callback(name: &str) -> Result<Value, vectors::Error> {
    Ok(match name {
        "echo" => Closure::object(|args| {
            let mut array = PhpArray::new();
            array.insert("arg", args.first().cloned().unwrap_or_default());
            Value::Array(array)
        }),
        "int" => Closure::object(|args| {
            Value::Int(php_value::numeric::string_to_int(
                args.first()
                    .and_then(Value::to_php_string)
                    .unwrap_or_default()
                    .as_bytes(),
            ))
        }),
        "null" => Closure::object(|_| Value::Null),
        "false" => Closure::object(|_| Value::Bool(false)),
        "upper" => Closure::object(|args| {
            let text = args
                .first()
                .and_then(Value::to_php_string)
                .unwrap_or_default();
            Value::String(PhpString::from(text.as_bytes().to_ascii_uppercase()))
        }),
        other => return Err(format!("unknown callback {other}").into()),
    })
}

/// The options as recorded: arrays of typed values, callbacks by name.
fn options(json: &Json) -> Result<Value, vectors::Error> {
    match json[0].as_str() {
        Some("callback") => callback(json[1].as_str().ok_or("callback name")?),
        Some("array") => {
            let mut array = PhpArray::new();
            for pair in json[1].as_array().ok_or("pairs")? {
                let key = Key::from_value(&vectors::decode(&pair[0])?).ok_or("key")?;
                array.insert(key, options(&pair[1])?);
            }
            Ok(Value::Array(array))
        }
        _ => Ok(vectors::decode(json)?),
    }
}

#[test]
fn constants_are_php_s() -> TestResult {
    let data = data()?;
    let recorded: Vec<(String, i64)> = data["constants"]
        .as_array()
        .ok_or("constants")?
        .iter()
        .filter_map(|pair| Some((pair[0].as_str()?.to_owned(), pair[1].as_i64()?)))
        .filter(|(name, _)| !name.starts_with("INPUT_"))
        .collect();
    let ours: Vec<(String, i64)> = CONSTANTS
        .iter()
        .map(|(name, value)| ((*name).to_owned(), *value))
        .collect();
    assert_eq!(recorded, ours);
    Ok(())
}

#[test]
fn filter_var_matches_php() -> TestResult {
    let data = data()?;
    let cases = data["cases"].as_array().ok_or("cases")?;
    let mut mismatches = Vec::new();
    let mut unsupported = Vec::new();
    for case in cases {
        let value = vectors::decode(&case[0])?;
        let filter = case[1].as_i64().ok_or("filter")?;
        let options = options(&case[2])?;
        let Outcome::Value(expected) = vectors::outcome(&case[3][1])? else {
            return Err("a recorded case threw".into());
        };
        match filter_var(&value, filter, &options) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => mismatches.push(format!(
                "{value:?} {filter} {options:?}: PHP {expected:?}, Rust {actual:?}"
            )),
            Err(e) => unsupported.push(format!("{value:?} {filter} {options:?}: {e}")),
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} differ:\n{}",
        mismatches.len(),
        cases.len(),
        mismatches[..mismatches.len().min(40)].join("\n")
    );
    assert!(
        unsupported.is_empty(),
        "{} refused:\n{}",
        unsupported.len(),
        unsupported[..unsupported.len().min(40)].join("\n")
    );
    Ok(())
}

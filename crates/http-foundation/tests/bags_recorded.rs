//! `ParameterBag` and `InputBag` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/http_foundation_bags.php` (`scripts/record.sh
//! http-foundation`): every getter, `all()`, `getEnum()` with int- and
//! string-backed enums, and `filter()`, for 53 stored values and as
//! many defaults. PHP's notices (`E_DEPRECATED` for a float truncated by
//! `getEnum()`) are recorded but not reproduced.
//!
//! Upstream: none

use php_value::{
    Object, PhpString, Value,
    constants::Registry,
    object::EnumCase,
    vectors::{self, Error},
};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::Error as BagError, input_bag::InputBag, parameter_bag::ParameterBag,
};

fn registry() -> Registry {
    let mut registry = Registry::empty();
    registry.define_enum(
        "IntBacked",
        vec![
            EnumCase::new("IntBacked", "One", Some(Value::Int(1))),
            EnumCase::new("IntBacked", "Minus", Some(Value::Int(-5))),
            EnumCase::new("IntBacked", "Big", Some(Value::Int(i64::MAX))),
        ],
    );
    registry.define_enum(
        "StringBacked",
        vec![
            EnumCase::new("StringBacked", "A", Some(Value::from("a"))),
            EnumCase::new("StringBacked", "One", Some(Value::from("1"))),
            EnumCase::new("StringBacked", "Empty", Some(Value::from(""))),
        ],
    );
    registry.define_enum("Unit", vec![EnumCase::new("Unit", "Only", None)]);
    registry
}

fn text(json: &Json) -> Result<&str, Error> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn string_arg(args: &[Value], i: usize) -> PhpString {
    args.get(i)
        .and_then(Value::to_php_string)
        .unwrap_or_default()
}

/// `===` for recorded values, with objects (rebuilt from the recording)
/// compared by class, properties and string form, and floats by bits.
fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.class() == b.class()
                && a.to_php_string() == b.to_php_string()
                && same(&Value::Array(a.properties()), &Value::Array(b.properties()))
        }
        _ => expected == actual,
    }
}

/// The call the case records, on either bag.
fn call(
    class: &str,
    method: &str,
    stored: &Value,
    args: &[Value],
    key: &str,
    enums: &Registry,
) -> Result<Result<Value, BagError>, Error> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let int_arg = |i: usize| match args.get(i) {
        Some(Value::Int(i)) => *i,
        _ => 0,
    };
    let bool_arg = |i: usize| matches!(args.get(i), Some(Value::Bool(true)));
    let default_case =
        |i: usize| -> Option<Object> { args.get(i).and_then(Value::as_object).cloned() };
    let filter_id = || match args.get(1) {
        Some(Value::Int(i)) => *i,
        _ => php_value::filter::FILTER_DEFAULT,
    };
    let options = || {
        args.get(2)
            .cloned()
            .unwrap_or(Value::Array(php_value::PhpArray::new()))
    };
    let string = |r: Result<PhpString, BagError>| r.map(Value::String);
    Ok(if class.ends_with("\\ParameterBag") {
        let mut bag = ParameterBag::default();
        bag.set("k", stored.clone());
        match method {
            "get" => Ok(bag.get(key, arg(0))),
            "getString" => string(bag.get_string(key, string_arg(args, 0))),
            "getAlpha" => string(bag.get_alpha(key, string_arg(args, 0))),
            "getAlnum" => string(bag.get_alnum(key, string_arg(args, 0))),
            "getDigits" => string(bag.get_digits(key, string_arg(args, 0))),
            "getInt" => bag.get_int(key, int_arg(0)).map(Value::Int),
            "getBoolean" => bag.get_boolean(key, bool_arg(0)).map(Value::Bool),
            "all" => bag.all_of(key).map(Value::Array),
            "getEnum" => bag
                .get_enum(
                    key,
                    enums,
                    &string_arg(args, 0).to_string_lossy(),
                    default_case(1),
                )
                .map(|o| o.map_or(Value::Null, Value::Object)),
            "filter" => bag.filter(key, arg(0), filter_id(), &options()),
            other => return Err(format!("unknown method {other}").into()),
        }
    } else {
        let mut bag = InputBag::default();
        if let Err(error) = bag.set("k", stored.clone()) {
            return Ok(Err(error));
        }
        match method {
            "set" => Ok(Value::Null),
            "get" => bag.get(key, arg(0)),
            "getString" => string(bag.get_string(key, string_arg(args, 0))),
            "getAlpha" => string(bag.get_alpha(key, string_arg(args, 0))),
            "getAlnum" => string(bag.get_alnum(key, string_arg(args, 0))),
            "getDigits" => string(bag.get_digits(key, string_arg(args, 0))),
            "getInt" => bag.get_int(key, int_arg(0)).map(Value::Int),
            "getBoolean" => bag.get_boolean(key, bool_arg(0)).map(Value::Bool),
            "all" => bag.all_of(key).map(Value::Array),
            "getEnum" => bag
                .get_enum(
                    key,
                    enums,
                    &string_arg(args, 0).to_string_lossy(),
                    default_case(1),
                )
                .map(|o| o.map_or(Value::Null, Value::Object)),
            "filter" => bag.filter(key, arg(0), filter_id(), &options()),
            other => return Err(format!("unknown method {other}").into()),
        }
    })
}

#[test]
fn bags_match_php() -> Result<(), Error> {
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/bags.json"))?;
    let enums = registry();
    let mut failures = Vec::new();
    for case in &cases {
        let class = text(&case[0])?;
        let method = text(&case[1])?;
        let stored = vectors::decode(&case[2])?;
        let args = case[3]
            .as_array()
            .ok_or("args")?
            .iter()
            .map(vectors::decode)
            .collect::<Result<Vec<_>, _>>()?;
        let key = if case[6].as_bool() == Some(true) {
            "missing"
        } else {
            "k"
        };
        let actual = call(class, method, &stored, &args, key, &enums)?;
        let outcome = &case[4];
        let ok = match (outcome[0].as_str(), &actual) {
            (Some("value"), Ok(value)) => same(&vectors::decode(&outcome[1])?, value),
            (Some("throws"), Err(error)) => {
                error.class().name() == text(&outcome[1])?
                    && error.message().as_bytes() == vectors::base64(text(&outcome[2])?)?.as_slice()
            }
            _ => false,
        };
        if !ok {
            let expected = match outcome[0].as_str() {
                Some("value") => format!("{:?}", vectors::decode(&outcome[1])?),
                _ => format!(
                    "{} {:?}",
                    text(&outcome[1])?,
                    String::from_utf8_lossy(&vectors::base64(text(&outcome[2])?)?)
                ),
            };
            failures.push(format!(
                "{}::{method}({key}) stored {stored:?} args {args:?}: PHP {expected}, Rust {actual:?}",
                class.rsplit('\\').next().unwrap_or(class)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    Ok(())
}

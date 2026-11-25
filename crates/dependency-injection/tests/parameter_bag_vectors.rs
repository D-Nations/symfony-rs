//! `ParameterBag` against the real one: `resolveValue()`,
//! `resolveString()`, `get()` (alternatives, the nested-array hint) and
//! `resolve()` on the edge cases of `oracle/recorders/di_parameter_bag.php`
//! (`scripts/record.sh di-parameter-bag`, `data/parameter_bag.json`). The
//! value, or the exception's class and message, must match.
//!
//! Parameter names are UTF-8 `&str`s in this crate, so a `get()` whose
//! name is not UTF-8 cannot be called and is skipped (counted below).
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::expect_used)]

use php_value::Value;
use php_value::vectors::{base64, decode};
use serde_json::Value as Json;
use symfony_dependency_injection::{ParameterBag, Result};

fn run(bags: &[Json], case: &Json) -> Option<Result<Value>> {
    let bag = usize::try_from(case["bag"].as_u64().unwrap()).unwrap();
    let Value::Array(parameters) = decode(&bags[bag]).unwrap() else {
        panic!("parameters are not an array");
    };
    let arg = decode(&case["arg"]).unwrap();
    let mut bag = ParameterBag::new(&parameters).unwrap();
    let op = case["op"].as_str().unwrap();
    if case["resolved"].as_bool().unwrap()
        && let Err(e) = bag.resolve()
    {
        return Some(Err(e));
    }
    Some(match op {
        "resolveValue" => bag.resolve_value(&arg, &[]),
        "resolveString" => bag.resolve_string(arg.as_string().unwrap(), &[]),
        "get" => {
            let name = std::str::from_utf8(arg.as_string().unwrap().as_bytes()).ok()?;
            bag.get(name)
        }
        "resolve" => bag.resolve().map(|()| Value::Array(bag.all().clone())),
        other => panic!("unknown op {other}"),
    })
}

#[test]
fn parameter_bag_matches_php() {
    let data: Json =
        serde_json::from_str(include_str!("data/parameter_bag.json")).expect("parameter_bag.json");
    let bags = data["bags"].as_array().unwrap();
    let data = data["cases"].as_array().unwrap();
    assert!(data.len() > 500, "{} cases", data.len());
    let mut failures = Vec::new();
    let mut skipped = 0;
    for (index, case) in data.iter().enumerate() {
        let Some(actual) = run(bags, case) else {
            skipped += 1;
            continue;
        };
        let expected = &case["result"];
        let label = format!(
            "#{index} {} {:?} (resolved: {})",
            case["op"], case["arg"], case["resolved"]
        );
        if expected[0] == "throws" {
            let class = expected[1].as_str().unwrap();
            let message = base64(expected[2].as_str().unwrap()).unwrap();
            let message = String::from_utf8_lossy(&message);
            match actual {
                Ok(value) => {
                    failures.push(format!("{label}: {value:?}, PHP threw {class}: {message}"));
                }
                Err(e) if e.class() != class || e.message() != message => failures.push(format!(
                    "{label}: {}: {}\n   PHP: {class}: {message}",
                    e.class(),
                    e.message()
                )),
                Err(_) => {}
            }
        } else {
            let value = decode(expected).unwrap();
            match actual {
                Ok(actual) if actual == value => {}
                Ok(actual) => failures.push(format!("{label}: {actual:?}\n   PHP: {value:?}")),
                Err(e) => failures.push(format!(
                    "{label}: {}: {}\n   PHP: {value:?}",
                    e.class(),
                    e.message()
                )),
            }
        }
    }
    assert!(skipped <= 2, "{skipped} skipped");
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        data.len(),
        failures.join("\n")
    );
}

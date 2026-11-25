//! `EnvVarProcessor` against the real one: every prefix on the inputs of
//! `oracle/recorders/di_env_var_processor.php` (`scripts/record.sh
//! di-env-var-processor`, `data/env_var_processor.json`), with the
//! container parameters recorded there. For each call, the names the
//! `$getEnv` closure was asked for, and the value (floats to the bit) or
//! the exception's class and message, must match. The `lookups` read
//! `$_ENV`, `$_SERVER` and `getenv()` as set for each case.
//!
//! `require:` refuses where PHP would run the file; no recorded case
//! reaches that point (the files do not exist). `shuffle:` is random in
//! both, so its values are compared as a multiset. A `resolve:` value
//! whose placeholder names are not UTF-8 is skipped: the name reaches the
//! container through `ContainerInterface::get_parameter(&str)`, lossily,
//! so PHP's suggestions for the raw bytes differ (counted below).
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex, RwLock};

use php_value::vectors::{base64, decode};
use php_value::{PhpArray, Value};
use serde_json::Value as Json;
use symfony_dependency_injection::{
    ContainerInterface, EnvVarProcessor, EnvVarProcessorInterface, Error, ErrorKind,
    InvalidBehavior, ParameterBag, Result,
};
use symfony_dotenv::Globals;

/// `new Container(new FrozenParameterBag($params))`.
struct TestContainer(ParameterBag);

impl ContainerInterface for TestContainer {
    fn set(&self, _id: &str, _service: Option<Value>) -> Result<()> {
        Ok(())
    }

    fn get(&self, id: &str, _invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        Err(Error::service_not_found(id, None, Vec::new(), None))
    }

    fn has(&self, _id: &str) -> bool {
        false
    }

    fn initialized(&self, _id: &str) -> bool {
        false
    }

    fn get_parameter(&self, name: &str) -> Result<Value> {
        self.0.get(name)
    }

    fn has_parameter(&self, name: &str) -> bool {
        self.0.has(name)
    }

    fn set_parameter(&self, _name: &str, _value: Value) -> Result<()> {
        Err(Error::logic("frozen"))
    }
}

fn array(json: &Json) -> PhpArray {
    match decode(json).unwrap() {
        Value::Array(array) => array,
        other => panic!("not an array: {other:?}"),
    }
}

/// `===`, with floats compared to the bit (`-0.0` is not `0.0`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => {
            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
        }
        _ => a == b,
    }
}

/// The values of a list, sorted by their debug form.
fn sorted(value: &Value) -> Vec<String> {
    let mut values: Vec<String> = value
        .as_array()
        .map(|a| a.values().map(|v| format!("{v:?}")).collect())
        .unwrap_or_default();
    values.sort();
    values
}

fn check(label: &str, actual: Result<Value>, expected: &Json, failures: &mut Vec<String>) {
    if expected[0] == "throws" {
        let class = expected[1].as_str().unwrap();
        let message = base64(expected[2].as_str().unwrap()).unwrap();
        let message = String::from_utf8_lossy(&message);
        match actual {
            Ok(value) => failures.push(format!(
                "{label}: {value:?}\n   PHP threw {class}: {message}"
            )),
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
            Ok(actual) if same(&actual, &value) => {}
            Ok(actual) => failures.push(format!("{label}: {actual:?}\n   PHP: {value:?}")),
            Err(e) => failures.push(format!(
                "{label}: {}: {}\n   PHP: {value:?}",
                e.class(),
                e.message()
            )),
        }
    }
}

#[test]
fn env_var_processor_matches_php() {
    let data: Json = serde_json::from_str(include_str!("data/env_var_processor.json"))
        .expect("env_var_processor.json");
    let params = array(&data["params"]);
    let container: Arc<dyn ContainerInterface> = Arc::new(TestContainer(ParameterBag::frozen(
        params,
        Vec::new(),
        Vec::new(),
    )));
    let cases = data["cases"].as_array().unwrap();
    assert!(cases.len() > 900, "{} cases", cases.len());
    let mut failures = Vec::new();
    let mut skipped = 0;

    for (index, case) in cases.iter().enumerate() {
        let prefix = case["prefix"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let value = decode(&case["value"]).unwrap();
        if prefix == "resolve" && value.as_string().is_some_and(|s| s.to_str().is_none()) {
            skipped += 1;
            continue;
        }
        let label = format!("#{index} {prefix}:{name} {value:?}");
        let processor = EnvVarProcessor::new(container.clone(), None)
            .with_globals(Arc::new(RwLock::new(Globals::new())));
        let calls = Mutex::new(Vec::<String>::new());
        let actual = processor.get_env(prefix, name, &|n| {
            calls.lock().unwrap().push(n.to_owned());
            Ok(value.clone())
        });
        let expected_calls: Vec<String> = case["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().to_owned())
            .collect();
        if *calls.lock().unwrap() != expected_calls {
            failures.push(format!(
                "{label}: calls {:?}, PHP {expected_calls:?}",
                calls.lock().unwrap()
            ));
        }
        if prefix == "shuffle" && case["result"][0] == "array" {
            let expected = decode(&case["result"]).unwrap();
            match actual {
                Ok(actual)
                    if actual.as_array().is_some_and(PhpArray::is_list)
                        && sorted(&actual) == sorted(&expected) => {}
                other => failures.push(format!("{label}: {other:?}\n   PHP: {expected:?}")),
            }
            continue;
        }
        check(&label, actual, &case["result"], &mut failures);
    }
    assert!(skipped <= 2, "{skipped} skipped");

    for (index, case) in data["lookups"].as_array().unwrap().iter().enumerate() {
        let prefix = case["prefix"].as_str().unwrap();
        let name = case["name"].as_str().unwrap();
        let mut globals = Globals::new();
        globals.env = array(&case["env"]);
        globals.server = array(&case["server"]);
        for setting in case["putenv"].as_array().unwrap() {
            globals.putenv(setting.as_str().unwrap().as_bytes());
        }
        let label = format!(
            "lookup #{index} {prefix:?} {name} env {:?} server {:?} putenv {}",
            globals.env, globals.server, case["putenv"]
        );
        let processor = EnvVarProcessor::new(container.clone(), None)
            .with_globals(Arc::new(RwLock::new(globals)));
        let actual = processor.get_env(prefix, name, &|_| {
            Err(Error::new(
                ErrorKind::Other("LogicException".to_owned()),
                "getEnv called",
            ))
        });
        check(&label, actual, &case["result"], &mut failures);
    }

    assert!(
        failures.is_empty(),
        "{} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

//! Replays the parser and dumper calls recorded from the upstream tests
//! (`oracle/recorders/yaml_test_calls.php`, `scripts/record.sh yaml`).
//!
//! A recording maps each upstream test method to its rows (one, or one
//! per data-provider row). A row lists every call the test made to the
//! parser or the dumper (constructor arguments, input, other arguments,
//! and what PHP returned or threw) and the exception the test expects.
//! The recorder keeps a recording only when the upstream test passes on
//! the oracle, so what PHP returned is what the upstream assertions
//! accept. Replaying a row makes the same calls on the Rust port, which
//! must return exactly what PHP returned or fail with PHP's exception
//! class and message; then the expected exception (`expectException`,
//! `expectExceptionMessage`, `expectExceptionMessageMatches`) is checked
//! against the last call, as PHPUnit checks it.
//!
//! `{tests}` in the recordings is this tests directory (upstream's
//! Tests/Fixtures are in tests/Fixtures) and `{suite}` is
//! tests/yaml-test-suite.
//!
//! Upstream: none
#![allow(
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::map_unwrap_or,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use std::sync::{Arc, LazyLock};

use php_value::{
    Key, PhpArray, Value,
    datetime::DateTimeImmutable,
    object::{EnumCase, StdClass},
    php_array,
    vectors::decode,
};
use serde_json::Value as Json;
use symfony_yaml::{Dumper, Environment, Flags, Parser, TaggedValue};

pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const A: &str = r"Symfony\Component\Yaml\Tests\A";
const B: &str = r"Symfony\Component\Yaml\Tests\B";
const FOO_UNIT_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooUnitEnum";
const FOO_BACKED_ENUM: &str = r"Symfony\Component\Yaml\Tests\Fixtures\FooBackedEnum";
const PARSE_EXCEPTION: &str = r"Symfony\Component\Yaml\Exception\ParseException";

/// The environment of the upstream tests: PHP's constants, the Yaml
/// class constants, the test classes `A` (DumperTest.php) and `B`
/// (ParserTest.php) with their constants and defaults, which
/// `unserialize()` may create, and the two fixture enums.
pub fn environment() -> Arc<Environment> {
    static ENVIRONMENT: LazyLock<Arc<Environment>> = LazyLock::new(|| {
        let mut environment = Environment::new();
        for (name, value) in [("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")] {
            environment
                .constants
                .define(format!("{B}::{name}"), Value::from(value));
        }
        environment
            .classes
            .allow_with_defaults(A, php_array!["a" => "foo"])
            .allow_with_defaults(B, php_array!["b" => "foo"]);
        environment.constants.define_enum(
            FOO_UNIT_ENUM,
            vec![EnumCase::new(FOO_UNIT_ENUM, "BAR", None)],
        );
        environment.constants.define_enum(
            FOO_BACKED_ENUM,
            vec![EnumCase::new(FOO_BACKED_ENUM, "BAR", Some("bar".into()))],
        );
        Arc::new(environment)
    });
    ENVIRONMENT.clone()
}

/// Reads a recording. The nesting-depth tests record values nested past
/// `serde_json`'s default limit.
pub fn load(text: &str) -> Json {
    let mut reader = serde_json::Deserializer::from_str(text);
    reader.disable_recursion_limit();
    match reader.into_iter::<Json>().next() {
        Some(Ok(data)) => data,
        other => panic!("bad recording: {other:?}"),
    }
}

fn dir(name: &str) -> String {
    format!("{}/tests{name}", env!("CARGO_MANIFEST_DIR")).replace('\\', "/")
}

/// `{tests}` and `{suite}` replaced by their directories here.
pub fn localize(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    if text.contains("{tests}") || text.contains("{suite}") {
        return text
            .replace("{suite}", &dir("/yaml-test-suite"))
            .replace("{tests}", &dir(""))
            .into_bytes();
    }
    bytes.to_vec()
}

/// A recorded value as the Rust port has it: `{tests}` replaced in its
/// strings, and recorded `TaggedValue` objects as real ones.
pub fn realize(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(localize(&s).as_slice().into()),
        Value::Array(array) => Value::Array(realize_array(&array)),
        Value::Object(object) if object.class() == TaggedValue::CLASS => {
            let properties = object.properties();
            let tag = properties
                .get(&Key::from("tag"))
                .and_then(Value::as_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let inner = properties
                .get(&Key::from("value"))
                .cloned()
                .unwrap_or(Value::Null);
            Value::Object(Arc::new(TaggedValue::new(tag, realize(inner))))
        }
        Value::Object(object) if object.class() == StdClass::CLASS => {
            StdClass::object(realize_array(&object.properties()))
        }
        other => other,
    }
}

fn realize_array(array: &PhpArray) -> PhpArray {
    array
        .iter()
        .map(|(k, v)| (k.clone(), realize(v.clone())))
        .collect()
}

/// The same value: types, order and contents, objects by class and
/// properties, dates by their formatted value and time zone.
pub fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(a), Value::Float(b)) => {
            a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
        }
        (Value::Array(a), Value::Array(b)) => same_array(a, b),
        (Value::Object(a), Value::Object(b)) => {
            if a.class() != b.class() {
                return false;
            }
            if let (Some(x), Some(y)) = (
                a.downcast_ref::<DateTimeImmutable>(),
                b.downcast_ref::<DateTimeImmutable>(),
            ) {
                let format = r"Y-m-d\TH:i:s.uP e";
                return x.format(format) == y.format(format);
            }
            same_array(&a.properties(), &b.properties())
        }
        (a, b) => a.identical(b),
    }
}

fn same_array(a: &PhpArray, b: &PhpArray) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
}

fn text(json: &Json) -> &str {
    json.as_str().unwrap_or_default()
}

fn int(json: &Json) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    json.as_i64()
        .ok_or_else(|| format!("expected an integer, got {json}").into())
}

/// What a call threw: the PHP class and the message.
type Thrown = (String, Vec<u8>);

/// One recorded call on the Rust port. `build` gives the inputs too large
/// to record.
fn call(
    call: &Json,
    index: usize,
    label: &str,
    build: &dyn Fn(usize) -> Value,
) -> Result<Result<Value, Thrown>, Box<dyn std::error::Error + Send + Sync>> {
    let input = match call["input"][0].as_str() {
        None => Value::Null,
        Some("omitted") => {
            let built = build(index);
            if let (Some(length), Value::String(s)) = (call["input"][1].as_u64(), &built) {
                assert_eq!(
                    s.len() as u64,
                    length,
                    "{label}: the built input differs from upstream's"
                );
            }
            built
        }
        Some(_) => realize(decode(&call["input"])?),
    };
    let bytes = || {
        input
            .as_string()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default()
    };
    let flags = || Flags(call["flags"].as_u64().unwrap_or(0) as u32);
    let limits = match &call["limits"] {
        Json::Array(limits) if limits.len() == 2 => (int(&limits[0])?, int(&limits[1])?),
        _ => (128, 128),
    };
    let parser = || Parser::with_environment(environment()).with_limits(limits.0, limits.1);
    let parse_error =
        |e: symfony_yaml::ParseException| (PARSE_EXCEPTION.to_owned(), e.get_message().to_vec());
    let invalid = |e: symfony_yaml::InvalidArgumentException| {
        (
            "InvalidArgumentException".to_owned(),
            e.message.into_bytes(),
        )
    };
    Ok(match text(&call["method"]) {
        "parse" => parser()?.parse(&bytes(), flags()).map_err(parse_error),
        "parseFile" if call.get("contents").is_some() => {
            // a file the upstream test wrote: written here under the same
            // name, with the recorded path in the messages
            let recorded = String::from_utf8_lossy(&bytes()).into_owned();
            let name = recorded.rsplit('/').next().unwrap_or("yaml");
            let path = std::env::temp_dir().join(format!("symfony-yaml-{name}"));
            let contents = decode(&call["contents"])?;
            std::fs::write(
                &path,
                contents
                    .as_string()
                    .map(php_value::PhpString::as_bytes)
                    .unwrap_or(b""),
            )?;
            let local = path.to_string_lossy().replace('\\', "/");
            let result = parser()?.parse_file(&local, flags());
            std::fs::remove_file(&path)?;
            result.map_err(|e| {
                let message = e.get_message().to_string_lossy().replace(&local, &recorded);
                (PARSE_EXCEPTION.to_owned(), message.into_bytes())
            })
        }
        "parseFile" => parser()?
            .parse_file(&String::from_utf8_lossy(&bytes()), flags())
            .map_err(parse_error),
        "new Parser" => parser().map(|_| Value::Null).map_err(invalid),
        "new Dumper" => Dumper::with_environment(environment(), int(&call["indentation"])?)
            .map(|_| Value::Null)
            .map_err(invalid),
        "dump" => Dumper::with_environment(environment(), int(&call["indentation"])?)?
            .dump(
                &input,
                int(&call["inline"])?,
                usize::try_from(int(&call["indent"])?)?,
                flags(),
            )
            .map(Value::String)
            .map_err(|e| (e.class().to_owned(), e.message().as_bytes().to_vec())),
        other => return Err(format!("unknown method {other}").into()),
    })
}

/// Replays the recorded calls of `test`.
pub fn replay(recording: &Json, test: &str) -> TestResult {
    replay_with(recording, test, &|index| {
        panic!("{test}: call {index} has no recorded input")
    })
}

/// Replays the recorded calls of `test`; `build` gives the inputs that
/// were too large to record, by call index within the row.
pub fn replay_with(recording: &Json, test: &str, build: &dyn Fn(usize) -> Value) -> TestResult {
    let rows = recording[test]
        .as_array()
        .ok_or_else(|| format!("no recording for {test}"))?;
    for row in rows {
        let label = match row["row"].as_str() {
            Some(name) => format!("{test} [{name}]"),
            None => test.to_owned(),
        };
        if row["status"] == "skipped" {
            continue;
        }
        assert_eq!(row["status"], "pass", "{label}: not passing upstream");
        let mut last = None;
        for (index, recorded_call) in row["calls"]
            .as_array()
            .ok_or("no calls")?
            .iter()
            .enumerate()
        {
            let result = call(recorded_call, index, &label, build)?;
            let input = &recorded_call["input"];
            let what = format!(
                "{label}, call {index} ({}): {input}",
                text(&recorded_call["method"])
            );
            let recorded = &recorded_call["result"];
            match (recorded[0].as_str(), &result) {
                (Some("omitted"), _) => {}
                (Some("throws"), Err((class, message))) => {
                    assert_eq!(text(&recorded[1]), class, "{what}");
                    assert_eq!(
                        String::from_utf8_lossy(&localize(text(&recorded[2]).as_bytes())),
                        String::from_utf8_lossy(message),
                        "{what}"
                    );
                }
                (Some("throws"), Ok(value)) => panic!(
                    "{what}: expected {} {:?}, got {value:?}",
                    text(&recorded[1]),
                    text(&recorded[2])
                ),
                (_, Err((class, message))) => panic!(
                    "{what}: expected {recorded}, got {class}: {}",
                    String::from_utf8_lossy(message)
                ),
                (_, Ok(value)) => {
                    let expected = realize(decode(recorded)?);
                    assert!(
                        same(&expected, value),
                        "{what}:\nexpected {expected:?}\n     got {value:?}"
                    );
                }
            }
            last = Some(result);
        }
        if let Some(expected) = row["expected"].as_object() {
            let Some(Err((class, message))) = &last else {
                panic!("{label}: expected {:?}, got {last:?}", expected["class"]);
            };
            let message = String::from_utf8_lossy(message).into_owned();
            let expected_class = text(&expected["class"]);
            assert!(
                class == expected_class
                    || (expected_class == "InvalidArgumentException"
                        && class.ends_with("InvalidArgumentException")),
                "{label}: {class} is not a {expected_class}"
            );
            if let Some(contains) = expected["message"].as_str() {
                let contains = String::from_utf8_lossy(&localize(contains.as_bytes())).into_owned();
                assert!(
                    message.contains(&contains),
                    "{label}: {message:?} does not contain {contains:?}"
                );
            }
            if let Some(pattern) = expected["matches"].as_str() {
                let regex = regex::Regex::new(&pcre(pattern))?;
                assert!(
                    regex.is_match(&message),
                    "{label}: {message:?} does not match {pattern}"
                );
            }
        }
    }
    Ok(())
}

/// A PCRE pattern (`#…#` or `/…/`) as a Rust regex.
fn pcre(pattern: &str) -> String {
    let delimiter = pattern.chars().next().unwrap_or('/');
    let end = pattern.rfind(delimiter).unwrap_or(pattern.len());
    pattern[1..end].to_owned()
}

/// The recorded test names, for checking that each has a Rust test.
pub fn names(recording: &Json) -> Vec<&str> {
    recording
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default()
}

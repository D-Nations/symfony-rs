//! StreamedJsonResponseTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's.
//!
//! PHP arrays are php-value arrays; a generator is a [`Generator`], the
//! test's anonymous classes are small [`PhpObject`]s answering
//! `instanceof` as theirs do. What `send()` echoed is read from the
//! recorded output in place of `ob_start()`/`ob_get_clean()`.
//!
//! Upstream: symfony/http-foundation Tests/StreamedJsonResponseTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{
    PhpArray, PhpObject, Thrown, Value,
    json::{JSON_NUMERIC_CHECK, JSON_UNESCAPED_SLASHES},
    object::{ArrayObject, StdClass},
};
use symfony_http_foundation::{
    generator::Generator,
    header_bag::HeaderValues,
    json_response::JsonResponse,
    php::{Globals, OutputCall, set_globals, with_output},
    response::Response,
    streamed_json_response,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const NO_HEADERS: [(&str, HeaderValues); 0] = [];

fn s(text: &str) -> Value {
    Value::string(text)
}

/// `['key' => value, ...]`.
fn map(pairs: Vec<(&str, Value)>) -> Value {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    Value::Array(array)
}

fn list(values: Vec<Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

fn generator(pairs: Vec<(Value, Value)>) -> Value {
    Generator::new(pairs.into_iter().map(Ok)).object()
}

fn send(response: &mut Response) -> Result<String, Box<dyn std::error::Error>> {
    set_globals(Globals::default());
    response.send(true)?;
    let echoed: Vec<u8> = with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .concat()
    });
    Ok(String::from_utf8(echoed)?)
}

fn create_send_response(data: Value) -> Result<String, Box<dyn std::error::Error>> {
    let mut response = streamed_json_response::new(
        data,
        200,
        NO_HEADERS,
        JsonResponse::DEFAULT_ENCODING_OPTIONS,
    )?;
    send(&mut response)
}

fn generator_simple(test: &str, length: i64) -> Value {
    let test = test.to_owned();
    Generator::values((1..=length).map(move |i| Ok(s(&format!("{test} {i}"))))).object()
}

fn generator_array(test: &str, length: i64) -> Value {
    let test = test.to_owned();
    Generator::values(
        (1..=length).map(move |i| Ok(map(vec![("title", s(&format!("{test} {i}")))]))),
    )
    .object()
}

/// `new class implements \IteratorAggregate { getIterator() }`, and with
/// `\JsonSerializable` too when `json` is given.
#[derive(Debug)]
struct Aggregate {
    iterator: PhpArray,
    json: Option<Value>,
}

impl PhpObject for Aggregate {
    fn class(&self) -> &'static str {
        "IteratorAggregate@anonymous"
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn json_serialize(&self) -> Option<Result<Value, Thrown>> {
        self.json.clone().map(Ok)
    }

    fn instance_of(&self, class: &str) -> bool {
        match class {
            "Traversable" | "IteratorAggregate" => true,
            "JsonSerializable" => self.json.is_some(),
            _ => false,
        }
    }

    fn traverse(&self) -> Option<Result<Vec<(Value, Value)>, Thrown>> {
        Some(Ok(self
            .iterator
            .iter()
            .map(|(k, v)| (k.clone().into(), v.clone()))
            .collect()))
    }
}

#[test]
fn testResponseSimpleList() -> TestResult {
    let content = create_send_response(map(vec![(
        "_embedded",
        map(vec![
            ("articles", generator_simple("Article", 3)),
            ("news", generator_simple("News", 3)),
        ]),
    )]))?;

    assert_eq!(
        r#"{"_embedded":{"articles":["Article 1","Article 2","Article 3"],"news":["News 1","News 2","News 3"]}}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseSimpleGenerator() -> TestResult {
    let content = create_send_response(generator_simple("Article", 3))?;

    assert_eq!(r#"["Article 1","Article 2","Article 3"]"#, content);
    Ok(())
}

#[test]
fn testResponseNestedGenerator() -> TestResult {
    let content = create_send_response(generator(vec![
        (s("articles"), generator_simple("Article", 3)),
        (s("news"), generator_simple("News", 3)),
    ]))?;

    assert_eq!(
        r#"{"articles":["Article 1","Article 2","Article 3"],"news":["News 1","News 2","News 3"]}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseEmptyList() -> TestResult {
    let content = create_send_response(map(vec![(
        "_embedded",
        map(vec![("articles", generator_simple("Article", 0))]),
    )]))?;

    assert_eq!(r#"{"_embedded":{"articles":[]}}"#, content);
    Ok(())
}

#[test]
fn testResponseObjectsList() -> TestResult {
    let content = create_send_response(map(vec![(
        "_embedded",
        map(vec![("articles", generator_array("Article", 3))]),
    )]))?;

    assert_eq!(
        r#"{"_embedded":{"articles":[{"title":"Article 1"},{"title":"Article 2"},{"title":"Article 3"}]}}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseWithoutGenerator() -> TestResult {
    // while it is not the intended usage, all kind of iterables should be supported for good DX
    let content = create_send_response(map(vec![(
        "_embedded",
        map(vec![(
            "articles",
            list(vec![s("Article 1"), s("Article 2"), s("Article 3")]),
        )]),
    )]))?;

    assert_eq!(
        r#"{"_embedded":{"articles":["Article 1","Article 2","Article 3"]}}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseWithPlaceholder() -> TestResult {
    // the placeholder must not conflict with generator injection
    let content = create_send_response(map(vec![
        (
            "_embedded",
            map(vec![
                ("articles", generator_array("Article", 3)),
                ("placeholder", s("__symfony_json__")),
                ("news", generator_simple("News", 3)),
            ]),
        ),
        ("placeholder", s("__symfony_json__")),
    ]))?;

    assert_eq!(
        r#"{"_embedded":{"articles":[{"title":"Article 1"},{"title":"Article 2"},{"title":"Article 3"}],"placeholder":"__symfony_json__","news":["News 1","News 2","News 3"]},"placeholder":"__symfony_json__"}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseWithMixedKeyType() -> TestResult {
    let content = create_send_response(map(vec![(
        "_embedded",
        map(vec![
            (
                "list",
                generator(vec![(Value::Int(0), s("test")), (s("key"), s("value"))]),
            ),
            (
                "map",
                generator(vec![(s("key"), s("value")), (Value::Int(0), s("test"))]),
            ),
            (
                "integer",
                generator(vec![(Value::Int(1), s("one")), (Value::Int(3), s("three"))]),
            ),
        ]),
    )]))?;

    assert_eq!(
        r#"{"_embedded":{"list":["test","value"],"map":{"key":"value","0":"test"},"integer":{"1":"one","3":"three"}}}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseOtherTraversable() -> TestResult {
    let mut storage = PhpArray::new();
    storage.insert("__symfony_json__", s("__symfony_json__"));
    let array_object = Value::Object(Arc::new(ArrayObject::new(storage.clone())));

    let iterator_aggregate = Value::Object(Arc::new(Aggregate {
        iterator: PhpArray::list([s("__symfony_json__")]),
        json: None,
    }));

    let json_serializable = Value::Object(Arc::new(Aggregate {
        iterator: PhpArray::list([s("This should be ignored")]),
        json: Some(Value::Array(storage)),
    }));

    // while Generators should be used for performance reasons, the object should also work with any Traversable
    // to make things easier for a developer
    let content = create_send_response(map(vec![
        ("arrayObject", array_object),
        ("iteratorAggregate", iterator_aggregate),
        ("jsonSerializable", json_serializable),
        // add a Generator to make sure it still work in combination with other Traversable objects
        ("articles", generator_array("Article", 3)),
    ]))?;

    assert_eq!(
        r#"{"arrayObject":{"__symfony_json__":"__symfony_json__"},"iteratorAggregate":["__symfony_json__"],"jsonSerializable":{"__symfony_json__":"__symfony_json__"},"articles":[{"title":"Article 1"},{"title":"Article 2"},{"title":"Article 3"}]}"#,
        content
    );
    Ok(())
}

#[test]
fn testPlaceholderAsKeyAndValueInStructure() -> TestResult {
    let content = create_send_response(map(vec![
        ("__symfony_json__", s("__symfony_json__")),
        ("articles", generator_array("Article", 3)),
    ]))?;

    assert_eq!(
        r#"{"__symfony_json__":"__symfony_json__","articles":[{"title":"Article 1"},{"title":"Article 2"},{"title":"Article 3"}]}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseStatusCode() -> TestResult {
    let response = streamed_json_response::new(
        list(vec![]),
        201,
        NO_HEADERS,
        JsonResponse::DEFAULT_ENCODING_OPTIONS,
    )?;

    assert_eq!(201, response.get_status_code());
    Ok(())
}

#[test]
fn testPlaceholderAsObjectStructure() -> TestResult {
    let mut properties = PhpArray::new();
    properties.insert("__symfony_json__", s("foo"));
    properties.insert("bar", s("__symfony_json__"));
    let object = StdClass::object(properties);

    let content = create_send_response(map(vec![
        ("object", object),
        // add a Generator to make sure it still work in combination with other object holding placeholders
        ("articles", generator_array("Article", 3)),
    ]))?;

    assert_eq!(
        r#"{"object":{"__symfony_json__":"foo","bar":"__symfony_json__"},"articles":[{"title":"Article 1"},{"title":"Article 2"},{"title":"Article 3"}]}"#,
        content
    );
    Ok(())
}

#[test]
fn testResponseHeaders() -> TestResult {
    let response = streamed_json_response::new(
        list(vec![]),
        200,
        [("X-Test", HeaderValues::from("Test"))],
        JsonResponse::DEFAULT_ENCODING_OPTIONS,
    )?;

    assert_eq!(Some("Test".into()), response.headers().get("X-Test", None));
    Ok(())
}

#[test]
fn testCustomContentType() -> TestResult {
    let response = streamed_json_response::new(
        list(vec![]),
        200,
        [(
            "Content-Type",
            HeaderValues::from("application/json+stream"),
        )],
        JsonResponse::DEFAULT_ENCODING_OPTIONS,
    )?;

    assert_eq!(
        Some("application/json+stream".into()),
        response.headers().get("Content-Type", None)
    );
    Ok(())
}

#[test]
fn testEncodingOptions() -> TestResult {
    let mut response = streamed_json_response::new(
        map(vec![(
            "_embedded",
            map(vec![
                ("count", s("2")), // options are applied to the initial json encode
                (
                    "values",
                    generator(vec![
                        (s("with/unescaped/slash"), s("With/a/slash")), // options are applied to key and values
                        (s("3"), s("3")), // numeric check for value, but not for the key
                    ]),
                ),
            ]),
        )]),
        200,
        NO_HEADERS,
        JSON_UNESCAPED_SLASHES | JSON_NUMERIC_CHECK,
    )?;

    let content = send(&mut response)?;

    assert_eq!(
        r#"{"_embedded":{"count":2,"values":{"with/unescaped/slash":"With/a/slash","3":3}}}"#,
        content
    );
    Ok(())
}

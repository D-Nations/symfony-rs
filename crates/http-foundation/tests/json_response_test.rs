//! JsonResponseTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's. `expectException()` checks the class the
//! error carries; the anonymous `\Stringable` class is a
//! `StringableObject`, and `JsonSerializableObject` a Rust object whose
//! `jsonSerialize()` throws.
//!
//! Upstream: symfony/http-foundation Tests/JsonResponseTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{
    PhpArray, PhpObject, PhpString, Thrown, Value,
    compare::loose_equals,
    json::{JSON_FORCE_OBJECT, JSON_HEX_AMP, JSON_HEX_APOS, JSON_HEX_QUOT, JSON_HEX_TAG},
    object::{StdClass, StringableObject},
};
use symfony_http_foundation::json_response::{JsonResponse, JsonResponseError};

const NO_HEADERS: [(&str, &str); 0] = [];

fn content(response: &JsonResponse) -> &[u8] {
    response.get_content().as_bytes()
}

fn content_type(response: &JsonResponse) -> Option<PhpString> {
    response.headers().get("Content-Type", None)
}

fn new(data: Value) -> JsonResponse {
    JsonResponse::new(data, 200, NO_HEADERS, false).unwrap()
}

fn list(values: impl IntoIterator<Item = i64>) -> Value {
    Value::Array(PhpArray::list(values.into_iter().map(Value::Int)))
}

fn assoc(pairs: &[(&str, &str)]) -> Value {
    let mut array = PhpArray::new();
    for (k, v) in pairs {
        array.insert(*k, Value::string(*v));
    }
    Value::Array(array)
}

/// `$this->expectException($class)`.
fn assert_throws<T: std::fmt::Debug>(class: &str, result: Result<T, JsonResponseError>) {
    match result {
        Err(JsonResponseError::Exception(e)) => assert!(e.is_a(class), "{e:?} is not a {class}"),
        Err(JsonResponseError::Thrown(t)) => assert!(
            t.class == class || class == "Exception",
            "{t:?} is not a {class}"
        ),
        other => panic!("expected {class}, got {other:?}"),
    }
}

#[test]
fn testConstructorEmptyCreatesJsonObject() {
    let response = new(Value::Null);
    assert_eq!(b"{}", content(&response));
}

#[test]
fn testConstructorWithArrayCreatesJsonArray() {
    let response = new(list([0, 1, 2, 3]));
    assert_eq!(b"[0,1,2,3]", content(&response));
}

#[test]
fn testConstructorWithAssocArrayCreatesJsonObject() {
    let response = new(assoc(&[("foo", "bar")]));
    assert_eq!(br#"{"foo":"bar"}"#, content(&response));
}

#[test]
fn testConstructorWithSimpleTypes() {
    let response = new(Value::string("foo"));
    assert_eq!(br#""foo""#, content(&response));

    let response = new(Value::Int(0));
    assert_eq!(b"0", content(&response));

    let response = new(Value::Float(0.1));
    assert!(loose_equals(
        &Value::Float(0.1),
        &Value::String(response.get_content().clone())
    ));
    // assertIsString: getContent() is a string

    let response = new(Value::Bool(true));
    assert_eq!(b"true", content(&response));
}

#[test]
fn testConstructorWithCustomStatus() {
    let response = JsonResponse::new(list([]), 202, NO_HEADERS, false).unwrap();
    assert_eq!(202, response.get_status_code());
}

#[test]
fn testConstructorAddsContentTypeHeader() {
    let response = new(Value::Null);
    assert_eq!(
        Some(PhpString::from("application/json")),
        content_type(&response)
    );
}

#[test]
fn testConstructorWithCustomHeaders() {
    let response = JsonResponse::new(list([]), 200, [("ETag", "foo")], false).unwrap();
    assert_eq!(
        Some(PhpString::from("application/json")),
        content_type(&response)
    );
    assert_eq!(
        Some(PhpString::from("foo")),
        response.headers().get("ETag", None)
    );
}

#[test]
fn testConstructorWithCustomContentType() {
    let headers = [("Content-Type", "application/vnd.acme.blog-v1+json")];

    let response = JsonResponse::new(list([]), 200, headers, false).unwrap();
    assert_eq!(
        Some(PhpString::from("application/vnd.acme.blog-v1+json")),
        content_type(&response)
    );
}

#[test]
fn testSetJson() {
    let response = JsonResponse::new(Value::string("1"), 200, NO_HEADERS, true).unwrap();
    assert_eq!(b"1", content(&response));

    let response = JsonResponse::new(Value::string("[1]"), 200, NO_HEADERS, true).unwrap();
    assert_eq!(b"[1]", content(&response));

    let mut response = JsonResponse::new(Value::Null, 200, NO_HEADERS, false).unwrap();
    response.set_json("true");
    assert_eq!(b"true", content(&response));
}

#[test]
fn testSetCallback() {
    let mut response = new(assoc(&[("foo", "bar")]));
    response.set_callback(Some(b"callback")).unwrap();

    assert_eq!(br#"/**/callback({"foo":"bar"});"#, content(&response));
    assert_eq!(
        Some(PhpString::from("text/javascript")),
        content_type(&response)
    );
}

#[test]
fn testJsonEncodeFlags() {
    let response = new(Value::string("<>'&\""));

    assert_eq!(br#""\u003C\u003E\u0027\u0026\u0022""#, content(&response));
}

#[test]
fn testGetEncodingOptions() {
    let response = new(Value::Null);

    assert_eq!(
        JSON_HEX_TAG | JSON_HEX_APOS | JSON_HEX_AMP | JSON_HEX_QUOT,
        response.get_encoding_options()
    );
}

#[test]
fn testSetEncodingOptions() {
    let mut response = new(Value::Null);
    response
        .set_data(&Value::Array(PhpArray::list([list([1, 2, 3])])))
        .unwrap();

    assert_eq!(b"[[1,2,3]]", content(&response));

    response.set_encoding_options(JSON_FORCE_OBJECT).unwrap();

    assert_eq!(br#"{"0":{"0":1,"1":2,"2":3}}"#, content(&response));
}

#[test]
fn testItAcceptsJsonAsString() {
    let response = JsonResponse::from_json_string(r#"{"foo":"bar"}"#, 200, NO_HEADERS).unwrap();
    assert_eq!(br#"{"foo":"bar"}"#, content(&response));
}

#[test]
fn testSetCallbackInvalidIdentifier() {
    let mut response = new(Value::string("foo"));
    assert_throws(
        "InvalidArgumentException",
        response.set_callback(Some(b"+invalid")),
    );
}

#[test]
fn testSetContent() {
    assert_throws(
        "InvalidArgumentException",
        JsonResponse::new(Value::string(&b"\xB1\x31"[..]), 200, NO_HEADERS, false),
    );
}

/// The test file's `JsonSerializableObject`.
#[derive(Debug)]
struct JsonSerializableObject;

impl PhpObject for JsonSerializableObject {
    fn class(&self) -> &str {
        "Symfony\\Component\\HttpFoundation\\Tests\\JsonSerializableObject"
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn json_serialize(&self) -> Option<Result<Value, Thrown>> {
        Some(Err(Thrown::new("Exception", "This error is expected", 0)))
    }
}

#[test]
fn testSetContentJsonSerializeError() {
    let serializable = Value::Object(Arc::new(JsonSerializableObject));

    let result = JsonResponse::new(serializable, 200, NO_HEADERS, false);
    assert_throws("Exception", result.clone());
    let Err(JsonResponseError::Thrown(thrown)) = result else {
        panic!("expected the jsonSerialize() exception");
    };
    assert!(
        thrown
            .message
            .as_bytes()
            .windows(22)
            .any(|w| w == b"This error is expected")
    );
}

#[test]
fn testSetComplexCallback() {
    let mut response = new(assoc(&[("foo", "bar")]));
    response
        .set_callback(Some("ಠ_ಠ[\"foo\"].bar[0]".as_bytes()))
        .unwrap();

    assert_eq!(
        "/**/ಠ_ಠ[\"foo\"].bar[0]({\"foo\":\"bar\"});".as_bytes(),
        content(&response)
    );
}

/// `expectExceptionMessage()`: the message contains the text.
fn assert_message(result: Result<JsonResponse, JsonResponseError>, class: &str, text: &str) {
    let Err(JsonResponseError::Exception(e)) = result else {
        panic!("expected {class}");
    };
    assert!(e.is_a(class), "{e:?}");
    assert!(
        e.message()
            .as_bytes()
            .windows(text.len())
            .any(|w| w == text.as_bytes()),
        "{e:?}"
    );
}

#[test]
fn testConstructorWithNullAsDataThrowsAnUnexpectedValueException() {
    assert_message(
        JsonResponse::new(Value::Null, 200, NO_HEADERS, true),
        "TypeError",
        "If $json is set to true, argument $data must be a string or object implementing __toString(), \"null\" given.",
    );
}

#[test]
fn testConstructorWithObjectWithToStringMethod() {
    let class = StringableObject::object("class@anonymous", "{}");

    let response = JsonResponse::new(class, 200, NO_HEADERS, true).unwrap();

    assert_eq!(b"{}", content(&response));
}

#[test]
fn testConstructorWithObjectWithoutToStringMethodThrowsAnException() {
    assert_message(
        JsonResponse::new(StdClass::object(PhpArray::new()), 200, NO_HEADERS, true),
        "TypeError",
        "If $json is set to true, argument $data must be a string or object implementing __toString(), \"stdClass\" given.",
    );
}

#[test]
fn testSetDataWithNull() {
    let mut response = new(Value::Null);
    response.set_data(&Value::Null).unwrap();

    assert_eq!(b"null", content(&response));
}

//! LoaderLoadExceptionTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; `new \Exception($message)` is an `Error` of
//! class `\Exception`.
//!
//! Upstream: symfony/config Tests/Exception/LoaderLoadExceptionTest.php
#![allow(non_snake_case)]

use php_value::Value;
use symfony_config::{Class, Error, exception::loader_load_exception};

fn exception(message: &str) -> Error {
    Error::new(
        Class::Other {
            class: "Exception".to_owned(),
            exception: true,
        },
        message,
    )
}

#[track_caller]
fn assert_message(expected: &str, e: &Error) {
    assert_eq!(expected, String::from_utf8_lossy(e.get_message()));
}

#[test]
fn testMessageCannotLoadResource() {
    let exception = loader_load_exception(&Value::string("resource"), None, 0, None, None);
    assert_message("Cannot load resource \"resource\".", &exception);
}

#[test]
fn testMessageCannotLoadResourceWithType() {
    let exception =
        loader_load_exception(&Value::string("resource"), None, 0, None, Some(b"foobar"));
    assert_message(
        "Cannot load resource \"resource\". Make sure there is a loader supporting the \"foobar\" type.",
        &exception,
    );
}

#[test]
fn testMessageCannotLoadResourceWithAnnotationType() {
    let exception =
        loader_load_exception(&Value::string("resource"), None, 0, None, Some(b"annotation"));
    assert_message(
        "Cannot load resource \"resource\". Make sure there is a loader supporting the \"annotation\" type.",
        &exception,
    );
}

#[test]
fn testMessageCannotLoadResourceWithAttributeType() {
    let exception =
        loader_load_exception(&Value::string("resource"), None, 0, None, Some(b"attribute"));
    assert_message(
        "Cannot load resource \"resource\". Make sure there is a loader supporting the \"attribute\" type.",
        &exception,
    );
}

#[test]
fn testMessageCannotImportResourceFromSource() {
    let exception = loader_load_exception(
        &Value::string("resource"),
        Some(b"sourceResource"),
        0,
        None,
        None,
    );
    assert_message(
        "Cannot import resource \"resource\" from \"sourceResource\".",
        &exception,
    );
}

#[test]
fn testMessageCannotImportBundleResource() {
    let exception = loader_load_exception(
        &Value::string("@resource"),
        Some(b"sourceResource"),
        0,
        None,
        None,
    );
    assert_message(
        concat!(
            "Cannot import resource \"@resource\" from \"sourceResource\". ",
            "Make sure the \"resource\" bundle is correctly registered and loaded in the application kernel class. ",
            "If the bundle is registered, make sure the bundle path \"@resource\" is not empty."
        ),
        &exception,
    );
}

#[test]
fn testMessageHasPreviousErrorWithDotAndUnableToLoad() {
    let exception = loader_load_exception(
        &Value::string("resource"),
        None,
        0,
        Some(exception("There was a previous error with an ending dot.")),
        None,
    );
    assert_message(
        "There was a previous error with an ending dot in resource (which is loaded in resource \"resource\").",
        &exception,
    );
}

#[test]
fn testMessageHasPreviousErrorWithoutDotAndUnableToLoad() {
    let exception = loader_load_exception(
        &Value::string("resource"),
        None,
        0,
        Some(exception("There was a previous error with no ending dot")),
        None,
    );
    assert_message(
        "There was a previous error with no ending dot in resource (which is loaded in resource \"resource\").",
        &exception,
    );
}

#[test]
fn testMessageHasPreviousErrorAndUnableToLoadBundle() {
    let exception = loader_load_exception(
        &Value::string("@resource"),
        None,
        0,
        Some(exception("There was a previous error with an ending dot.")),
        None,
    );
    assert_message(
        concat!(
            "There was a previous error with an ending dot in @resource ",
            "(which is loaded in resource \"@resource\"). ",
            "Make sure the \"resource\" bundle is correctly registered and loaded in the application kernel class. ",
            "If the bundle is registered, make sure the bundle path \"@resource\" is not empty."
        ),
        &exception,
    );
}

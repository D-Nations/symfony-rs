//! RequestAttributeValueResolverTest.php at symfony/http-kernel v8.1.11.
//! Names and assertions are PHPUnit's; a data provider's sets run in one
//! test.
//!
//! Upstream: symfony/http-kernel Tests/Controller/ArgumentResolver/RequestAttributeValueResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{Handle, Value};
use symfony_http_foundation::{request::Request, request_stack::RequestHandle};
use symfony_http_kernel::{
    argument_resolver::ValueResolverInterface, controller_metadata::ArgumentMetadata,
    exception::instance_of, value_resolver::RequestAttributeValueResolver,
};

const NOT_FOUND: &str = "Symfony\\Component\\HttpKernel\\Exception\\NotFoundHttpException";

fn request_with(name: &str, value: Value) -> RequestHandle {
    let request = Handle::new(Request::default());
    request.with_mut(|r| r.attributes_mut().set(name, value));
    request
}

/// The resolved values, or `Err` for a `NotFoundHttpException`.
fn resolve(name: &str, value: Value, type_name: &str, has_default: bool) -> Result<Vec<Value>, ()> {
    let request = request_with(name, value);
    let metadata = ArgumentMetadata::simple(name, Some(type_name), false, has_default, Value::Null);
    RequestAttributeValueResolver
        .resolve(&request, &metadata)
        .map_err(|e| {
            assert!(instance_of(e.as_ref(), NOT_FOUND), "{e}");
        })
}

#[test]
fn testValidIntWithinRangeWorks() {
    assert_eq!(
        Ok(vec![Value::Int(123)]),
        resolve("id", Value::string("123"), "int", false)
    );
}

#[test]
fn testInvalidStringBecomes404() {
    assert!(resolve("id", Value::string("abc"), "int", false).is_err());
}

#[test]
fn testOutOfRangeIntBecomes404() {
    // one more than PHP_INT_MAX on 64-bit (string input)
    assert!(resolve("id", Value::string("9223372036854775808"), "int", false).is_err());
}

#[test]
fn testNullableIntAllowsNull() {
    assert_eq!(
        Ok(vec![Value::Null]),
        resolve("id", Value::Null, "int", true)
    );
}

/// A zero-padded decimal (e.g. "06" from a {month} placeholder with a
/// "\d+" requirement) must resolve to its int value instead of becoming a
/// 404.
#[test]
fn testLeadingZeroIntIsAccepted() {
    for (value, expected) in [("06", 6), ("007", 7), ("08", 8), ("00", 0), ("-06", -6)] {
        assert_eq!(
            Ok(vec![Value::Int(expected)]),
            resolve("id", Value::string(value), "int", false)
        );
    }
}

#[test]
fn testLeadingZeroOutOfRangeIntBecomes404() {
    // leading zero in front of (PHP_INT_MAX + 1) must still overflow, not
    // silently truncate
    assert!(resolve("id", Value::string("09223372036854775808"), "int", false).is_err());
}

#[test]
fn testFloatIsCoerced() {
    for (value, expected) in [("1.5", 1.5), ("06", 6.0)] {
        assert_eq!(
            Ok(vec![Value::Float(expected)]),
            resolve("price", Value::string(value), "float", false)
        );
    }
}

#[test]
fn testInvalidFloatBecomes404() {
    assert!(resolve("price", Value::string("abc"), "float", false).is_err());
}

#[test]
fn testBoolUsesStrictValidation() {
    for (value, expected) in [("1", true), ("0", false), ("true", true), ("false", false)] {
        assert_eq!(
            Ok(vec![Value::Bool(expected)]),
            resolve("active", Value::string(value), "bool", false)
        );
    }
}

#[test]
fn testNonTokenBoolBecomes404() {
    // unlike PHP's coercion (any non-empty string is true), bool keeps
    // strict token validation
    assert!(resolve("active", Value::string("banana"), "bool", false).is_err());
}

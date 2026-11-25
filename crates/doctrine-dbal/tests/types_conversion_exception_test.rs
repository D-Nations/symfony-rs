//! Port of tests/Types/ConversionExceptionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/ConversionExceptionTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use std::sync::Arc;

use common::{bytes, provider};
use doctrine_dbal::types::{ConversionException, Previous};
use types_support::provider_value;

/// `self::createStub(Throwable::class)`.
#[derive(Debug)]
struct StubThrowable;

impl std::fmt::Display for StubThrowable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stub")
    }
}

impl std::error::Error for StubThrowable {}

fn stub_throwable() -> Previous {
    Arc::new(StubThrowable)
}

fn same(a: Option<&Previous>, b: &Previous) -> bool {
    a.is_some_and(|a| Arc::ptr_eq(a, b))
}

#[test]
fn testConversionFailedPreviousException() {
    let previous = stub_throwable();

    let exception = ConversionException::value_not_convertible(
        &"foo".into(),
        "foo",
        None,
        Some(Arc::clone(&previous)),
    )
    .unwrap();

    assert!(same(exception.previous(), &previous));
}

#[test]
fn testConversionFailedInvalidTypeWithScalar() {
    let rows = provider(
        "Types/ConversionExceptionTest",
        "testConversionFailedInvalidTypeWithScalar",
    );
    assert_eq!(rows.len(), 7);
    for (name, args) in rows {
        let scalar_value = provider_value(&args[0]);
        let expected = bytes(&args[1]);
        let exception =
            ConversionException::invalid_type(&scalar_value, "foo", &["bar", "baz"], None);

        let message = exception.message().as_bytes();
        assert!(
            message
                .windows(expected.len())
                .any(|w| w == expected.as_slice()),
            "{name}: {:?} does not contain {:?}",
            exception.message(),
            String::from_utf8_lossy(&expected)
        );
    }
}

#[test]
fn testConversionFailedInvalidTypePreviousException() {
    let previous = stub_throwable();

    let exception = ConversionException::invalid_type(
        &"foo".into(),
        "foo",
        &["bar", "baz"],
        Some(Arc::clone(&previous)),
    );

    assert!(same(exception.previous(), &previous));
}

#[test]
fn testConversionFailedFormatPreservesPreviousException() {
    let previous = stub_throwable();

    let exception = ConversionException::invalid_format(
        b"foo",
        "bar",
        Some("baz"),
        Some(Arc::clone(&previous)),
    );

    assert!(same(exception.previous(), &previous));
}

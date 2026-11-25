//! Port of tests/Types/Exception/SerializationFailedTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/Exception/SerializationFailedTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::types::ConversionException;
use php_value::{
    PhpString, Value,
    json::{EncodeFailure, JSON_THROW_ON_ERROR, encode},
};

#[test]
fn testNew() {
    let value = Value::Float(f64::NAN);
    // json_encode($value); json_last_error_msg()
    let error = match encode(&value, 0, 512) {
        Ok(encoded) => encoded.error.expect("an error"),
        Err(EncodeFailure::Json { error, .. }) => error,
        Err(EncodeFailure::Thrown(thrown)) => panic!("{thrown}"),
    };
    assert!(encode(&value, JSON_THROW_ON_ERROR, 512).is_err());

    let exception =
        ConversionException::serialization_failed(&value, "json", error.message(), None);

    assert_eq!(
        exception.message(),
        &PhpString::from(
            "Could not convert PHP type \"float\" to \"json\". An error was triggered by the serialization: \
             Inf and NaN cannot be JSON encoded"
        )
    );
}

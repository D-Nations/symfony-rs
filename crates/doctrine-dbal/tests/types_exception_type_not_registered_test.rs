//! Port of tests/Types/Exception/TypeNotRegisteredTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/Exception/TypeNotRegisteredTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::types::{TypesException, get_type};

#[test]
fn testNew() {
    let exception = TypesException::type_not_registered(&get_type("string").unwrap());

    // '/Type of the class Doctrine\\\DBAL\\\Types\\\StringType@([0-9a-zA-Z]+) is not registered./'
    let message = exception.message();
    let rest = message
        .strip_prefix("Type of the class Doctrine\\DBAL\\Types\\StringType@")
        .unwrap_or_else(|| panic!("{message}"));
    let hash_len = rest.bytes().take_while(u8::is_ascii_alphanumeric).count();
    assert!(hash_len > 0, "{message}");
    let tail = &rest[hash_len..];
    assert!(
        tail.starts_with(" is not registered") && tail.len() == " is not registered.".len(),
        "{message}"
    );
}

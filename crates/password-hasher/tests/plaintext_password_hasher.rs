//! PlaintextPasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/PlaintextPasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_password_hasher::{Class, PasswordHasherInterface, PlaintextPasswordHasher};

#[test]
fn testVerify() {
    let hasher = PlaintextPasswordHasher::default();

    assert!(hasher.verify("foo", "foo", Some("")).unwrap());
    assert!(!hasher.verify("bar", "foo", Some("")).unwrap());
    assert!(!hasher.verify("FOO", "foo", Some("")).unwrap());

    let hasher = PlaintextPasswordHasher::new(true);

    assert!(hasher.verify("foo", "foo", Some("")).unwrap());
    assert!(!hasher.verify("bar", "foo", Some("")).unwrap());
    assert!(hasher.verify("FOO", "foo", Some("")).unwrap());
}

#[test]
fn testHash() {
    let hasher = PlaintextPasswordHasher::default();
    assert_eq!("foo", hasher.hash("foo", Some("")).unwrap());
}

#[test]
fn testHashLength() {
    let hasher = PlaintextPasswordHasher::default();
    let error = hasher.hash(&"a".repeat(5000), Some("salt")).unwrap_err();
    assert_eq!(&Class::InvalidPassword, error.class());
}

#[test]
fn testCheckPasswordLength() {
    let hasher = PlaintextPasswordHasher::default();
    assert!(
        !hasher
            .verify("encoded", &"a".repeat(5000), Some("salt"))
            .unwrap()
    );
}

#[test]
fn testUsingBracketInSaltThrows() {
    let hasher = PlaintextPasswordHasher::default();
    let error = hasher.hash("password", Some("{")).unwrap_err();
    assert!(error.is_logic());
}

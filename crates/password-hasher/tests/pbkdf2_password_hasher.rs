//! Pbkdf2PasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/Pbkdf2PasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_password_hasher::{Class, PasswordHasherInterface, Pbkdf2PasswordHasher};

#[test]
fn testVerify() {
    let hasher = Pbkdf2PasswordHasher::new("sha256", false, 1, 40).unwrap();
    assert!(
        hasher
            .verify(
                "c1232f10f62715fda06ae7c0a2037ca19b33cf103b727ba56d870c11f290a2ab106974c75607c8a3",
                "password",
                Some("")
            )
            .unwrap()
    );
}

#[test]
fn testHash() {
    let hasher = Pbkdf2PasswordHasher::new("sha256", false, 1, 40).unwrap();
    assert_eq!(
        "c1232f10f62715fda06ae7c0a2037ca19b33cf103b727ba56d870c11f290a2ab106974c75607c8a3",
        hasher.hash("password", Some("")).unwrap()
    );

    let hasher = Pbkdf2PasswordHasher::new("sha256", true, 1, 40).unwrap();
    assert_eq!(
        "wSMvEPYnFf2gaufAogN8oZszzxA7cnulbYcMEfKQoqsQaXTHVgfIow==",
        hasher.hash("password", Some("")).unwrap()
    );

    let hasher = Pbkdf2PasswordHasher::new("sha256", false, 2, 40).unwrap();
    assert_eq!(
        "8bc2f9167a81cdcfad1235cd9047f1136271c1f978fcfcb35e22dbeafa4634f6fd2214218ed63ebb",
        hasher.hash("password", Some("")).unwrap()
    );
}

#[test]
fn testHashAlgorithmDoesNotExist() {
    let hasher = Pbkdf2PasswordHasher::new("foobar", true, 1000, 40).unwrap();
    let error = hasher.hash("password", Some("")).unwrap_err();
    assert!(error.is_logic());
}

#[test]
fn testHashLength() {
    let hasher = Pbkdf2PasswordHasher::new("foobar", true, 1000, 40).unwrap();
    let error = hasher.hash(&"a".repeat(5000), Some("salt")).unwrap_err();
    assert_eq!(&Class::InvalidPassword, error.class());
}

#[test]
fn testCheckPasswordLength() {
    let hasher = Pbkdf2PasswordHasher::new("foobar", true, 1000, 40).unwrap();
    assert!(
        !hasher
            .verify("encoded", &"a".repeat(5000), Some("salt"))
            .unwrap()
    );
}

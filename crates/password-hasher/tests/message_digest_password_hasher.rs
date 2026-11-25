//! MessageDigestPasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's; `hash()` and `base64_encode()` are the port's own
//! (src/php/hash.rs, checked against PHP in tests/php.rs).
//!
//! Upstream: symfony/password-hasher Tests/Hasher/MessageDigestPasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_password_hasher::{
    Class, MessageDigestPasswordHasher, PasswordHasherInterface,
    php::hash::{HashAlgorithm, base64_encode, bin2hex},
};

#[test]
fn testVerify() {
    let hasher = MessageDigestPasswordHasher::new("sha256", false, 1);
    let hash = bin2hex(&HashAlgorithm::Sha256.digest(b"password"));
    assert!(hasher.verify(&hash, "password", Some("")).unwrap());
}

#[test]
fn testHash() {
    let hasher = MessageDigestPasswordHasher::new("sha256", false, 1);
    assert_eq!(
        bin2hex(&HashAlgorithm::Sha256.digest(b"password")),
        hasher.hash("password", Some("")).unwrap()
    );

    let hasher = MessageDigestPasswordHasher::new("sha256", true, 1);
    assert_eq!(
        base64_encode(&HashAlgorithm::Sha256.digest(b"password")),
        hasher.hash("password", Some("")).unwrap()
    );

    let hasher = MessageDigestPasswordHasher::new("sha256", false, 2);
    let mut inner = HashAlgorithm::Sha256.digest(b"password");
    inner.extend_from_slice(b"password");
    assert_eq!(
        bin2hex(&HashAlgorithm::Sha256.digest(&inner)),
        hasher.hash("password", Some("")).unwrap()
    );
}

#[test]
fn testHashAlgorithmDoesNotExist() {
    let hasher = MessageDigestPasswordHasher::new("foobar", true, 5000);
    let error = hasher.hash("password", Some("")).unwrap_err();
    assert!(error.is_logic());
}

#[test]
fn testHashLength() {
    let hasher = MessageDigestPasswordHasher::default();
    let error = hasher.hash(&"a".repeat(5000), Some("salt")).unwrap_err();
    assert_eq!(&Class::InvalidPassword, error.class());
}

#[test]
fn testCheckPasswordLength() {
    let hasher = MessageDigestPasswordHasher::default();
    assert!(
        !hasher
            .verify("encoded", &"a".repeat(5000), Some("salt"))
            .unwrap()
    );
}

#[test]
fn testUsingBracketInSaltThrows() {
    let hasher = MessageDigestPasswordHasher::new("sha256", false, 1);
    let error = hasher.hash("password", Some("{")).unwrap_err();
    assert!(error.is_logic());
}

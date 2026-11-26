//! SodiumPasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's. `setUp()` skips without libsodium; the port always has it.
//!
//! Upstream: symfony/password-hasher Tests/Hasher/SodiumPasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_password_hasher::{
    Class, NativePasswordHasher, PASSWORD_BCRYPT, PasswordHasherInterface, SodiumPasswordHasher,
    php::password::{PasswordAlgorithm, PasswordOptions, password_hash},
};

fn hasher() -> SodiumPasswordHasher {
    assert!(SodiumPasswordHasher::is_supported());
    SodiumPasswordHasher::new(None, None).unwrap()
}

#[test]
fn testValidation() {
    let hasher = hasher();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
    assert!(!hasher.verify(&result, "anotherPassword", None).unwrap());
    assert!(!hasher.verify(&result, "", None).unwrap());
}

#[test]
fn testBcryptValidation() {
    let hasher = hasher();
    assert!(
        hasher
            .verify(
                "$2y$04$M8GDODMoGQLQRpkYCdoJh.lbiZPee3SZI32RcYK49XYTolDGwoRMm",
                "abc",
                None
            )
            .unwrap()
    );
}

#[test]
fn testNonArgonValidation() {
    let hasher = hasher();
    assert!(
        hasher
            .verify(
                "$5$abcdefgh$ZLdkj8mkc2XVSrPVjskDAgZPGjtj1VGVaa1aUkrMTU/",
                "password",
                None
            )
            .unwrap()
    );
    assert!(
        !hasher
            .verify(
                "$5$abcdefgh$ZLdkj8mkc2XVSrPVjskDAgZPGjtj1VGVaa1aUkrMTU/",
                "anotherPassword",
                None
            )
            .unwrap()
    );
    assert!(hasher.verify("$6$abcdefgh$yVfUwsw5T.JApa8POvClA1pQ5peiq97DUNyXCZN5IrF.BMSkiaLQ5kvpuEm/VQ1Tvh/KV2TcaWh8qinoW5dhA1", "password", None).unwrap());
    assert!(!hasher.verify("$6$abcdefgh$yVfUwsw5T.JApa8POvClA1pQ5peiq97DUNyXCZN5IrF.BMSkiaLQ5kvpuEm/VQ1Tvh/KV2TcaWh8qinoW5dhA1", "anotherPassword", None).unwrap());
}

#[test]
fn testHashLength() {
    let error = hasher().hash(&"a".repeat(4097), None).unwrap_err();
    assert_eq!(&Class::InvalidPassword, error.class());
}

#[test]
fn testCheckPasswordLength() {
    let hasher = hasher();
    let result = hasher.hash(&"a".repeat(4096), None).unwrap();
    assert!(!hasher.verify(&result, &"a".repeat(4097), None).unwrap());
    assert!(hasher.verify(&result, &"a".repeat(4096), None).unwrap());
}

#[test]
fn testBcryptWithLongPassword() {
    let hasher = SodiumPasswordHasher::new(None, None).unwrap();
    let plain_password = "a".repeat(100);

    let options = PasswordOptions {
        cost: 4,
        time_cost: 4,
        memory_cost: 65536,
        threads: 1,
    };
    let php_hash = password_hash(
        plain_password.as_bytes(),
        &PasswordAlgorithm::Bcrypt,
        &options,
    )
    .unwrap();
    assert!(!hasher.verify(&php_hash, &plain_password, None).unwrap());
    let native = NativePasswordHasher::new(None, None, Some(4), Some(PASSWORD_BCRYPT)).unwrap();
    assert!(
        hasher
            .verify(
                &native.hash(&plain_password, None).unwrap(),
                &plain_password,
                None
            )
            .unwrap()
    );
}

#[test]
fn testPasswordNulByteGracefullyHandled() {
    let hasher = SodiumPasswordHasher::new(None, None).unwrap();
    let plain_password = "a\0b";
    assert!(
        hasher
            .verify(
                &hasher.hash(plain_password, None).unwrap(),
                plain_password,
                None
            )
            .unwrap()
    );
}

#[test]
fn testUserProvidedSaltIsNotUsed() {
    let hasher = hasher();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
}

#[test]
fn testNeedsRehash() {
    let hasher = SodiumPasswordHasher::new(Some(4), Some(11000)).unwrap();

    assert!(hasher.needs_rehash("dummyhash"));

    let hash = hasher.hash("foo", None).unwrap();
    assert!(!hasher.needs_rehash(&hash));

    let hasher = SodiumPasswordHasher::new(Some(5), Some(11000)).unwrap();
    assert!(hasher.needs_rehash(&hash));
}

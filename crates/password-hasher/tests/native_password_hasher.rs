//! NativePasswordHasherTest.php at symfony/password-hasher
//! 6934d16beaa4677f2c4584229fff1b51099dd7af. Names and assertions are
//! PHPUnit's; the data provider `validRangeData` (costs 4 to 31) and the
//! `#[TestWithJson]` costs are loops. `password_hash()` is the port's own
//! (src/php/password.rs).
//!
//! Upstream: symfony/password-hasher Tests/Hasher/NativePasswordHasherTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_password_hasher::{
    Class, NativePasswordHasher, PASSWORD_BCRYPT, PasswordHasherInterface,
    php::password::{PasswordAlgorithm, PasswordOptions, password_hash},
};

fn expect_invalid_argument(
    result: Result<NativePasswordHasher, symfony_password_hasher::Error>,
) -> String {
    let error = result.unwrap_err();
    assert_eq!(&Class::InvalidArgument, error.class());
    error.message().to_owned()
}

#[test]
fn testCostBelowRange() {
    expect_invalid_argument(NativePasswordHasher::new(None, None, Some(3), None));
}

#[test]
fn testCostAboveRange() {
    expect_invalid_argument(NativePasswordHasher::new(None, None, Some(32), None));
}

#[test]
fn testCostInRange() {
    // validRangeData
    for cost in 4..=31 {
        assert!(NativePasswordHasher::new(None, None, Some(cost), None).is_ok());
    }
}

#[test]
fn testValidation() {
    let hasher = NativePasswordHasher::new(None, None, None, None).unwrap();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
    assert!(!hasher.verify(&result, "anotherPassword", None).unwrap());
    assert!(!hasher.verify(&result, "", None).unwrap());
}

#[test]
fn testNonArgonValidation() {
    let hasher = NativePasswordHasher::new(None, None, None, None).unwrap();
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
fn testConfiguredAlgorithm() {
    let hasher = NativePasswordHasher::new(None, None, None, Some(PASSWORD_BCRYPT)).unwrap();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
    assert!(result.starts_with("$2"));
}

#[test]
fn testDefaultAlgorithm() {
    let hasher = NativePasswordHasher::new(None, None, None, None).unwrap();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
    assert!(result.starts_with("$2"));
}

#[test]
fn testConfiguredAlgorithmWithLegacyConstValue() {
    let hasher = NativePasswordHasher::new(None, None, None, Some("1")).unwrap();
    let result = hasher.hash("password", None).unwrap();
    assert!(hasher.verify(&result, "password", None).unwrap());
    assert!(result.starts_with("$2"));
}

#[test]
fn testBcryptWithLongPassword() {
    let hasher = NativePasswordHasher::new(None, None, Some(4), Some(PASSWORD_BCRYPT)).unwrap();
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
    assert!(
        hasher
            .verify(
                &hasher.hash(&plain_password, None).unwrap(),
                &plain_password,
                None
            )
            .unwrap()
    );
}

#[test]
fn testPasswordNulByteGracefullyHandled() {
    let hasher = NativePasswordHasher::new(None, None, Some(4), Some(PASSWORD_BCRYPT)).unwrap();
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
fn testNeedsRehash() {
    let hasher = NativePasswordHasher::new(Some(4), Some(11000), Some(4), None).unwrap();

    assert!(hasher.needs_rehash("dummyhash"));

    let hash = hasher.hash("foo", None).unwrap();
    assert!(!hasher.needs_rehash(&hash));

    let hasher = NativePasswordHasher::new(Some(5), Some(11000), Some(5), None).unwrap();
    assert!(hasher.needs_rehash(&hash));
}

#[test]
fn testLowOpsLimitsThrows() {
    let message = expect_invalid_argument(NativePasswordHasher::new(Some(2), None, None, None));
    assert_eq!("$opsLimit must be 3 or greater.", message);
}

#[test]
fn testLowMemLimitThrows() {
    let message =
        expect_invalid_argument(NativePasswordHasher::new(Some(3), Some(9999), None, None));
    assert_eq!("$memLimit must be 10k or greater.", message);
}

#[test]
fn testInvalidCostThrows() {
    for cost in [1, 40] {
        let message = expect_invalid_argument(NativePasswordHasher::new(
            Some(4),
            Some(11000),
            Some(cost),
            None,
        ));
        assert_eq!("$cost must be in the range of 4-31.", message);
    }
}

#[test]
fn testHashTooLongPasswordThrows() {
    let hasher = NativePasswordHasher::new(Some(4), Some(11000), Some(4), None).unwrap();
    let error = hasher.hash(&"a".repeat(5000), None).unwrap_err();
    assert_eq!(&Class::InvalidPassword, error.class());
}

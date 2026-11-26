//! Helpers of the Schema tests: `VerifyDeprecations` and
//! `expectException()`.
//!
//! Upstream: none
#![allow(dead_code)]

use doctrine_dbal::Exception;
use doctrine_dbal::deprecations;

/// `$this->expectDeprecationWithIdentifier($link)` around `f`.
pub fn expect_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert!(
        deprecations::count(link) > before,
        "Expected deprecation with identifier \"{link}\" was not triggered"
    );
    deprecations::disable();
    result
}

/// `$this->expectNoDeprecationWithIdentifier($link)` around `f`.
pub fn expect_no_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert_eq!(
        deprecations::count(link),
        before,
        "Unexpected deprecation with identifier \"{link}\" was triggered"
    );
    deprecations::disable();
    result
}

/// `$this->expectException($class)`: `result` fails with an instance of
/// `class` (a short name is taken from `Doctrine\DBAL\Schema\Exception`).
pub fn expect_exception<T: std::fmt::Debug>(
    class: &str,
    result: Result<T, Exception>,
) -> Exception {
    let class = if class.contains('\\') {
        class.to_owned()
    } else {
        format!("Doctrine\\DBAL\\Schema\\Exception\\{class}")
    };
    let e = result.expect_err(&format!("expected {class}"));
    assert!(e.is_instance_of(&class), "expected {class}, got {e}");
    e
}

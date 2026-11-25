//! Tests/ReferenceTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/ReferenceTest.php
#![allow(non_snake_case)]

use symfony_dependency_injection::Reference;

#[test]
fn testConstructor() {
    let reference = Reference::new("foo");
    assert_eq!(
        "foo",
        reference.to_string(),
        "__construct() sets the id of the reference, which is used for the __toString() method"
    );
}

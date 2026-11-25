//! Tests/ParameterTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/ParameterTest.php
#![allow(non_snake_case)]

use symfony_dependency_injection::Parameter;

#[test]
fn testConstructor() {
    let parameter = Parameter::new("foo");
    assert_eq!(
        "foo",
        parameter.to_string(),
        "__construct() sets the id of the parameter, which is used for the __toString() method"
    );
}

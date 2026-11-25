//! Tests/ParameterBag/FrozenParameterBagTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/ParameterBag/FrozenParameterBagTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, php_array};
use symfony_deprecation_contracts::collect;
use symfony_dependency_injection::ParameterBag;
use symfony_dependency_injection::parameter_bag::ParameterDeprecation;

#[test]
fn testConstructor() {
    let parameters = php_array!["foo" => "foo", "bar" => "bar"];
    let bag = ParameterBag::frozen(parameters.clone(), Vec::new(), Vec::new());
    assert_eq!(&parameters, bag.all(), "__construct() takes an array of parameters as its first argument");
}

#[test]
fn testClear() {
    let mut bag = ParameterBag::frozen(PhpArray::new(), Vec::new(), Vec::new());
    assert!(bag.clear().unwrap_err().instance_of("LogicException"));
}

#[test]
fn testSet() {
    let mut bag = ParameterBag::frozen(PhpArray::new(), Vec::new(), Vec::new());
    assert!(bag.set("foo", Value::from("bar")).unwrap_err().instance_of("LogicException"));
}

#[test]
fn testAdd() {
    let mut bag = ParameterBag::frozen(PhpArray::new(), Vec::new(), Vec::new());
    assert!(bag.add(&PhpArray::new()).unwrap_err().instance_of("LogicException"));
}

#[test]
fn testRemove() {
    let mut bag = ParameterBag::frozen(php_array!["foo" => "bar"], Vec::new(), Vec::new());
    assert!(bag.remove("foo").unwrap_err().instance_of("LogicException"));
}

#[test]
fn testDeprecate() {
    let mut bag = ParameterBag::frozen(php_array!["foo" => "bar"], Vec::new(), Vec::new());
    assert!(bag
        .deprecate("foo", "symfony/test", "6.3", None)
        .unwrap_err()
        .instance_of("LogicException"));
}

#[test]
fn testGetDeprecated() {
    let bag = ParameterBag::frozen(
        php_array!["foo" => "bar"],
        vec![(
            "foo".to_string(),
            ParameterDeprecation {
                package: "symfony/test".into(),
                version: "6.3".into(),
                message: "The parameter \"%s\" is deprecated.".into(),
                args: vec![Value::from("foo")],
            },
        )],
        Vec::new(),
    );
    let (result, deprecations) = collect(|| bag.get("foo"));
    result.unwrap();
    assert_eq!(1, deprecations.len());
    assert_eq!(
        b"Since symfony/test 6.3: The parameter \"foo\" is deprecated.".to_vec(),
        deprecations[0].message
    );
}

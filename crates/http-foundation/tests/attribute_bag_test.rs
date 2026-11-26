//! Session/Attribute/AttributeBagTest.php at symfony/http-foundation
//! v8.1.11. Names and assertions are PHPUnit's; each data set of a provider
//! is a case of the same test. `setUp()` is [`set_up`]; the array the bag
//! is initialized with is a [`Reference`].
//!
//! Upstream: symfony/http-foundation Tests/Session/Attribute/AttributeBagTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, Value};
use symfony_http_foundation::{
    php::Reference,
    session::{
        attribute::{attribute_bag::AttributeBag, attribute_bag_interface::AttributeBagInterface},
        session_bag_interface::SessionBagInterface,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn s(text: &str) -> Value {
    Value::string(text)
}

fn map(pairs: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

fn array() -> PhpArray {
    map(vec![
        ("hello", s("world")),
        ("always", s("be happy")),
        ("user.login", s("drak")),
        (
            "csrf.token",
            Value::Array(map(vec![("a", s("1234")), ("b", s("4321"))])),
        ),
        (
            "category",
            Value::Array(map(vec![(
                "fishing",
                Value::Array(map(vec![("first", s("cod")), ("second", s("sole"))])),
            )])),
        ),
    ])
}

fn set_up() -> Result<AttributeBag, Box<dyn std::error::Error>> {
    let bag = AttributeBag::new("_sf");
    bag.initialize(&Reference::new(Value::Array(array())))?;
    Ok(bag)
}

fn attributes_provider() -> Vec<(&'static str, Value, bool)> {
    vec![
        ("hello", s("world"), true),
        ("always", s("be happy"), true),
        ("user.login", s("drak"), true),
        (
            "csrf.token",
            Value::Array(map(vec![("a", s("1234")), ("b", s("4321"))])),
            true,
        ),
        (
            "category",
            Value::Array(map(vec![(
                "fishing",
                Value::Array(map(vec![("first", s("cod")), ("second", s("sole"))])),
            )])),
            true,
        ),
        ("user2.login", Value::Null, false),
        ("never", Value::Null, false),
        ("bye", Value::Null, false),
        ("bye/for/now", Value::Null, false),
    ]
}

#[test]
fn testInitialize() -> TestResult {
    let bag = AttributeBag::default();
    bag.initialize(&Reference::new(Value::Array(array())))?;
    assert_eq!(array(), bag.all());
    let changed = map(vec![("should", s("change"))]);
    bag.initialize(&Reference::new(Value::Array(changed.clone())))?;
    assert_eq!(changed, bag.all());
    Ok(())
}

#[test]
fn testGetStorageKey() -> TestResult {
    let bag = set_up()?;
    assert_eq!("_sf", bag.get_storage_key());
    let attribute_bag = AttributeBag::new("test");
    assert_eq!("test", attribute_bag.get_storage_key());
    Ok(())
}

#[test]
fn testGetSetName() -> TestResult {
    let bag = set_up()?;
    assert_eq!("attributes", bag.get_name());
    bag.set_name("foo");
    assert_eq!("foo", bag.get_name());
    Ok(())
}

#[test]
fn testHas() -> TestResult {
    for (key, _value, exists) in attributes_provider() {
        assert_eq!(exists, set_up()?.has(key), "{key}");
    }
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    for (key, value, _expected) in attributes_provider() {
        assert_eq!(value, set_up()?.get(key, Value::Null), "{key}");
    }
    Ok(())
}

#[test]
fn testGetDefaults() -> TestResult {
    let bag = set_up()?;
    assert_eq!(Value::Null, bag.get("user2.login", Value::Null));
    assert_eq!(s("default"), bag.get("user2.login", s("default")));
    Ok(())
}

#[test]
fn testSet() -> TestResult {
    for (key, value, _expected) in attributes_provider() {
        let bag = set_up()?;
        bag.set(key, value.clone());
        assert_eq!(value, bag.get(key, Value::Null), "{key}");
    }
    Ok(())
}

#[test]
fn testAll() -> TestResult {
    let bag = set_up()?;
    assert_eq!(array(), bag.all());

    bag.set("hello", s("fabien"));
    let mut expected = array();
    expected.insert("hello", s("fabien"));
    assert_eq!(expected, bag.all());
    Ok(())
}

#[test]
fn testReplace() -> TestResult {
    let bag = set_up()?;
    let array = map(vec![("name", s("jack")), ("foo.bar", s("beep"))]);
    bag.replace(&array);
    assert_eq!(array, bag.all());
    assert_eq!(Value::Null, bag.get("hello", Value::Null));
    assert_eq!(Value::Null, bag.get("always", Value::Null));
    assert_eq!(Value::Null, bag.get("user.login", Value::Null));
    Ok(())
}

#[test]
fn testRemove() -> TestResult {
    let bag = set_up()?;
    assert_eq!(s("world"), bag.get("hello", Value::Null));
    bag.remove("hello");
    assert_eq!(Value::Null, bag.get("hello", Value::Null));

    assert_eq!(s("be happy"), bag.get("always", Value::Null));
    bag.remove("always");
    assert_eq!(Value::Null, bag.get("always", Value::Null));

    assert_eq!(s("drak"), bag.get("user.login", Value::Null));
    bag.remove("user.login");
    assert_eq!(Value::Null, bag.get("user.login", Value::Null));
    Ok(())
}

#[test]
fn testClear() -> TestResult {
    let bag = set_up()?;
    bag.clear()?;
    assert_eq!(PhpArray::new(), bag.all());
    Ok(())
}

#[test]
fn testGetIterator() -> TestResult {
    let bag = set_up()?;
    let array = array();
    let mut i = 0;
    for (key, val) in bag.get_iterator().iter() {
        assert_eq!(array.get(key), Some(val));
        i += 1;
    }
    assert_eq!(array.len(), i);
    Ok(())
}

#[test]
fn testCount() -> TestResult {
    assert_eq!(array().len(), set_up()?.count());
    Ok(())
}

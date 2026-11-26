//! Session/Flash/FlashBagTest.php at symfony/http-foundation v8.1.11.
//! Names and assertions are PHPUnit's. `setUp()` is [`set_up`]; the array
//! the bag is initialized with is a [`Reference`].
//!
//! Upstream: symfony/http-foundation Tests/Session/Flash/FlashBagTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{PhpArray, Value, object::StdClass};
use symfony_http_foundation::{
    php::Reference,
    session::{
        flash::{flash_bag::FlashBag, flash_bag_interface::FlashBagInterface},
        session_bag_interface::SessionBagInterface,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn s(text: &str) -> Value {
    Value::string(text)
}

fn list(items: &[&str]) -> PhpArray {
    PhpArray::list(items.iter().map(|t| s(t)))
}

fn map(pairs: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

fn array() -> PhpArray {
    map(vec![(
        "notice",
        Value::Array(list(&["A previous flash message"])),
    )])
}

fn set_up() -> Result<FlashBag, Box<dyn std::error::Error>> {
    let bag = FlashBag::default();
    bag.initialize(&Reference::new(Value::Array(array())))?;
    Ok(bag)
}

#[test]
fn testInitialize() -> TestResult {
    let bag = FlashBag::default();
    bag.initialize(&Reference::new(Value::Array(array())))?;
    assert_eq!(array(), bag.peek_all()?);
    let changed = map(vec![("should", Value::Array(list(&["change"])))]);
    bag.initialize(&Reference::new(Value::Array(changed.clone())))?;
    assert_eq!(changed, bag.peek_all()?);
    Ok(())
}

#[test]
fn testGetStorageKey() -> TestResult {
    assert_eq!("_symfony_flashes", set_up()?.get_storage_key());
    let attribute_bag = FlashBag::new("test");
    assert_eq!("test", attribute_bag.get_storage_key());
    Ok(())
}

#[test]
fn testGetSetName() -> TestResult {
    let bag = set_up()?;
    assert_eq!("flashes", bag.get_name());
    bag.set_name("foo");
    assert_eq!("foo", bag.get_name());
    Ok(())
}

#[test]
fn testPeek() -> TestResult {
    let bag = set_up()?;
    assert_eq!(PhpArray::new(), bag.peek("non_existing", PhpArray::new())?);
    assert_eq!(
        list(&["default"]),
        bag.peek("not_existing", list(&["default"]))?
    );
    assert_eq!(
        list(&["A previous flash message"]),
        bag.peek("notice", PhpArray::new())?
    );
    assert_eq!(
        list(&["A previous flash message"]),
        bag.peek("notice", PhpArray::new())?
    );
    Ok(())
}

#[test]
fn testAdd() -> TestResult {
    let bag = set_up()?;
    let tab = map(vec![("bar", s("baz"))]);
    bag.add("string_message", s("lorem"))?;
    bag.add(
        "object_message",
        Value::Object(Arc::new(StdClass::default())),
    )?;
    bag.add("array_message", Value::Array(tab.clone()))?;

    assert_eq!(
        list(&["lorem"]),
        bag.get("string_message", PhpArray::new())?
    );
    let objects = bag.get("object_message", PhpArray::new())?;
    assert_eq!(1, objects.len());
    assert!(
        matches!(objects.values().next(), Some(Value::Object(o)) if o.class() == "stdClass" && o.properties().is_empty())
    );
    assert_eq!(
        PhpArray::list([Value::Array(tab)]),
        bag.get("array_message", PhpArray::new())?
    );
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    let bag = set_up()?;
    assert_eq!(PhpArray::new(), bag.get("non_existing", PhpArray::new())?);
    assert_eq!(
        list(&["default"]),
        bag.get("not_existing", list(&["default"]))?
    );
    assert_eq!(
        list(&["A previous flash message"]),
        bag.get("notice", PhpArray::new())?
    );
    assert_eq!(PhpArray::new(), bag.get("notice", PhpArray::new())?);
    Ok(())
}

#[test]
fn testAll() -> TestResult {
    let bag = set_up()?;
    bag.set("notice", s("Foo"))?;
    bag.set("error", s("Bar"))?;
    assert_eq!(
        map(vec![
            ("notice", Value::Array(list(&["Foo"]))),
            ("error", Value::Array(list(&["Bar"]))),
        ]),
        bag.all()?
    );

    assert_eq!(PhpArray::new(), bag.all()?);
    Ok(())
}

#[test]
fn testSet() -> TestResult {
    let bag = set_up()?;
    bag.set("notice", s("Foo"))?;
    bag.set("notice", s("Bar"))?;
    assert_eq!(list(&["Bar"]), bag.peek("notice", PhpArray::new())?);
    Ok(())
}

#[test]
fn testHas() -> TestResult {
    let bag = set_up()?;
    assert!(!bag.has("nothing")?);
    assert!(bag.has("notice")?);
    Ok(())
}

#[test]
fn testKeys() -> TestResult {
    assert_eq!(vec![s("notice")], set_up()?.keys()?);
    Ok(())
}

#[test]
fn testSetAll() -> TestResult {
    let bag = set_up()?;
    bag.add("one_flash", s("Foo"))?;
    bag.add("another_flash", s("Bar"))?;
    assert!(bag.has("one_flash")?);
    assert!(bag.has("another_flash")?);
    bag.set_all(map(vec![("unique_flash", s("FooBar"))]))?;
    assert!(!bag.has("one_flash")?);
    assert!(!bag.has("another_flash")?);
    assert_eq!(map(vec![("unique_flash", s("FooBar"))]), bag.all()?);
    assert_eq!(PhpArray::new(), bag.all()?);
    Ok(())
}

#[test]
fn testPeekAll() -> TestResult {
    let bag = set_up()?;
    bag.set("notice", s("Foo"))?;
    bag.set("error", s("Bar"))?;
    let expected = map(vec![
        ("notice", Value::Array(list(&["Foo"]))),
        ("error", Value::Array(list(&["Bar"]))),
    ]);
    assert_eq!(expected, bag.peek_all()?);
    assert!(bag.has("notice")?);
    assert!(bag.has("error")?);
    assert_eq!(expected, bag.peek_all()?);
    Ok(())
}

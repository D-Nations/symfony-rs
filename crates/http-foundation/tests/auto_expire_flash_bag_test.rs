//! Session/Flash/AutoExpireFlashBagTest.php at symfony/http-foundation
//! v8.1.11 (which imports `AutoExpireFlashBag` as `FlashBag`). Names and
//! assertions are PHPUnit's. `setUp()` is [`set_up`], which also returns
//! the reference standing for `$this->array`.
//!
//! Upstream: symfony/http-foundation Tests/Session/Flash/AutoExpireFlashBagTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, Value};
use symfony_http_foundation::{
    php::Reference,
    session::{
        flash::{
            auto_expire_flash_bag::AutoExpireFlashBag as FlashBag,
            flash_bag_interface::FlashBagInterface,
        },
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

fn set_up() -> Result<(FlashBag, Reference), Box<dyn std::error::Error>> {
    let bag = FlashBag::default();
    let array = Reference::new(Value::Array(map(vec![(
        "new",
        Value::Array(map(vec![(
            "notice",
            Value::Array(list(&["A previous flash message"])),
        )])),
    )])));
    bag.initialize(&array)?;
    Ok((bag, array))
}

#[test]
fn testInitialize() -> TestResult {
    let bag = FlashBag::default();
    let array = map(vec![(
        "new",
        Value::Array(map(vec![(
            "notice",
            Value::Array(list(&["A previous flash message"])),
        )])),
    )]);
    bag.initialize(&Reference::new(Value::Array(array)))?;
    assert_eq!(
        list(&["A previous flash message"]),
        bag.peek("notice", PhpArray::new())?
    );
    let array = map(vec![(
        "new",
        Value::Array(map(vec![
            ("notice", Value::Array(list(&["Something else"]))),
            ("error", Value::Array(list(&["a"]))),
        ])),
    )]);
    bag.initialize(&Reference::new(Value::Array(array)))?;
    assert_eq!(
        list(&["Something else"]),
        bag.peek("notice", PhpArray::new())?
    );
    assert_eq!(list(&["a"]), bag.peek("error", PhpArray::new())?);
    Ok(())
}

#[test]
fn testGetStorageKey() -> TestResult {
    assert_eq!("_symfony_flashes", set_up()?.0.get_storage_key());
    let attribute_bag = FlashBag::new("test");
    assert_eq!("test", attribute_bag.get_storage_key());
    Ok(())
}

#[test]
fn testGetSetName() -> TestResult {
    let (bag, _) = set_up()?;
    assert_eq!("flashes", bag.get_name());
    bag.set_name("foo");
    assert_eq!("foo", bag.get_name());
    Ok(())
}

#[test]
fn testPeek() -> TestResult {
    let (bag, _) = set_up()?;
    assert_eq!(PhpArray::new(), bag.peek("non_existing", PhpArray::new())?);
    assert_eq!(
        list(&["default"]),
        bag.peek("non_existing", list(&["default"]))?
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
fn testSet() -> TestResult {
    let (bag, _) = set_up()?;
    bag.set("notice", s("Foo"))?;
    assert_eq!(
        list(&["A previous flash message"]),
        bag.peek("notice", PhpArray::new())?
    );
    Ok(())
}

#[test]
fn testHas() -> TestResult {
    let (bag, _) = set_up()?;
    assert!(!bag.has("nothing")?);
    assert!(bag.has("notice")?);
    Ok(())
}

#[test]
fn testKeys() -> TestResult {
    assert_eq!(vec![s("notice")], set_up()?.0.keys()?);
    Ok(())
}

#[test]
fn testPeekAll() -> TestResult {
    let (bag, _) = set_up()?;
    let array = map(vec![(
        "new",
        Value::Array(map(vec![("notice", s("Foo")), ("error", s("Bar"))])),
    )]);

    bag.initialize(&Reference::new(Value::Array(array)))?;
    let expected = map(vec![("notice", s("Foo")), ("error", s("Bar"))]);
    assert_eq!(expected, bag.peek_all()?);

    assert_eq!(expected, bag.peek_all()?);
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    let (bag, _) = set_up()?;
    assert_eq!(PhpArray::new(), bag.get("non_existing", PhpArray::new())?);
    assert_eq!(
        list(&["default"]),
        bag.get("non_existing", list(&["default"]))?
    );
    assert_eq!(
        list(&["A previous flash message"]),
        bag.get("notice", PhpArray::new())?
    );
    assert_eq!(PhpArray::new(), bag.get("notice", PhpArray::new())?);
    Ok(())
}

#[test]
fn testSetAll() -> TestResult {
    let (bag, _) = set_up()?;
    bag.set_all(map(vec![("a", s("first")), ("b", s("second"))]))?;
    assert!(!bag.has("a")?);
    assert!(!bag.has("b")?);
    Ok(())
}

#[test]
fn testAll() -> TestResult {
    let (bag, _) = set_up()?;
    bag.set("notice", s("Foo"))?;
    bag.set("error", s("Bar"))?;
    assert_eq!(
        map(vec![(
            "notice",
            Value::Array(list(&["A previous flash message"]))
        )]),
        bag.all()?
    );

    assert_eq!(PhpArray::new(), bag.all()?);
    Ok(())
}

#[test]
fn testClear() -> TestResult {
    assert_eq!(
        Value::Array(map(vec![(
            "notice",
            Value::Array(list(&["A previous flash message"]))
        )])),
        set_up()?.0.clear()?
    );
    Ok(())
}

#[test]
fn testDoNotRemoveTheNewFlashesWhenDisplayingTheExistingOnes() -> TestResult {
    let (bag, array) = set_up()?;
    bag.add("success", s("Something"))?;
    bag.all()?;

    assert_eq!(
        Value::Array(map(vec![
            (
                "new",
                Value::Array(map(vec![("success", Value::Array(list(&["Something"])))]))
            ),
            ("display", Value::Array(PhpArray::new())),
        ])),
        array.get()
    );
    Ok(())
}

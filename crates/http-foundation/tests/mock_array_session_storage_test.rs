//! Session/Storage/MockArraySessionStorageTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's.
//! `setUp()` is [`set_up`].
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/MockArraySessionStorageTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{PhpArray, Value};
use symfony_http_foundation::session::{
    attribute::{attribute_bag::AttributeBag, attribute_bag_interface::AttributeBagInterface},
    flash::{flash_bag::FlashBag, flash_bag_interface::FlashBagInterface},
    session_bag_interface::SessionBagInterface,
    storage::{
        mock_array_session_storage::MockArraySessionStorage,
        session_storage_interface::SessionStorageInterface,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn map(pairs: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

fn set_up() -> MockArraySessionStorage {
    let attributes = Arc::new(AttributeBag::default());
    let flashes = Arc::new(FlashBag::default());
    let data = map(vec![
        (
            &attributes.get_storage_key(),
            Value::Array(map(vec![("foo", Value::string("bar"))])),
        ),
        (
            &flashes.get_storage_key(),
            Value::Array(map(vec![("notice", Value::string("hello"))])),
        ),
    ]);

    let storage = MockArraySessionStorage::default();
    storage.register_bag(flashes);
    storage.register_bag(attributes);
    storage.set_session_data(&data);
    storage
}

fn attributes(
    storage: &MockArraySessionStorage,
) -> Result<Arc<dyn AttributeBagInterface>, Box<dyn std::error::Error>> {
    Ok(storage
        .get_bag("attributes")?
        .into_attribute_bag()
        .ok_or("not an attribute bag")?)
}

fn flashes(
    storage: &MockArraySessionStorage,
) -> Result<Arc<dyn FlashBagInterface>, Box<dyn std::error::Error>> {
    Ok(storage
        .get_bag("flashes")?
        .into_flash_bag()
        .ok_or("not a flash bag")?)
}

#[test]
fn testStart() -> TestResult {
    let storage = set_up();
    assert_eq!("", storage.get_id());
    storage.start()?;
    let id = storage.get_id();
    assert_ne!("", id);
    storage.start()?;
    assert_eq!(id, storage.get_id());
    Ok(())
}

#[test]
fn testRegenerate() -> TestResult {
    let storage = set_up();
    storage.start()?;
    let id = storage.get_id();
    storage.regenerate(false, None)?;
    assert_ne!(id, storage.get_id());
    assert_eq!(
        map(vec![("foo", Value::string("bar"))]),
        attributes(&storage)?.all()
    );
    assert_eq!(
        map(vec![("notice", Value::string("hello"))]),
        flashes(&storage)?.peek_all()?
    );

    let id = storage.get_id();
    storage.regenerate(true, None)?;
    assert_ne!(id, storage.get_id());
    assert_eq!(
        map(vec![("foo", Value::string("bar"))]),
        attributes(&storage)?.all()
    );
    assert_eq!(
        map(vec![("notice", Value::string("hello"))]),
        flashes(&storage)?.peek_all()?
    );
    Ok(())
}

#[test]
fn testGetId() -> TestResult {
    let storage = set_up();
    assert_eq!("", storage.get_id());
    storage.start()?;
    assert_ne!("", storage.get_id());
    Ok(())
}

#[test]
fn testClearClearsBags() -> TestResult {
    let storage = set_up();
    storage.clear()?;

    assert_eq!(PhpArray::new(), attributes(&storage)?.all());
    assert_eq!(PhpArray::new(), flashes(&storage)?.peek_all()?);
    Ok(())
}

#[test]
fn testClearStartsSession() -> TestResult {
    let storage = set_up();
    storage.clear()?;

    assert!(storage.is_started());
    Ok(())
}

#[test]
fn testClearWithNoBagsStartsSession() -> TestResult {
    let storage = MockArraySessionStorage::default();

    storage.clear()?;

    assert!(storage.is_started());
    Ok(())
}

#[test]
fn testUnstartedSave() -> TestResult {
    let error = set_up().save().err().ok_or("expected a RuntimeException")?;
    assert!(error.is_a("RuntimeException"));
    Ok(())
}

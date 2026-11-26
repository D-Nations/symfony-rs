//! Session/Storage/MockFileSessionStorageTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's.
//! `setUp()` and `tearDown()` are [`Fixture`]; each test has a directory
//! of its own (`sys_get_temp_dir().'/sftest'` and a suffix), as Rust runs
//! the tests in parallel.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/MockFileSessionStorageTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{
    PhpArray, Value,
    constants::Registry,
    serialize::{Classes, serialize, unserialize},
};
use symfony_http_foundation::session::{
    attribute::{attribute_bag::AttributeBag, attribute_bag_interface::AttributeBagInterface},
    flash::{flash_bag::FlashBag, flash_bag_interface::FlashBagInterface},
    storage::{
        mock_file_session_storage::MockFileSessionStorage,
        session_storage_interface::SessionStorageInterface,
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Fixture {
    session_dir: String,
    storage: MockFileSessionStorage,
}

impl Fixture {
    fn new(tag: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let session_dir = format!(
            "{}/sftest-{}-{tag}",
            String::from_utf8(symfony_filesystem::php::sys_get_temp_dir())?,
            std::process::id()
        );
        let storage = get_storage(&session_dir)?;
        Ok(Self {
            session_dir,
            storage,
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.session_dir);
    }
}

fn get_storage(dir: &str) -> Result<MockFileSessionStorage, Box<dyn std::error::Error>> {
    let storage = MockFileSessionStorage::new(Some(dir.as_bytes().to_vec()), "MOCKSESSID", None)?;
    storage.register_bag(Arc::new(FlashBag::default()))?;
    storage.register_bag(Arc::new(AttributeBag::default()))?;
    Ok(storage)
}

fn attributes(
    storage: &MockFileSessionStorage,
) -> Result<Arc<dyn AttributeBagInterface>, Box<dyn std::error::Error>> {
    Ok(storage
        .get_bag("attributes")?
        .into_attribute_bag()
        .ok_or("not an attribute bag")?)
}

fn flashes(
    storage: &MockFileSessionStorage,
) -> Result<Arc<dyn FlashBagInterface>, Box<dyn std::error::Error>> {
    Ok(storage
        .get_bag("flashes")?
        .into_flash_bag()
        .ok_or("not a flash bag")?)
}

#[test]
fn testStart() -> TestResult {
    let f = Fixture::new("start")?;
    assert_eq!("", f.storage.get_id());
    assert!(f.storage.start()?);
    let id = f.storage.get_id();
    assert_ne!("", f.storage.get_id());
    assert!(f.storage.start()?);
    assert_eq!(id, f.storage.get_id());
    Ok(())
}

#[test]
fn testRegenerate() -> TestResult {
    let f = Fixture::new("regenerate")?;
    f.storage.start()?;
    attributes(&f.storage)?.set("regenerate", Value::Int(1234));
    f.storage.regenerate(false, None)?;
    assert_eq!(
        Value::Int(1234),
        attributes(&f.storage)?.get("regenerate", Value::Null)
    );
    f.storage.regenerate(true, None)?;
    assert_eq!(
        Value::Int(1234),
        attributes(&f.storage)?.get("regenerate", Value::Null)
    );
    Ok(())
}

#[test]
fn testGetId() -> TestResult {
    let f = Fixture::new("get-id")?;
    assert_eq!("", f.storage.get_id());
    f.storage.start()?;
    assert_ne!("", f.storage.get_id());
    Ok(())
}

#[test]
fn testSave() -> TestResult {
    let f = Fixture::new("save")?;
    f.storage.start()?;
    let id = f.storage.get_id();
    assert_ne!(
        Value::string("108"),
        attributes(&f.storage)?.get("new", Value::Null)
    );
    assert!(!flashes(&f.storage)?.has("newkey")?);
    attributes(&f.storage)?.set("new", Value::string("108"));
    flashes(&f.storage)?.set("newkey", Value::string("test"))?;
    f.storage.save()?;

    let storage = get_storage(&f.session_dir)?;
    storage.set_id(&id)?;
    storage.start()?;
    assert_eq!(
        Value::string("108"),
        attributes(&storage)?.get("new", Value::Null)
    );
    assert!(flashes(&storage)?.has("newkey")?);
    assert_eq!(
        PhpArray::list([Value::string("test")]),
        flashes(&storage)?.peek("newkey", PhpArray::new())?
    );
    Ok(())
}

#[test]
fn testMultipleInstances() -> TestResult {
    let f = Fixture::new("multiple")?;
    let storage1 = get_storage(&f.session_dir)?;
    storage1.start()?;
    attributes(&storage1)?.set("foo", Value::string("bar"));
    storage1.save()?;

    let storage2 = get_storage(&f.session_dir)?;
    storage2.set_id(&storage1.get_id())?;
    storage2.start()?;
    assert_eq!(
        Value::string("bar"),
        attributes(&storage2)?.get("foo", Value::Null),
        "values persist between instances"
    );
    Ok(())
}

#[test]
fn testSaveWithoutStart() -> TestResult {
    let f = Fixture::new("save-without-start")?;
    let storage1 = get_storage(&f.session_dir)?;
    let error = storage1.save().err().ok_or("expected a RuntimeException")?;
    assert!(error.is_a("RuntimeException"));
    Ok(())
}

#[test]
fn testSetIdIgnoresAnIdThatIsNotUsableAsAFileName() -> TestResult {
    let f = Fixture::new("unusable-id")?;
    let base = f
        .session_dir
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    f.storage.set_id(&format!("../{base}/planted"))?;

    assert_eq!("", f.storage.get_id());
    Ok(())
}

#[test]
fn testSetIdIgnoresAnIdThatIsTooLongForAFileName() -> TestResult {
    let f = Fixture::new("long-id")?;
    let id = "a".repeat(246);
    f.storage.set_id(&id)?;
    assert_eq!(id, f.storage.get_id());

    f.storage.set_id(&"a".repeat(247))?;
    assert_eq!("", f.storage.get_id());
    Ok(())
}

#[test]
fn testAnIdThatIsNotUsableAsAFileNameStartsANewSession() -> TestResult {
    let f = Fixture::new("planted")?;
    let planted = format!("{}/planted.mocksess", f.session_dir);
    let mut attributes_array = PhpArray::new();
    attributes_array.insert("foo", Value::string("bar"));
    let mut planted_data = PhpArray::new();
    planted_data.insert("_sf2_attributes", Value::Array(attributes_array));
    std::fs::write(
        &planted,
        serialize(&Value::Array(planted_data.clone())).as_bytes(),
    )?;

    let base = f
        .session_dir
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    f.storage.set_id(&format!("../{base}/planted"))?;
    f.storage.start()?;
    attributes(&f.storage)?.set("new", Value::string("108"));
    f.storage.save()?;

    assert_eq!(Value::Null, attributes(&f.storage)?.get("foo", Value::Null));
    let written = std::fs::read(&planted)?;
    let read = unserialize(&written, &Classes::new(), &Registry::default()).ok_or("unserialize")?;
    assert!(read.identical(&Value::Array(planted_data)));
    Ok(())
}

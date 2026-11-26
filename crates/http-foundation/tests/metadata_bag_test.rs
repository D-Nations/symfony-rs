//! Session/Storage/MetadataBagTest.php at symfony/http-foundation
//! v8.1.11. Names and assertions are PHPUnit's. `time()` and `sleep()` are
//! php-value's mocked clock, so `sleep(1)` moves it on at once.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/MetadataBagTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, Value, clock};
use symfony_http_foundation::{
    php::Reference,
    session::{
        session_bag_interface::SessionBagInterface,
        storage::metadata_bag::{CREATED, LIFETIME, MetadataBag, UPDATED},
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn meta(created: i64, updated: i64, lifetime: i64) -> Reference {
    let mut array = PhpArray::new();
    array.insert(CREATED, Value::Int(created));
    array.insert(UPDATED, Value::Int(updated));
    array.insert(LIFETIME, Value::Int(lifetime));
    Reference::new(Value::Array(array))
}

fn set_up() -> Result<MetadataBag, Box<dyn std::error::Error>> {
    let bag = MetadataBag::default();
    bag.initialize(&meta(1_234_567, 12_345_678, 0))?;
    Ok(bag)
}

fn updated(reference: &Reference) -> Value {
    match reference.get() {
        Value::Array(a) => a
            .get(&php_value::Key::from_bytes(UPDATED.as_bytes()))
            .cloned()
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

#[test]
fn testInitialize() -> TestResult {
    let _clock = clock::mock(1_700_000_000);
    let session_metadata = Reference::new(Value::Array(PhpArray::new()));

    let bag1 = MetadataBag::default();
    bag1.initialize(&session_metadata)?;
    assert!(bag1.get_created()? >= clock::time());
    assert_eq!(bag1.get_created()?, bag1.get_last_used()?);

    clock::sleep(1);
    let bag2 = MetadataBag::default();
    bag2.initialize(&session_metadata)?;
    assert_eq!(bag1.get_created()?, bag2.get_created()?);
    assert_eq!(bag1.get_last_used()?, bag2.get_last_used()?);
    assert_eq!(bag2.get_created()?, bag2.get_last_used()?);

    clock::sleep(1);
    let bag3 = MetadataBag::default();
    bag3.initialize(&session_metadata)?;
    assert_eq!(bag1.get_created()?, bag3.get_created()?);
    assert!(bag3.get_last_used()? > bag2.get_last_used()?);
    assert_ne!(bag3.get_created()?, bag3.get_last_used()?);
    Ok(())
}

#[test]
fn testGetSetName() -> TestResult {
    let bag = set_up()?;
    assert_eq!("__metadata", bag.get_name());
    bag.set_name("foo");
    assert_eq!("foo", bag.get_name());
    Ok(())
}

#[test]
fn testGetStorageKey() -> TestResult {
    assert_eq!("_sf2_meta", set_up()?.get_storage_key());
    Ok(())
}

#[test]
fn testGetLifetime() -> TestResult {
    let bag = MetadataBag::default();
    bag.initialize(&meta(1_234_567, 12_345_678, 1000))?;
    assert_eq!(1000, bag.get_lifetime()?);
    Ok(())
}

#[test]
fn testGetCreated() -> TestResult {
    assert_eq!(1_234_567, set_up()?.get_created()?);
    Ok(())
}

#[test]
fn testGetLastUsed() -> TestResult {
    assert!(set_up()?.get_last_used()? <= clock::time());
    Ok(())
}

#[test]
fn testClear() -> TestResult {
    set_up()?.clear()?;

    // the clear method has no side effects, we just want to ensure it doesn't trigger any exceptions
    Ok(())
}

#[test]
fn testSkipLastUsedUpdate() -> TestResult {
    let bag = MetadataBag::new("", 30, None);
    let time_stamp = clock::time();

    let created = time_stamp - 15;
    let session_metadata = meta(created, created, 1000);
    bag.initialize(&session_metadata)?;

    assert_eq!(Value::Int(created), updated(&session_metadata));
    Ok(())
}

#[test]
fn testDoesNotSkipLastUsedUpdate() -> TestResult {
    let _clock = clock::mock(1_700_000_000);
    let bag = MetadataBag::new("", 30, None);
    let time_stamp = clock::time();

    let created = time_stamp - 45;
    let session_metadata = meta(created, created, 1000);
    bag.initialize(&session_metadata)?;

    assert_eq!(Value::Int(time_stamp), updated(&session_metadata));
    Ok(())
}

#[test]
fn testLifetimeIsInt() -> TestResult {
    let session_metadata = Reference::new(Value::Array(PhpArray::new()));
    let bag = MetadataBag::default();
    bag.initialize(&session_metadata)?;

    // getLifetime() returns an int
    bag.get_lifetime()?;
    Ok(())
}

#[test]
fn testCookieLifetimeFromConstructor() -> TestResult {
    let bag = MetadataBag::new("_sf2_meta", 0, Some(60));
    let session_metadata = Reference::new(Value::Array(PhpArray::new()));

    bag.initialize(&session_metadata)?;

    assert_eq!(60, bag.get_lifetime()?);
    Ok(())
}

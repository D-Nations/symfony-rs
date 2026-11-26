//! Session/Storage/PhpBridgeSessionStorageTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! each test runs on a thread of its own, with a save path of its own.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/PhpBridgeSessionStorageTest.php
#![allow(non_snake_case)]

#[path = "native_session/mod.rs"]
mod native_session;

use std::sync::Arc;

use native_session::{SavePath, TestResult};
use php_value::{PhpArray, Value};
use symfony_http_foundation::{
    php_session::{self, SessionVar, Status, session_start, session_status},
    session::{
        attribute::attribute_bag::AttributeBag,
        session_bag_interface::SessionBagInterface,
        storage::{
            native_session_storage::{NativeSessionStorage, session_value},
            php_bridge_session_storage,
            session_storage_interface::SessionStorageInterface,
        },
    },
};

/// `getStorage()`.
fn get_storage() -> Result<NativeSessionStorage, Box<dyn std::error::Error>> {
    let storage = php_bridge_session_storage::new(None, None)?;
    storage.register_bag(Arc::new(AttributeBag::default()))?;
    Ok(storage)
}

#[test]
fn testPhpSession() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = get_storage()?;

    assert_ne!(Status::Active, session_status());
    assert!(!storage.is_started());

    session_start()?;
    assert!(php_session::with_session_var(|s| !matches!(
        s,
        SessionVar::Unset
    )));
    assert_eq!(Status::Active, session_status());
    // PHP session might have started, but the storage driver has not, so false is correct here
    assert!(!storage.is_started());

    let key = storage.get_metadata_bag().get_storage_key();
    assert_eq!(None, session_value(&key));
    storage.start()?;
    assert!(session_value(&key).is_some());
    Ok(())
}

#[test]
fn testClear() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = get_storage()?;
    session_start()?;
    php_session::with_session_var(|s| {
        if let SessionVar::Array(data) = s {
            data.assign("drak".into(), Value::string("loves symfony"));
        }
    });
    storage
        .get_bag("attributes")?
        .into_attribute_bag()
        .ok_or("not an attribute bag")?
        .set("symfony", Value::string("greatness"));
    let key = storage.get_bag("attributes")?.get_storage_key();
    let mut expected = PhpArray::new();
    expected.insert("symfony", Value::string("greatness"));
    assert_eq!(Some(Value::Array(expected)), session_value(&key));
    assert_eq!(Some(Value::string("loves symfony")), session_value("drak"));
    storage.clear()?;
    assert_eq!(Some(Value::Array(PhpArray::new())), session_value(&key));
    assert_eq!(Some(Value::string("loves symfony")), session_value("drak"));
    Ok(())
}

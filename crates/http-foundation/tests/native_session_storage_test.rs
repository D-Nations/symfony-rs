//! Session/Storage/NativeSessionStorageTest.php at symfony/http-foundation
//! v8.1.11. Names and assertions are PHPUnit's; each test runs on a
//! thread of its own, as PHPUnit runs it in a process of its own. Session
//! files go to a save path of the test's own ([`native_session`]).
//! `testCreatingManyInstancesDoesNotLeakMemory` is `n/a` (parity
//! manifest): it measures PHP's `memory_get_usage()`.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/NativeSessionStorageTest.php
#![allow(non_snake_case)]

#[path = "native_session/mod.rs"]
mod native_session;

use std::sync::Arc;

use native_session::{SavePath, TestResult};
use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    php::with_globals,
    php_session::{
        self, SessionVar, ini_get, ini_set, session_get_cookie_params, session_id, session_start,
    },
    session::{
        attribute::attribute_bag::AttributeBag,
        flash::flash_bag::FlashBag,
        session_bag_interface::{SessionBagInterface, same_bag},
        storage::{
            handler::{
                native_file_session_handler::NativeFileSessionHandler,
                null_session_handler::NullSessionHandler,
            },
            native_session_storage::{
                NativeSessionStorage, SaveHandler, SaveHandlerProxy, session_value,
            },
            proxy::session_handler_proxy::SessionHandlerProxy,
            session_storage_interface::SessionStorageInterface,
        },
    },
};

fn options(pairs: &[(&str, Value)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(*key, value.clone());
    }
    array
}

/// `getStorage($options)`.
fn get_storage(options: &PhpArray) -> Result<NativeSessionStorage, Box<dyn std::error::Error>> {
    let storage = NativeSessionStorage::new(options, None, None)?;
    storage.register_bag(Arc::new(AttributeBag::default()))?;
    Ok(storage)
}

fn storage() -> Result<NativeSessionStorage, Box<dyn std::error::Error>> {
    get_storage(&PhpArray::new())
}

fn attributes(
    storage: &NativeSessionStorage,
) -> Result<Arc<dyn symfony_http_foundation::session::attribute::attribute_bag_interface::AttributeBagInterface>, Box<dyn std::error::Error>>{
    Ok(storage
        .get_bag("attributes")?
        .into_attribute_bag()
        .ok_or("not an attribute bag")?)
}

#[test]
fn testBag() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    let bag = Arc::new(FlashBag::default());
    storage.register_bag(bag.clone())?;
    let bag: Arc<dyn SessionBagInterface> = bag;
    assert!(same_bag(&bag, &storage.get_bag(&bag.get_name())?));
    Ok(())
}

#[test]
fn testRegisterBagException() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    let error = storage
        .get_bag("non_existing")
        .err()
        .ok_or("expected an exception")?;
    assert!(error.is_a("InvalidArgumentException"));
    Ok(())
}

#[test]
fn testRegisterBagForAStartedSessionThrowsException() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    let error = storage
        .register_bag(Arc::new(AttributeBag::default()))
        .err()
        .ok_or("expected an exception")?;
    assert!(error.is_a("LogicException"));
    Ok(())
}

#[test]
fn testGetId() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    assert_eq!("", storage.get_id(), "Empty ID before starting session");

    storage.start()?;
    let id = storage.get_id();
    assert_ne!("", id);

    storage.save()?;
    assert_eq!(id, storage.get_id(), "ID stays after saving session");
    Ok(())
}

#[test]
fn testRegenerate() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    let id = storage.get_id();
    attributes(&storage)?.set("lucky", Value::Int(7));
    storage.regenerate(false, None)?;
    assert_ne!(id, storage.get_id());
    assert_eq!(
        Value::Int(7),
        attributes(&storage)?.get("lucky", Value::Null)
    );
    Ok(())
}

#[test]
fn testRegenerateDestroy() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    let id = storage.get_id();
    attributes(&storage)?.set("legs", Value::Int(11));
    storage.regenerate(true, None)?;
    assert_ne!(id, storage.get_id());
    assert_eq!(
        Value::Int(11),
        attributes(&storage)?.get("legs", Value::Null)
    );
    Ok(())
}

#[test]
fn testRegenerateWithCustomLifetime() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    let id = storage.get_id();
    let lifetime = 999_999;
    attributes(&storage)?.set("legs", Value::Int(11));
    storage.regenerate(false, Some(lifetime))?;
    assert_ne!(id, storage.get_id());
    assert_eq!(
        Value::Int(11),
        attributes(&storage)?.get("legs", Value::Null)
    );
    assert_eq!(Some(PhpString::from("999999")), ini_get("cookie_lifetime"));
    Ok(())
}

#[test]
fn testSessionGlobalIsUpToDateAfterIdRegeneration() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    attributes(&storage)?.set("lucky", Value::Int(7));
    storage.regenerate(false, None)?;
    attributes(&storage)?.set("lucky", Value::Int(42));

    let attributes = session_value("_sf2_attributes").ok_or("no attributes")?;
    let lucky = attributes
        .as_array()
        .and_then(|a| a.get(&"lucky".into()).cloned());
    assert_eq!(Some(Value::Int(42)), lucky);
    Ok(())
}

#[test]
fn testRegenerationFailureDoesNotFlagStorageAsStarted() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    assert!(!storage.regenerate(false, None)?);
    assert!(!storage.is_started());
    Ok(())
}

#[test]
fn testDefaultSessionCacheLimiter() -> TestResult {
    let _save_path = SavePath::set_up();
    ini_set("cache_limiter", "nocache");
    NativeSessionStorage::new(&PhpArray::new(), None, None)?;
    assert_eq!(Some(PhpString::default()), ini_get("cache_limiter"));
    Ok(())
}

#[test]
fn testExplicitSessionCacheLimiter() -> TestResult {
    let _save_path = SavePath::set_up();
    ini_set("cache_limiter", "nocache");
    NativeSessionStorage::new(
        &options(&[("cache_limiter", Value::string("public"))]),
        None,
        None,
    )?;
    assert_eq!(Some(PhpString::from("public")), ini_get("cache_limiter"));
    Ok(())
}

#[test]
fn testCookieOptions() -> TestResult {
    let _save_path = SavePath::set_up();
    // PHP 8.4: no cookie_partitioned
    let expected = options(&[
        ("cookie_lifetime", Value::Int(123_456)),
        ("cookie_path", Value::string("/my/cookie/path")),
        ("cookie_domain", Value::string("symfony.example.com")),
        ("cookie_secure", Value::Bool(true)),
        ("cookie_httponly", Value::Bool(false)),
        ("cookie_samesite", Value::string("lax")),
    ]);

    get_storage(&expected)?;
    let mut gco = PhpArray::new();
    for (key, value) in session_get_cookie_params().iter() {
        gco.insert(
            format!("cookie_{}", String::from_utf8_lossy(&key.bytes())).as_str(),
            value.clone(),
        );
    }

    assert!(php_value::compare::loose_equals(
        &Value::Array(expected),
        &Value::Array(gco)
    ));
    Ok(())
}

#[test]
fn testCacheExpireOption() -> TestResult {
    let _save_path = SavePath::set_up();
    get_storage(&options(&[("cache_expire", Value::string("200"))]))?;
    assert_eq!(Some(PhpString::from("200")), ini_get("cache_expire"));
    Ok(())
}

#[test]
fn testSetSaveHandler() -> TestResult {
    let _save_path = SavePath::set_up();
    ini_set("save_handler", "files");
    let is_handler_proxy = |storage: &NativeSessionStorage| {
        matches!(
            storage.get_save_handler(),
            SaveHandlerProxy::HandlerProxy(_)
        )
    };
    let storage = storage()?;
    storage.set_save_handler(None)?;
    assert!(is_handler_proxy(&storage));
    storage.set_save_handler(Some(SaveHandler::HandlerProxy(Arc::new(
        SessionHandlerProxy::new(Arc::new(NativeFileSessionHandler::new(None)?)),
    ))))?;
    assert!(is_handler_proxy(&storage));
    storage.set_save_handler(Some(SaveHandler::Handler(Arc::new(
        NativeFileSessionHandler::new(None)?,
    ))))?;
    assert!(is_handler_proxy(&storage));
    storage.set_save_handler(Some(SaveHandler::HandlerProxy(Arc::new(
        SessionHandlerProxy::new(Arc::new(NullSessionHandler::default())),
    ))))?;
    assert!(is_handler_proxy(&storage));
    storage.set_save_handler(Some(SaveHandler::Handler(Arc::new(
        NullSessionHandler::default(),
    ))))?;
    assert!(is_handler_proxy(&storage));
    Ok(())
}

#[test]
fn testStarted() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;

    assert!(!storage.get_save_handler().proxy().is_active());
    assert!(!storage.is_started());

    session_start()?;
    assert!(php_session::with_session_var(|s| !matches!(
        s,
        SessionVar::Unset
    )));
    assert!(storage.get_save_handler().proxy().is_active());

    // PHP session might have started, but the storage driver has not, so false is correct here
    assert!(!storage.is_started());

    let key = storage.get_metadata_bag().get_storage_key();
    assert_eq!(None, session_value(&key));
    let error = storage.start().err().ok_or("expected a RuntimeException")?;
    assert!(error.is_a("RuntimeException"));
    Ok(())
}

#[test]
fn testRestart() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    storage.start()?;
    let id = storage.get_id();
    attributes(&storage)?.set("lucky", Value::Int(7));
    storage.save()?;
    storage.start()?;
    assert_eq!(id, storage.get_id(), "Same session ID after restarting");
    assert_eq!(
        Value::Int(7),
        attributes(&storage)?.get("lucky", Value::Null),
        "Data still available"
    );
    Ok(())
}

#[test]
fn testCanCreateNativeSessionStorageWhenSessionAlreadyStarted() -> TestResult {
    let _save_path = SavePath::set_up();
    session_start()?;
    storage()?;
    Ok(())
}

#[test]
fn testSetSessionOptionsOnceSessionStartedIsIgnored() -> TestResult {
    let _save_path = SavePath::set_up();
    session_start()?;
    get_storage(&options(&[("name", Value::string("something-else"))]))?;
    Ok(())
}

#[test]
fn testGetBagsOnceSessionStartedIsIgnored() -> TestResult {
    let _save_path = SavePath::set_up();
    session_start()?;
    let bag = Arc::new(AttributeBag::default());
    bag.set_name("flashes");

    let storage = storage()?;
    storage.register_bag(bag.clone())?;

    let bag: Arc<dyn SessionBagInterface> = bag;
    assert!(same_bag(&storage.get_bag("flashes")?, &bag));
    Ok(())
}

fn valid_id(id: &[u8]) -> bool {
    (22..=250).contains(&id.len())
        && id
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b',' || *b == b'-')
}

fn tampered_cookie() {
    let name = php_session::session_name();
    with_globals(|g| {
        g.cookie.insert(
            php_value::Key::from_bytes(name.as_bytes()),
            Value::string("&~["),
        );
    });
    php_session::set_session_id("&~[");
}

#[test]
fn testRegenerateInvalidSessionIdForNativeFileSessionHandler() -> TestResult {
    let _save_path = SavePath::set_up();
    tampered_cookie();
    let storage = NativeSessionStorage::new(
        &PhpArray::new(),
        Some(SaveHandler::Handler(Arc::new(
            NativeFileSessionHandler::new(None)?,
        ))),
        None,
    )?;
    let started = storage.start()?;

    assert!(started);
    assert!(valid_id(session_id().as_bytes()));
    storage.save()?;

    tampered_cookie();
    let storage = NativeSessionStorage::new(
        &PhpArray::new(),
        Some(SaveHandler::HandlerProxy(Arc::new(
            SessionHandlerProxy::new(Arc::new(NativeFileSessionHandler::new(None)?)),
        ))),
        None,
    )?;
    let started = storage.start()?;

    assert!(started);
    assert!(valid_id(session_id().as_bytes()));
    storage.save()?;

    tampered_cookie();
    let storage = NativeSessionStorage::new(
        &PhpArray::new(),
        Some(SaveHandler::Handler(
            Arc::new(NullSessionHandler::default()),
        )),
        None,
    )?;
    let started = storage.start()?;
    assert!(started);
    assert_eq!(PhpString::from("&~["), session_id());
    Ok(())
}

#[test]
fn testSaveHandlesNullSessionGracefully() -> TestResult {
    let _save_path = SavePath::set_up();
    let storage = storage()?;
    php_session::with_session_var(|s| *s = SessionVar::Other(Value::Null));
    storage.save()?;
    Ok(())
}

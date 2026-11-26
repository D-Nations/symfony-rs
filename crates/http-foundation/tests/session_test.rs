//! Session/SessionTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's; each data set of a provider is a case of the
//! same test. `setUp()` is [`set_up`]. `assertSame()` of objects compares
//! the `Arc`s' addresses.
//!
//! Upstream: symfony/http-foundation Tests/Session/SessionTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex, atomic::AtomicI64};

use php_value::{PhpArray, Value};
use symfony_http_foundation::session::{
    Data, Session,
    attribute::attribute_bag::AttributeBag,
    flash::flash_bag::FlashBag,
    flash_bag_aware_session_interface::FlashBagAwareSessionInterface,
    session_bag_interface::{SessionBagInterface, same_bag},
    session_bag_proxy::SessionBagProxy,
    session_interface::SessionInterface,
    storage::{
        mock_array_session_storage::MockArraySessionStorage,
        session_storage_interface::SessionStorageInterface,
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

fn set_up() -> Session {
    let storage = Arc::new(MockArraySessionStorage::default());
    Session::new(
        storage,
        Some(Arc::new(AttributeBag::default())),
        Some(Arc::new(FlashBag::default())),
        None,
    )
}

fn set_provider() -> Vec<(&'static str, Value, PhpArray)> {
    vec![
        ("foo", s("bar"), map(vec![("foo", s("bar"))])),
        (
            "foo.bar",
            s("too much beer"),
            map(vec![("foo.bar", s("too much beer"))]),
        ),
        (
            "great",
            s("symfony is great"),
            map(vec![("great", s("symfony is great"))]),
        ),
    ]
}

#[test]
fn testStart() -> TestResult {
    let session = set_up();
    assert_eq!("", session.get_id());
    assert!(session.start()?);
    assert_ne!("", session.get_id());
    Ok(())
}

#[test]
fn testIsStarted() -> TestResult {
    let session = set_up();
    assert!(!session.is_started());
    session.start()?;
    assert!(session.is_started());
    Ok(())
}

#[test]
fn testSetId() -> TestResult {
    let session = set_up();
    assert_eq!("", session.get_id());
    session.set_id("0123456789abcdef")?;
    session.start()?;
    assert_eq!("0123456789abcdef", session.get_id());
    Ok(())
}

#[test]
fn testSetIdAfterStart() -> TestResult {
    let session = set_up();
    session.start()?;
    let id = session.get_id();

    let e = session.set_id(&id).err();
    assert!(e.is_none());

    let e = session
        .set_id("different")
        .err()
        .ok_or("expected a LogicException")?;
    assert!(e.is_a("LogicException"));
    Ok(())
}

#[test]
fn testSetName() -> TestResult {
    let session = set_up();
    assert_eq!("MOCKSESSID", session.get_name());
    session.set_name("session.test.com");
    session.start()?;
    assert_eq!("session.test.com", session.get_name());
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    let session = set_up();
    // tests defaults
    assert_eq!(Value::Null, session.get("foo", Value::Null)?);
    assert_eq!(Value::Int(1), session.get("foo", Value::Int(1))?);
    Ok(())
}

#[test]
fn testSet() -> TestResult {
    for (key, value, _result) in set_provider() {
        let session = set_up();
        session.set(key, value.clone())?;
        assert_eq!(value, session.get(key, Value::Null)?, "{key}");
    }
    Ok(())
}

#[test]
fn testHas() -> TestResult {
    for (key, value, _result) in set_provider() {
        let session = set_up();
        session.set(key, value)?;
        assert!(session.has(key)?, "{key}");
        assert!(!session.has(&format!("{key}non_value"))?, "{key}");
    }
    Ok(())
}

#[test]
fn testReplace() -> TestResult {
    let session = set_up();
    let attributes = map(vec![("happiness", s("be good")), ("symfony", s("awesome"))]);
    session.replace(&attributes)?;
    assert_eq!(attributes, session.all()?);
    session.replace(&PhpArray::new())?;
    assert_eq!(PhpArray::new(), session.all()?);
    Ok(())
}

#[test]
fn testAll() -> TestResult {
    for (key, value, result) in set_provider() {
        let session = set_up();
        session.set(key, value)?;
        assert_eq!(result, session.all()?, "{key}");
    }
    Ok(())
}

#[test]
fn testClear() -> TestResult {
    for (key, value, _result) in set_provider() {
        let session = set_up();
        session.set("hi", s("fabien"))?;
        session.set(key, value)?;
        session.clear()?;
        assert_eq!(PhpArray::new(), session.all()?, "{key}");
    }
    Ok(())
}

#[test]
fn testRemove() -> TestResult {
    for (key, value, _result) in set_provider() {
        let session = set_up();
        session.set("hi.world", s("have a nice day"))?;
        session.set(key, value)?;
        session.remove(key)?;
        assert_eq!(
            map(vec![("hi.world", s("have a nice day"))]),
            session.all()?,
            "{key}"
        );
    }
    Ok(())
}

#[test]
fn testInvalidate() -> TestResult {
    let session = set_up();
    session.set("invalidate", Value::Int(123))?;
    session.invalidate(None)?;
    assert_eq!(PhpArray::new(), session.all()?);
    Ok(())
}

#[test]
fn testMigrate() -> TestResult {
    let session = set_up();
    session.set("migrate", Value::Int(321))?;
    session.migrate(false, None)?;
    assert_eq!(Value::Int(321), session.get("migrate", Value::Null)?);
    Ok(())
}

#[test]
fn testMigrateDestroy() -> TestResult {
    let session = set_up();
    session.set("migrate", Value::Int(333))?;
    session.migrate(true, None)?;
    assert_eq!(Value::Int(333), session.get("migrate", Value::Null)?);
    Ok(())
}

#[test]
fn testSave() -> TestResult {
    let session = set_up();
    session.start()?;
    session.save()?;

    assert!(!session.is_started());
    Ok(())
}

#[test]
fn testGetId() -> TestResult {
    let session = set_up();
    assert_eq!("", session.get_id());
    session.start()?;
    assert_ne!("", session.get_id());
    Ok(())
}

#[test]
fn testGetFlashBag() -> TestResult {
    // a FlashBagInterface
    set_up().get_flash_bag()?;
    Ok(())
}

#[test]
fn testGetIterator() -> TestResult {
    let session = set_up();
    let attributes = map(vec![("hello", s("world")), ("symfony", s("rocks"))]);
    for (key, val) in attributes.iter() {
        session.set(
            &String::from_utf8_lossy(
                key.to_value()
                    .to_php_string()
                    .unwrap_or_default()
                    .as_bytes(),
            ),
            val.clone(),
        )?;
    }

    let mut i = 0;
    for (key, val) in session.get_iterator()?.iter() {
        assert_eq!(attributes.get(key), Some(val));
        i += 1;
    }

    assert_eq!(attributes.len(), i);
    Ok(())
}

#[test]
fn testGetCount() -> TestResult {
    let session = set_up();
    session.set("hello", s("world"))?;
    session.set("symfony", s("rocks"))?;

    assert_eq!(2, session.count()?);
    Ok(())
}

#[test]
fn testGetMeta() {
    // a MetadataBag
    let _bag = set_up().get_metadata_bag();
}

#[test]
fn testIsEmpty() -> TestResult {
    let session = set_up();
    assert!(session.is_empty());

    session.set("hello", s("world"))?;
    assert!(!session.is_empty());

    session.remove("hello")?;
    assert!(session.is_empty());

    let flash = session.get_flash_bag()?;
    flash.set("hello", s("world"))?;
    assert!(!session.is_empty());

    flash.get("hello", PhpArray::new())?;
    assert!(session.is_empty());
    Ok(())
}

#[test]
fn testGetBagWithBagImplementingGetBag() -> TestResult {
    let bag = Arc::new(AttributeBag::default());
    bag.set_name("foo");

    let storage = Arc::new(MockArraySessionStorage::default());
    storage.register_bag(bag.clone());

    let bag: Arc<dyn SessionBagInterface> = bag;
    assert!(same_bag(
        &bag,
        &Session::new(storage, None, None, None).get_bag("foo")?
    ));
    Ok(())
}

#[test]
fn testGetBagWithBagNotImplementingGetBag() -> TestResult {
    let data = Arc::new(Mutex::new(Data::default()));

    let bag = Arc::new(AttributeBag::default());
    bag.set_name("foo");

    let storage = Arc::new(MockArraySessionStorage::default());
    let inner: Arc<dyn SessionBagInterface> = bag.clone();
    storage.register_bag(Arc::new(SessionBagProxy::new(
        inner,
        data,
        Arc::new(AtomicI64::new(0)),
        None,
    )));

    let bag: Arc<dyn SessionBagInterface> = bag;
    assert!(same_bag(
        &bag,
        &Session::new(storage, None, None, None).get_bag("foo")?
    ));
    Ok(())
}

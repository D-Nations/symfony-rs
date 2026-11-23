//! FileResourceTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's. `sys_get_temp_dir()` is a private temporary directory;
//! `serialize()`/`unserialize()` go through `__serialize()` and
//! `from_serialized()`.
//!
//! Upstream: symfony/config Tests/Resource/FileResourceTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult, expect_class,
    fs::{TempDir, text, time, touch, unlink},
};
use php_value::serialize;
use symfony_config::{
    Class,
    resource::{FileResource, ResourceInterface},
};
use symfony_finder::fs::realpath;

struct SetUp {
    _dir: TempDir,
    file: Vec<u8>,
    time: i64,
    resource: FileResource,
}

fn set_up() -> Result<SetUp, symfony_config::Error> {
    let dir = TempDir::new();
    let file = dir.join("/tmp.xml");
    let time = time();
    touch(&file, Some(time));
    let resource = FileResource::new(&file)?;
    Ok(SetUp {
        _dir: dir,
        file,
        time,
        resource,
    })
}

#[test]
fn testGetResource() -> TestResult {
    let t = set_up()?;
    assert_eq!(
        realpath(&t.file),
        Some(t.resource.get_resource().to_vec()),
        "->getResource() returns the path to the resource"
    );
    Ok(())
}

#[test]
fn testGetResourceWithScheme() -> TestResult {
    let t = set_up()?;
    let mut schemed = b"file://".to_vec();
    schemed.extend_from_slice(&t.file);
    let resource = FileResource::new(&schemed)?;
    assert_eq!(
        text(&schemed),
        text(resource.get_resource()),
        "->getResource() returns the path to the schemed resource"
    );
    Ok(())
}

#[test]
fn testToString() -> TestResult {
    let t = set_up()?;
    assert_eq!(realpath(&t.file), Some(t.resource.to_string()));
    Ok(())
}

#[test]
fn testResourceDoesNotExist() {
    let result = FileResource::new(b"/____foo/foobar123456");
    let message = match &result {
        Err(e) => text(e.get_message()),
        Ok(_) => String::new(),
    };
    expect_class(result, Class::InvalidArgument);
    assert!(message.starts_with("The file \"") && message.ends_with("\" does not exist."));
}

#[test]
fn testIsFresh() -> TestResult {
    let t = set_up()?;
    assert!(
        t.resource.is_fresh_at(t.time)?,
        "->isFresh() returns true if the resource has not changed in same second"
    );
    assert!(
        t.resource.is_fresh_at(t.time + 10)?,
        "->isFresh() returns true if the resource has not changed"
    );
    assert!(
        !t.resource.is_fresh_at(t.time - 86400)?,
        "->isFresh() returns false if the resource has been updated"
    );
    Ok(())
}

#[test]
fn testIsFreshForDeletedResources() -> TestResult {
    let t = set_up()?;
    unlink(&t.file);

    assert!(
        !t.resource.is_fresh_at(t.time)?,
        "->isFresh() returns false if the resource does not exist"
    );
    Ok(())
}

#[test]
fn testSerializeUnserialize() -> TestResult {
    let t = set_up()?;
    let data = t.resource.serialize()?;
    let _ = serialize::serialize(&php_value::Value::Array(data.clone()));
    let _ = FileResource::from_serialized(&data)?;

    assert_eq!(realpath(&t.file), Some(t.resource.get_resource().to_vec()));
    Ok(())
}

trait FreshAt {
    fn is_fresh_at(&self, timestamp: i64) -> Result<bool, symfony_config::Error>;
}

impl<T: ResourceInterface> FreshAt for T {
    fn is_fresh_at(&self, timestamp: i64) -> Result<bool, symfony_config::Error> {
        match self.as_self_checking() {
            Some(r) => r.is_fresh(timestamp),
            None => Ok(false),
        }
    }
}

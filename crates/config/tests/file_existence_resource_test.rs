//! FileExistenceResourceTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's. `sys_get_temp_dir()` is a private temporary
//! directory; `unserialize(serialize(...))` is `from_serialized()` of
//! `__serialize()`.
//!
//! Upstream: symfony/config Tests/Resource/FileExistenceResourceTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult,
    fs::{TempDir, text, time, touch, unlink},
};
use symfony_config::resource::{
    FileExistenceResource, ResourceInterface, SelfCheckingResourceInterface,
};

struct SetUp {
    _dir: TempDir,
    file: Vec<u8>,
    time: i64,
    resource: FileExistenceResource,
}

fn set_up() -> SetUp {
    let dir = TempDir::new();
    let file = dir.join("/tmp.xml");
    SetUp {
        file: file.clone(),
        time: time(),
        resource: FileExistenceResource::new(&file),
        _dir: dir,
    }
}

#[test]
fn testToString() {
    let t = set_up();
    assert_eq!(
        format!("existence.{}", text(&t.file)),
        text(&t.resource.to_string())
    );
}

#[test]
fn testGetResource() {
    let t = set_up();
    assert_eq!(
        text(&t.file),
        text(t.resource.get_resource()),
        "->getResource() returns the path to the resource"
    );
}

#[test]
fn testIsFreshWithExistingResource() -> TestResult {
    let t = set_up();
    touch(&t.file, Some(t.time));
    let serialized = FileExistenceResource::new(&t.file).serialize()?;

    let resource = FileExistenceResource::from_serialized(&serialized)?;
    assert!(
        resource.is_fresh(t.time)?,
        "->isFresh() returns true if the resource is still present"
    );

    unlink(&t.file);
    let resource = FileExistenceResource::from_serialized(&serialized)?;
    assert!(
        !resource.is_fresh(t.time)?,
        "->isFresh() returns false if the resource has been deleted"
    );
    Ok(())
}

#[test]
fn testIsFreshWithAbsentResource() -> TestResult {
    let t = set_up();
    let serialized = FileExistenceResource::new(&t.file).serialize()?;

    let resource = FileExistenceResource::from_serialized(&serialized)?;
    assert!(
        resource.is_fresh(t.time)?,
        "->isFresh() returns true if the resource is still absent"
    );

    touch(&t.file, Some(t.time));
    let resource = FileExistenceResource::from_serialized(&serialized)?;
    assert!(
        !resource.is_fresh(t.time)?,
        "->isFresh() returns false if the resource has been created"
    );
    Ok(())
}

//! DirectoryResourceTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's. `sys_get_temp_dir()` is a private temporary
//! directory; `usleep(1500000)` is a sleep of 1.5 s.
//!
//! Upstream: symfony/config Tests/Resource/DirectoryResourceTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult, expect_class,
    fs::{TempDir, mkdir, remove_directory, text, time, touch, unlink},
};
use symfony_config::{
    Class,
    resource::{DirectoryResource, ResourceInterface, SelfCheckingResourceInterface},
};
use symfony_finder::fs::realpath;

struct SetUp {
    _dir: TempDir,
    directory: Vec<u8>,
}

impl SetUp {
    fn join(&self, suffix: &str) -> Vec<u8> {
        let mut p = self.directory.clone();
        p.extend_from_slice(suffix.as_bytes());
        p
    }
}

fn set_up() -> SetUp {
    let dir = TempDir::new();
    let directory = dir.join("/symfonyDirectoryIterator");
    mkdir(&directory);
    let t = SetUp {
        _dir: dir,
        directory,
    };
    touch(&t.join("/tmp.xml"), None);
    t
}

#[test]
fn testGetResource() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    assert_eq!(
        realpath(&t.directory),
        Some(resource.get_resource().to_vec()),
        "->getResource() returns the path to the resource"
    );
    Ok(())
}

#[test]
fn testGetPattern() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, Some(b"bar"))?;
    assert_eq!(Some(&b"bar"[..]), resource.get_pattern());
    Ok(())
}

#[test]
fn testResourceDoesNotExist() {
    let result = DirectoryResource::new(b"/____foo/foobar123456", None);
    let message = match &result {
        Err(e) => text(e.get_message()),
        Ok(_) => String::new(),
    };
    expect_class(result, Class::InvalidArgument);
    assert!(message.starts_with("The directory \"") && message.ends_with("\" does not exist."));
}

#[test]
fn testIsFresh() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    assert!(
        resource.is_fresh(time() + 10)?,
        "->isFresh() returns true if the resource has not changed"
    );
    assert!(
        !resource.is_fresh(time() - 86400)?,
        "->isFresh() returns false if the resource has been updated"
    );
    Ok(())
}

#[test]
fn testIsFreshForDeletedResources() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    remove_directory(&t.directory);

    assert!(
        !resource.is_fresh(time())?,
        "->isFresh() returns false if the resource does not exist"
    );
    Ok(())
}

#[test]
fn testIsFreshUpdateFile() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    touch(&t.join("/tmp.xml"), Some(time() + 20));
    assert!(
        !resource.is_fresh(time() + 10)?,
        "->isFresh() returns false if an existing file is modified"
    );
    Ok(())
}

#[test]
fn testIsFreshNewFile() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    touch(&t.join("/new.xml"), Some(time() + 20));
    assert!(
        !resource.is_fresh(time() + 10)?,
        "->isFresh() returns false if a new file is added"
    );
    Ok(())
}

#[test]
fn testIsFreshNewFileWithDifferentPattern() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, Some(b"/.xml$/"))?;
    touch(&t.join("/new.yaml"), Some(time() + 20));
    assert!(
        resource.is_fresh(time() + 10)?,
        "->isFresh() returns true if a new file with a non-matching pattern is added"
    );
    Ok(())
}

#[test]
fn testIsFreshDeleteFile() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    let time = time();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    unlink(&t.join("/tmp.xml"));
    assert!(
        !resource.is_fresh(time)?,
        "->isFresh() returns false if an existing file is removed"
    );
    Ok(())
}

#[test]
fn testIsFreshDeleteDirectory() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;
    remove_directory(&t.directory);
    assert!(
        !resource.is_fresh(time())?,
        "->isFresh() returns false if the whole resource is removed"
    );
    Ok(())
}

#[test]
fn testIsFreshCreateFileInSubdirectory() -> TestResult {
    let t = set_up();
    let subdirectory = t.join("/subdirectory");
    mkdir(&subdirectory);

    let resource = DirectoryResource::new(&t.directory, None)?;
    assert!(
        resource.is_fresh(time() + 10)?,
        "->isFresh() returns true if an unmodified subdirectory exists"
    );

    let mut file = subdirectory.clone();
    file.extend_from_slice(b"/newfile.xml");
    touch(&file, Some(time() + 20));
    assert!(
        !resource.is_fresh(time() + 10)?,
        "->isFresh() returns false if a new file in a subdirectory is added"
    );
    Ok(())
}

#[test]
fn testIsFreshModifySubdirectory() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, None)?;

    let subdirectory = t.join("/subdirectory");
    mkdir(&subdirectory);
    touch(&subdirectory, Some(time() + 20));

    assert!(
        !resource.is_fresh(time() + 10)?,
        "->isFresh() returns false if a subdirectory is modified (e.g. a file gets deleted)"
    );
    Ok(())
}

#[test]
fn testFilterRegexListNoMatch() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, Some(br"/\.(foo|xml)$/"))?;

    touch(&t.join("/new.bar"), Some(time() + 20));
    assert!(
        resource.is_fresh(time() + 10)?,
        "->isFresh() returns true if a new file not matching the filter regex is created"
    );
    Ok(())
}

#[test]
fn testFilterRegexListMatch() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, Some(br"/\.(foo|xml)$/"))?;

    touch(&t.join("/new.xml"), Some(time() + 20));
    assert!(
        !resource.is_fresh(time() + 10)?,
        "->isFresh() returns false if an new file matching the filter regex is created "
    );
    Ok(())
}

#[test]
fn testSerializeUnserialize() -> TestResult {
    let t = set_up();
    let resource = DirectoryResource::new(&t.directory, Some(br"/\.(foo|xml)$/"))?;

    let _ = DirectoryResource::from_serialized(&resource.serialize()?)?;

    assert_eq!(
        realpath(&t.directory),
        Some(resource.get_resource().to_vec())
    );
    assert_eq!(Some(&br"/\.(foo|xml)$/"[..]), resource.get_pattern());
    Ok(())
}

#[test]
fn testResourcesWithDifferentPatternsAreDifferent() -> TestResult {
    let t = set_up();
    let resource_a = DirectoryResource::new(&t.directory, Some(b"/.xml$/"))?;
    let resource_b = DirectoryResource::new(&t.directory, Some(b"/.yaml$/"))?;

    // array_unique() compares the string forms.
    assert_ne!(resource_a.to_string(), resource_b.to_string());
    Ok(())
}

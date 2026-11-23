//! ResourceCheckerConfigCacheTest.php at symfony/config v8.1.10. Names
//! and assertions are PHPUnit's; `createMock()` / `createStub()` of
//! ResourceCheckerInterface are `MockChecker`s that count their calls, and
//! an `\ArrayIterator` of checkers is any Rust iterator.
//!
//! Upstream: symfony/config Tests/ResourceCheckerConfigCacheTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{
    TestResult,
    cache::{LongNameResource, MockChecker, ResourceStub, classes, set_up, this_file},
    fs::{text, unlink},
};
use symfony_config::{
    ConfigCacheInterface, ResourceCheckerConfigCache, ResourceCheckerInterface,
    resource::{FileResource, ResourceInterface},
};

fn cache(
    file: &[u8],
    checkers: Vec<Arc<dyn ResourceCheckerInterface>>,
) -> ResourceCheckerConfigCache {
    ResourceCheckerConfigCache::new(file, checkers, None).with_classes(classes())
}

fn read(path: &[u8]) -> Vec<u8> {
    std::fs::read(common::fs::os(path)).unwrap_or_default()
}

#[test]
fn testGetPath() {
    let files = set_up();
    let cache = cache(&files.cache_file, vec![]);
    assert_eq!(cache.get_path(), files.cache_file.as_slice());
}

#[test]
fn testCacheIsNotFreshIfEmpty() -> TestResult {
    let files = set_up();
    let checker = MockChecker::stub();
    unlink(&files.cache_file);
    let mut cache = cache(&files.cache_file, vec![checker.clone()]);
    assert!(!cache.is_fresh()?);
    assert_eq!(checker.supports_calls(), 0);
    Ok(())
}

#[test]
fn testCacheIsFreshIfNoCheckerProvided() -> TestResult {
    let files = set_up();
    let mut cache = cache(&files.cache_file, vec![]);
    assert!(cache.is_fresh()?);
    Ok(())
}

#[test]
fn testCacheIsFreshIfEmptyCheckerIteratorProvided() -> TestResult {
    let files = set_up();
    let mut cache = ResourceCheckerConfigCache::new(&files.cache_file, std::iter::empty(), None);
    assert!(cache.is_fresh()?);
    Ok(())
}

#[test]
fn testResourcesWithoutcheckersAreIgnoredAndConsideredFresh() -> TestResult {
    let files = set_up();
    let mut cache = cache(&files.cache_file, vec![]);
    cache.write(
        b"",
        Some(&[Arc::new(ResourceStub::new()) as Arc<dyn ResourceInterface>]),
    )?;
    assert!(cache.is_fresh()?);
    Ok(())
}

#[test]
fn testIsFreshWithchecker() -> TestResult {
    let files = set_up();
    let checker = MockChecker::returning(true, true);
    let mut cache = cache(&files.cache_file, vec![checker.clone()]);
    cache.write(
        b"",
        Some(&[Arc::new(ResourceStub::new()) as Arc<dyn ResourceInterface>]),
    )?;
    assert!(cache.is_fresh()?);
    assert_eq!(checker.supports_calls(), 1);
    assert_eq!(checker.is_fresh_calls(), 1);
    Ok(())
}

#[test]
fn testIsNotFreshWithchecker() -> TestResult {
    let files = set_up();
    let checker = MockChecker::returning(true, false);
    let mut cache = cache(&files.cache_file, vec![checker.clone()]);
    cache.write(
        b"",
        Some(&[Arc::new(ResourceStub::new()) as Arc<dyn ResourceInterface>]),
    )?;
    assert!(!cache.is_fresh()?);
    assert_eq!(checker.supports_calls(), 1);
    assert_eq!(checker.is_fresh_calls(), 1);
    Ok(())
}

#[test]
fn testCacheIsNotFreshWhenUnserializeFails() -> TestResult {
    let files = set_up();
    let mut cache = cache(&files.cache_file, vec![MockChecker::stub()]);
    let resource = FileResource::new(&this_file("resource_checker_config_cache_test.rs"))?;
    cache.write(
        b"foo",
        Some(&[Arc::new(resource) as Arc<dyn ResourceInterface>]),
    )?;

    let mut meta_file = files.cache_file.clone();
    meta_file.extend_from_slice(b".meta");
    let content = text(&read(&meta_file)).replace("FileResource", "ClassNotHere");
    std::fs::write(common::fs::os(&meta_file), content)?;

    assert!(!cache.is_fresh()?);
    Ok(())
}

#[test]
fn testCacheKeepsContent() -> TestResult {
    let files = set_up();
    let mut cache = cache(&files.cache_file, vec![]);
    cache.write(b"FOOBAR", None)?;
    assert_eq!(read(cache.get_path()), b"FOOBAR");
    Ok(())
}

#[test]
fn testCacheIsNotFreshIfNotExistsMetaFile() -> TestResult {
    let files = set_up();
    let mut cache = cache(&files.cache_file, vec![MockChecker::stub()]);
    let resource = FileResource::new(&this_file("resource_checker_config_cache_test.rs"))?;
    cache.write(
        b"foo",
        Some(&[Arc::new(resource) as Arc<dyn ResourceInterface>]),
    )?;

    let mut meta_file = files.cache_file.clone();
    meta_file.extend_from_slice(b".meta");
    unlink(&meta_file);

    assert!(!cache.is_fresh()?);
    Ok(())
}

fn expected_json(class: &str, file: &[u8]) -> String {
    serde_json::json!({"resources": [{"@type": class, "resource": text(file)}]}).to_string()
}

#[test]
fn testCacheWithCustomMetaFile() -> TestResult {
    let files = set_up();
    assert_eq!(read(&files.meta_file), b"");

    let file = this_file("resource_checker_config_cache_test.rs");
    let mut cache = ResourceCheckerConfigCache::new(
        &files.cache_file,
        [MockChecker::stub() as Arc<dyn ResourceCheckerInterface>],
        Some(&files.meta_file),
    );
    cache.write(
        b"foo",
        Some(&[Arc::new(FileResource::new(&file)?) as Arc<dyn ResourceInterface>]),
    )?;

    assert_ne!(read(&files.meta_file), b"");

    let mut json_file = files.meta_file.clone();
    json_file.extend_from_slice(b".json");
    assert_eq!(
        text(&read(&json_file)),
        expected_json(symfony_config::resource::file_resource::CLASS, &file)
    );
    Ok(())
}

#[test]
fn testCacheWithResourceWithLongPropertyId() -> TestResult {
    let files = set_up();
    let file = this_file("resource_checker_config_cache_test.rs");
    let mut cache = cache(&files.cache_file, vec![]);
    cache.write(
        b"foo",
        Some(&[Arc::new(LongNameResource::new(&file)) as Arc<dyn ResourceInterface>]),
    )?;

    let mut meta_file = files.cache_file.clone();
    meta_file.extend_from_slice(b".meta");
    assert_ne!(read(&meta_file), b"");

    meta_file.extend_from_slice(b".json");
    assert_eq!(
        text(&read(&meta_file)),
        expected_json(common::cache::LONG_NAME, &file)
    );
    Ok(())
}

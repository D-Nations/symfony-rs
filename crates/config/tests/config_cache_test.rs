//! ConfigCacheTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; a data provider's rows run in one Rust test. The tests
//! that reset SelfCheckingResourceChecker's shared answers (upstream does
//! it through reflection) hold a lock, since Rust runs tests in parallel.
//!
//! Upstream: symfony/config Tests/ConfigCacheTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::{Arc, Mutex};

use common::{
    TestResult,
    cache::{ResourceStub, classes, set_up, this_file},
    fs::unlink,
};
use symfony_config::{
    ConfigCache, ConfigCacheInterface,
    resource::{FileResource, ResourceInterface, SelfCheckingResourceChecker},
};

static SHARED_CACHE: Mutex<()> = Mutex::new(());

fn debug_modes() -> [bool; 2] {
    [true, false]
}

fn read(path: &[u8]) -> Vec<u8> {
    std::fs::read(common::fs::os(path)).unwrap_or_default()
}

#[test]
fn testCacheIsNotValidIfNothingHasBeenCached() -> TestResult {
    for debug in debug_modes() {
        let files = set_up();
        unlink(&files.cache_file);
        let mut cache = ConfigCache::new(&files.cache_file, debug, None, None);
        assert!(!cache.is_fresh()?, "debug {debug}");
    }
    Ok(())
}

#[test]
fn testIsAlwaysFreshInProduction() -> TestResult {
    let files = set_up();
    let stale = ResourceStub::new();
    stale.set_fresh(false);
    let mut cache = ConfigCache::new(&files.cache_file, false, None, None).with_classes(classes());
    cache.write(b"", Some(&[Arc::new(stale) as Arc<dyn ResourceInterface>]))?;
    assert!(cache.is_fresh()?);
    Ok(())
}

#[test]
fn testIsFreshWhenNoResourceProvided() -> TestResult {
    for debug in debug_modes() {
        let files = set_up();
        let mut cache = ConfigCache::new(&files.cache_file, debug, None, None);
        cache.write(b"", Some(&[]))?;
        assert!(cache.is_fresh()?, "debug {debug}");
    }
    Ok(())
}

#[test]
fn testFreshResourceInDebug() -> TestResult {
    let _lock = SHARED_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    SelfCheckingResourceChecker::clear_cache();
    let files = set_up();
    let fresh = ResourceStub::new();
    fresh.set_fresh(true);
    let mut cache = ConfigCache::new(&files.cache_file, true, None, None).with_classes(classes());
    cache.write(b"", Some(&[Arc::new(fresh) as Arc<dyn ResourceInterface>]))?;
    assert!(cache.is_fresh()?);
    Ok(())
}

#[test]
fn testStaleResourceInDebug() -> TestResult {
    let _lock = SHARED_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    SelfCheckingResourceChecker::clear_cache();
    let files = set_up();
    let stale = ResourceStub::new();
    stale.set_fresh(false);
    let mut cache = ConfigCache::new(&files.cache_file, true, None, None).with_classes(classes());
    cache.write(b"", Some(&[Arc::new(stale) as Arc<dyn ResourceInterface>]))?;
    assert!(!cache.is_fresh()?);
    Ok(())
}

#[test]
fn testCacheWithCustomMetaFile() -> TestResult {
    let files = set_up();
    assert_eq!(read(&files.meta_file), b"");
    let mut cache = ConfigCache::new(&files.cache_file, false, Some(&files.meta_file), None);
    let resource = FileResource::new(&this_file("config_cache_test.rs"))?;
    cache.write(
        b"foo",
        Some(&[Arc::new(resource) as Arc<dyn ResourceInterface>]),
    )?;
    assert_ne!(read(&files.meta_file), b"");
    Ok(())
}

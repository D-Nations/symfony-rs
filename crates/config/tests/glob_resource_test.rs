//! GlobResourceTest.php at symfony/config v8.1.10. Names, `TestWith`
//! cases and assertions are PHPUnit's. Upstream's Tests/Fixtures (which
//! its tests write into) is a private copy per test;
//! `iterator_to_array($resource)` is `get_iterator()`, whose paths are
//! the array's keys. Setting `$globBrace` by reflection is
//! `set_glob_brace()`. testPhar is `n/a` (no phar reader).
//!
//! Upstream: symfony/config Tests/Resource/GlobResourceTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult,
    fs::{TempDir, mkdir, rmdir, text, touch, unlink},
};
use php_value::{Key, PhpArray, Value};
use symfony_config::resource::{GlobResource, ResourceInterface, SelfCheckingResourceInterface};

fn texts(paths: &[Vec<u8>]) -> Vec<String> {
    paths.iter().map(|p| text(p)).collect()
}

fn excluded(path: &[u8]) -> PhpArray {
    let mut a = PhpArray::new();
    a.insert(Key::from_bytes(path), Value::Bool(true));
    a
}

#[test]
fn testIterator() -> TestResult {
    for pattern in ["/Resource", "/**/Resource", "/**/Resource/"] {
        let fixtures = TempDir::fixtures();
        let dir = fixtures.path();
        let resource = GlobResource::new(&dir, pattern.as_bytes(), true, false, PhpArray::new())?;

        let paths = resource.get_iterator()?;

        let file = fixtures.join("/Resource/ConditionalClass.php");
        assert_eq!(vec![text(&file)], texts(&paths), "pattern {pattern}");
        assert_eq!(text(&dir), text(resource.get_prefix()));
    }
    Ok(())
}

#[test]
fn testIteratorForExclusionDoesntIterateThroughSubfolders() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/Exclude", true, true, PhpArray::new())?;

    let paths = resource.get_iterator()?;

    let file = fixtures.join("/Exclude");
    assert!(paths.contains(&file));
    assert_eq!(1, paths.len());
    Ok(())
}

#[test]
fn testIteratorSkipsFoldersForGivenExcludedPrefixes() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(
        &dir,
        b"/*Exclude*",
        true,
        false,
        excluded(&fixtures.join("/Exclude")),
    )?;

    let paths = resource.get_iterator()?;

    assert!(!paths.contains(&fixtures.join("/Exclude/AnExcludedFile.txt")));
    assert!(!paths.contains(&fixtures.join("/Exclude/ExcludeToo/AnotheExcludedFile.txt")));
    Ok(())
}

#[test]
fn testIteratorSkipsSubfoldersForGivenExcludedPrefixes() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(
        &dir,
        b"/*Exclude/*",
        true,
        false,
        excluded(&fixtures.join("/Exclude")),
    )?;

    let paths = resource.get_iterator()?;

    assert!(!paths.contains(&fixtures.join("/Exclude/AnExcludedFile.txt")));
    assert!(!paths.contains(&fixtures.join("/Exclude/ExcludeToo/AnotheExcludedFile.txt")));
    Ok(())
}

#[test]
fn testIteratorSkipsFoldersWithForwardSlashForGivenExcludedPrefixes() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(
        &dir,
        b"/*Exclude*",
        true,
        false,
        excluded(&fixtures.join("/Exclude")),
    )?;

    let paths = resource.get_iterator()?;

    assert!(!paths.contains(&fixtures.join("/Exclude/AnExcludedFile.txt")));
    assert!(!paths.contains(&fixtures.join("/Exclude/ExcludeToo/AnotheExcludedFile.txt")));
    Ok(())
}

#[test]
fn testIsFreshNonRecursiveDetectsNewFile() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/*", false, false, PhpArray::new())?;

    assert!(resource.is_fresh(0)?);

    mkdir(&fixtures.join("/TmpGlob"));
    assert!(resource.is_fresh(0)?);

    rmdir(&fixtures.join("/TmpGlob"));
    assert!(resource.is_fresh(0)?);

    touch(&fixtures.join("/TmpGlob"), None);
    assert!(!resource.is_fresh(0)?);

    unlink(&fixtures.join("/TmpGlob"));
    assert!(resource.is_fresh(0)?);
    Ok(())
}

#[test]
fn testIsFreshNonRecursiveDetectsRemovedFile() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/*", false, false, PhpArray::new())?;

    touch(&fixtures.join("/TmpGlob"), None);
    touch(&fixtures.join("/.TmpGlob"), None);
    assert!(resource.is_fresh(0)?);

    unlink(&fixtures.join("/.TmpGlob"));
    assert!(resource.is_fresh(0)?);

    unlink(&fixtures.join("/TmpGlob"));
    assert!(!resource.is_fresh(0)?);
    Ok(())
}

#[test]
fn testIsFreshRecursiveDetectsRemovedFile() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/*", true, false, PhpArray::new())?;

    touch(&fixtures.join("/Resource/TmpGlob"), None);
    assert!(resource.is_fresh(0)?);

    unlink(&fixtures.join("/Resource/TmpGlob"));
    assert!(!resource.is_fresh(0)?);

    touch(&fixtures.join("/Resource/TmpGlob"), None);
    assert!(resource.is_fresh(0)?);

    unlink(&fixtures.join("/Resource/.hiddenFile"));
    assert!(resource.is_fresh(0)?);
    Ok(())
}

#[test]
fn testIsFreshRecursiveDetectsNewFile() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/*", true, false, PhpArray::new())?;

    assert!(resource.is_fresh(0)?);

    touch(&fixtures.join("/Resource/TmpGlob"), None);
    assert!(!resource.is_fresh(0)?);
    Ok(())
}

#[test]
fn testBraceFallback() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let mut resource =
        GlobResource::new(&dir, b"/*{/*/*.txt,.x{m,n}l}", true, false, PhpArray::new())?;

    resource.set_glob_brace(false);

    let expected: Vec<String> = [
        "/Exclude/ExcludeToo/AnotheExcludedFile.txt",
        "/ExcludeTrailingSlash/exclude/baz.txt",
        "/Include/SubDir/ExcludeFile.txt",
        "/Include/SubDir/IncludeFile.txt",
        "/foo.xml",
    ]
    .iter()
    .map(|p| text(&fixtures.join(p)))
    .collect();

    assert_eq!(expected, texts(&resource.get_iterator()?));
    Ok(())
}

#[test]
fn testUnbalancedBraceFallback() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let mut resource =
        GlobResource::new(&dir, b"/*{/*/*.txt,.x{m,nl}", true, false, PhpArray::new())?;

    resource.set_glob_brace(false);

    assert_eq!(Vec::<String>::new(), texts(&resource.get_iterator()?));
    Ok(())
}

#[test]
fn testSerializeUnserialize() -> TestResult {
    let fixtures = TempDir::fixtures();
    let dir = fixtures.path();
    let resource = GlobResource::new(&dir, b"/Resource", true, false, PhpArray::new())?;

    let new_resource = GlobResource::from_serialized(&resource.serialize()?)?;

    assert_eq!(resource.glob_brace(), new_resource.glob_brace());
    Ok(())
}

#[test]
fn testFilePrefix() -> TestResult {
    let fixtures = TempDir::fixtures();
    let file = fixtures.join("/foo.xml");
    let resource = GlobResource::new(&file, b"/**/", true, false, PhpArray::new())?;
    assert_eq!(Vec::<String>::new(), texts(&resource.get_iterator()?));
    Ok(())
}

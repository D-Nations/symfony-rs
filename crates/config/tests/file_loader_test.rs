//! FileLoaderTest.php at symfony/config v8.1.10. Names, providers and
//! assertions are PHPUnit's. `__DIR__` (Tests/Loader) is a private
//! directory holding FileLoaderTest.php next to a copy of
//! Tests/Fixtures; the stub locator returns its answers in turn, as
//! `willReturn(...)` with several values does.
//!
//! Upstream: symfony/config Tests/Loader/FileLoaderTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{
    TestResult,
    fs::{TempDir, text, touch},
    loaders::{MockLocator, TestFileLoader},
};
use php_value::Value;
use symfony_config::{
    Class, FileLocator, Located,
    loader::{LoaderInterface, LoaderResolver},
};

fn s(v: &str) -> Value {
    Value::string(v)
}

fn paths(p: &[&str]) -> Located {
    Located::Paths(p.iter().map(|p| p.as_bytes().to_vec()).collect())
}

#[test]
fn testImportWithFileLocatorDelegation() -> TestResult {
    let locator_mock_for_additional_loader = MockLocator::new(vec![
        paths(&["path/to/file1"]),
        paths(&["path/to/file1", "path/to/file2"]),
        paths(&["path/to/file1", "path/to/file2"]),
        paths(&["path/to/file1"]),
        paths(&["path/to/file1", "path/to/file2"]),
    ]);

    let file_loader = TestFileLoader::new(Arc::new(FileLocator::default()));
    file_loader.set_supports(false);
    file_loader.file.set_current_dir(b".");

    let additional_loader = TestFileLoader::new(locator_mock_for_additional_loader);
    additional_loader.file.set_current_dir(b".");

    let loader_resolver = LoaderResolver::new(vec![
        file_loader.clone() as Arc<dyn LoaderInterface>,
        additional_loader.clone(),
    ]);
    file_loader.set_resolver(loader_resolver);

    // Default case
    assert!(file_loader.import("my_resource")?.identical(&s("path/to/file1")));

    // Check first file is imported if not already loading
    assert!(file_loader.import("my_resource")?.identical(&s("path/to/file1")));

    // Check second file is imported if first is already loading
    file_loader.add_loading("path/to/file1");
    assert!(file_loader.import("my_resource")?.identical(&s("path/to/file2")));

    // Check exception throws if first (and only available) file is
    // already loading
    match file_loader.import("my_resource") {
        Ok(_) => panic!(
            "->import() throws a FileLoaderImportCircularReferenceException if the resource is already loading"
        ),
        Err(e) => assert_eq!(
            Class::FileLoaderImportCircularReference,
            *e.class(),
            "->import() throws a FileLoaderImportCircularReferenceException if the resource is already loading"
        ),
    }

    // Check exception throws if all files are already loading
    file_loader.add_loading("path/to/file2");
    match file_loader.import("my_resource") {
        Ok(_) => panic!(
            "->import() throws a FileLoaderImportCircularReferenceException if the resource is already loading"
        ),
        Err(e) => assert_eq!(
            Class::FileLoaderImportCircularReference,
            *e.class(),
            "->import() throws a FileLoaderImportCircularReferenceException if the resource is already loading"
        ),
    }
    Ok(())
}

#[test]
fn testImportWithGlobLikeResource() -> TestResult {
    let locator_mock = MockLocator::new(vec![Located::Path(Vec::new())]);
    let loader = TestFileLoader::new(locator_mock.clone());

    assert!(loader.import("[foo]")?.identical(&s("[foo]")));
    assert_eq!(1, locator_mock.count());
    Ok(())
}

#[test]
fn testImportWithGlobLikeResourceWhichContainsSlashes() -> TestResult {
    let locator_mock = MockLocator::new(vec![Located::Path(Vec::new())]);
    let loader = TestFileLoader::new(locator_mock.clone());

    assert!(loader.import("foo/bar[foo]")?.is_null());
    assert_eq!(1, locator_mock.count());
    Ok(())
}

#[test]
fn testImportWithGlobLikeResourceWhichContainsMultipleLines() -> TestResult {
    let loader = TestFileLoader::new(Arc::new(FileLocator::default()));

    assert!(
        loader
            .import("foo\nfoobar[foo]")?
            .identical(&s("foo\nfoobar[foo]"))
    );
    Ok(())
}

#[test]
fn testImportWithGlobLikeResourceWhichContainsSlashesAndMultipleLines() -> TestResult {
    let loader = TestFileLoader::new(Arc::new(FileLocator::default()));

    assert!(
        loader
            .import("foo\nfoo/bar[foo]")?
            .identical(&s("foo\nfoo/bar[foo]"))
    );
    Ok(())
}

#[test]
fn testImportWithNoGlobMatch() -> TestResult {
    let locator_mock = MockLocator::new(vec![Located::Path(Vec::new())]);
    let loader = TestFileLoader::new(locator_mock.clone());

    assert!(loader.import("./*.abc")?.is_null());
    assert_eq!(1, locator_mock.count());
    Ok(())
}

/// Upstream's Tests/: Loader/FileLoaderTest.php and Fixtures/; returns
/// the directory, `__DIR__` and `__FILE__`.
fn tests_dir() -> (TempDir, Vec<u8>, Vec<u8>) {
    let dir = TempDir::new();
    dir.copy_fixtures_to("Fixtures");
    let loader_dir = dir.join("/Loader");
    std::fs::create_dir_all(common::fs::os(&loader_dir)).ok();
    let mut file = loader_dir.clone();
    file.extend_from_slice(b"/FileLoaderTest.php");
    touch(&file, None);
    (dir, loader_dir, file)
}

/// `__DIR__.$suffix`.
fn under(dir: &[u8], suffix: &str) -> Vec<u8> {
    let mut p = dir.to_vec();
    p.extend_from_slice(suffix.as_bytes());
    p
}

#[test]
fn testImportWithSimpleGlob() -> TestResult {
    let (_dir, loader_dir, file) = tests_dir();
    let loader = TestFileLoader::new(Arc::new(FileLocator::new([&loader_dir])));

    assert!(loader.import("FileLoaderTest.*")?.identical(&Value::string(file)));
    Ok(())
}

fn import_with_exclude_provider() -> Vec<(&'static str, &'static str, usize)> {
    vec![
        ("Include/*", "/Include/{ExcludeFile.txt}", 2),
        ("Include/", "/Include/{ExcludeFile.txt}", 4),
        ("Include", "/Include/{ExcludeFile.txt}", 4),
        ("Include/**/*", "/Include/{ExcludeFile.txt}", 4),
        ("Include/*", "/Include/{Exclude*.txt}", 2),
        ("Include/", "/Include/{Exclude*.txt}", 4),
        ("Include", "/Include/{Exclude*.txt}", 4),
        ("Include/**/*", "/Include/{Exclude*.txt}", 4),
        ("Include/", "/Include/**/{ExcludeFile.txt}", 3),
        ("Include", "/Include/**/{ExcludeFile.txt}", 3),
        ("Include/**/*", "/Include/**/{ExcludeFile.txt}", 3),
        ("Include/", "/Include/**/{Exclude*.txt}", 3),
        ("Include", "/Include/**/{Exclude*.txt}", 3),
        ("Include/**/*", "/Include/**/{Exclude*.txt}", 3),
    ]
}

/// `assertCount()` of what `import()` returns (an array, or one value).
fn count(value: &Value) -> usize {
    match value {
        Value::Array(a) => a.len(),
        _ => 1,
    }
}

fn contains(value: &Value, needle: &str) -> bool {
    match value {
        Value::Array(a) => a.values().any(|v| v.identical(&s(needle))),
        other => other.identical(&s(needle)),
    }
}

#[test]
fn testImportWithExclude() -> TestResult {
    for (include, exclude, expected_count) in import_with_exclude_provider() {
        let (_dir, loader_dir, _) = tests_dir();
        let fixtures = under(&loader_dir, "/../Fixtures");
        let loader = TestFileLoader::new(Arc::new(FileLocator::new([&fixtures])));
        let exclude = Value::string(under(&fixtures, exclude));
        let loaded_files = loader
            .file
            .import(&s(include), None, false, None, &exclude)?;
        assert_eq!(expected_count, count(&loaded_files), "{include} {}", php_value_text(&exclude));
        assert!(!contains(&loaded_files, "ExcludeFile.txt"));
    }
    Ok(())
}

fn exclude_trailing_slash_consistency_provider() -> Vec<&'static str> {
    vec![
        "/Exclude/ExcludeToo/",
        "/Exclude/ExcludeToo",
        "/Exclude/ExcludeToo/*",
        "/*/ExcludeToo",
        "/*/ExcludeToo/",
        "/Exclude/ExcludeToo/*",
        "/Exclude/ExcludeToo/AnotheExcludedFile.txt",
    ]
}

#[test]
fn testExcludeTrailingSlashConsistency() -> TestResult {
    for exclude in exclude_trailing_slash_consistency_provider() {
        let (_dir, loader_dir, _) = tests_dir();
        let fixtures = under(&loader_dir, "/../Fixtures");
        let loader = TestFileLoader::new(Arc::new(FileLocator::new([&fixtures])));
        let loaded_files = loader.file.import(
            &s("ExcludeTrailingSlash/*"),
            None,
            false,
            None,
            &Value::string(under(&fixtures, exclude)),
        )?;
        assert_eq!(2, count(&loaded_files), "{exclude}");
        assert!(!contains(&loaded_files, "baz.txt"));
    }
    Ok(())
}

fn php_value_text(v: &Value) -> String {
    match v {
        Value::String(s) => text(s.as_bytes()),
        _ => String::new(),
    }
}

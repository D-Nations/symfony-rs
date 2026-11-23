//! FileLocatorTest.php at symfony/config v8.1.10. Names, providers and
//! assertions are PHPUnit's. Upstream's `__DIR__` (Tests/, holding
//! FileLocatorTest.php and Fixtures/) is a temporary directory laid out
//! the same way; `isAbsolutePath()`, reached by reflection upstream, is
//! public here.
//!
//! Upstream: symfony/config Tests/FileLocatorTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult, expect_class, expect_exception,
    fs::{TempDir, touch},
};
use symfony_config::{Class, FileLocator, FileLocatorInterface, Located};

fn get_is_absolute_path_tests() -> Vec<&'static str> {
    vec![
        "/foo.xml",
        "c:\\\\foo.xml",
        "c:/foo.xml",
        "\\server\\foo.xml",
        "https://server/foo.xml",
        "phar://server/foo.xml",
        "phar:///server/foo.xml",
    ]
}

#[test]
fn testIsAbsolutePath() {
    for path in get_is_absolute_path_tests() {
        assert!(
            FileLocator::is_absolute_path(path.as_bytes()),
            "->isAbsolutePath() returns true for an absolute path: {path}"
        );
    }
}

/// Upstream's Tests directory: FileLocatorTest.php, Fixtures/foo.xml and
/// Fixtures/Again/foo.xml.
fn tests_dir() -> TempDir {
    let dir = TempDir::new();
    touch(&dir.join("/FileLocatorTest.php"), None);
    std::fs::create_dir_all(common::fs::os(&dir.join("/Fixtures/Again"))).ok();
    touch(&dir.join("/Fixtures/foo.xml"), None);
    touch(&dir.join("/Fixtures/Again/foo.xml"), None);
    dir
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn testLocate() -> TestResult {
    let tests = tests_dir();
    let dir = s(&tests.path());
    let fixtures = format!("{dir}/Fixtures");
    let again = format!("{dir}/Fixtures/Again");
    let loader = FileLocator::new([&fixtures]);

    assert_eq!(
        Located::Path(format!("{dir}/FileLocatorTest.php").into_bytes()),
        loader.locate(b"FileLocatorTest.php", Some(dir.as_bytes()), true)?,
        "->locate() returns the absolute filename if the file exists in the given path"
    );

    assert_eq!(
        Located::Path(format!("{fixtures}/foo.xml").into_bytes()),
        loader.locate(b"foo.xml", Some(dir.as_bytes()), true)?,
        "->locate() returns the absolute filename if the file exists in one of the paths given in the constructor"
    );

    assert_eq!(
        Located::Path(format!("{fixtures}/foo.xml").into_bytes()),
        loader.locate(format!("{fixtures}/foo.xml").as_bytes(), Some(dir.as_bytes()), true)?,
        "->locate() returns the absolute filename if the file exists in one of the paths given in the constructor"
    );

    let both = Located::Paths(vec![
        format!("{fixtures}/foo.xml").into_bytes(),
        format!("{again}/foo.xml").into_bytes(),
    ]);
    let loader = FileLocator::new([&fixtures, &again]);

    assert_eq!(
        both,
        loader.locate(b"foo.xml", Some(dir.as_bytes()), false)?,
        "->locate() returns an array of absolute filenames"
    );

    assert_eq!(
        both,
        loader.locate(b"foo.xml", Some(fixtures.as_bytes()), false)?,
        "->locate() returns an array of absolute filenames"
    );

    let loader = FileLocator::new([&again]);

    assert_eq!(
        both,
        loader.locate(b"foo.xml", Some(fixtures.as_bytes()), false)?,
        "->locate() returns an array of absolute filenames"
    );
    Ok(())
}

#[test]
fn testLocateThrowsAnExceptionIfTheFileDoesNotExists() {
    let tests = tests_dir();
    let dir = s(&tests.path());
    let loader = FileLocator::new([format!("{dir}/Fixtures")]);

    expect_exception(
        loader.locate(b"foobar.xml", Some(dir.as_bytes()), true),
        Class::FileLocatorFileNotFound,
        "The file \"foobar.xml\" does not exist",
    );
}

#[test]
fn testLocateThrowsAnExceptionIfTheFileDoesNotExistsInAbsolutePath() {
    let tests = tests_dir();
    let dir = s(&tests.path());
    let loader = FileLocator::new([format!("{dir}/Fixtures")]);

    expect_class(
        loader.locate(
            format!("{dir}/Fixtures/foobar.xml").as_bytes(),
            Some(dir.as_bytes()),
            true,
        ),
        Class::FileLocatorFileNotFound,
    );
}

#[test]
fn testLocateEmpty() {
    let tests = tests_dir();
    let dir = s(&tests.path());
    let loader = FileLocator::new([format!("{dir}/Fixtures")]);

    expect_exception(
        loader.locate(b"", Some(dir.as_bytes()), true),
        Class::InvalidArgument,
        "An empty file name is not valid to be located.",
    );
}

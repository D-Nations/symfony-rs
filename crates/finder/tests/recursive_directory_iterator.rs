//! RecursiveDirectoryIteratorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's. testRewindOnFtp and testSeekOnFtp are network integration
//! tests on PHP's ftp:// wrapper (`n/a` in the manifest).
//!
//! Upstream: symfony/finder Tests/Iterator/RecursiveDirectoryIteratorTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, fixture::upstream_tests};
use symfony_finder::iterator::{Flags, RecursiveDirectoryIterator};

#[test]
fn testTrailingDirectorySeparatorIsStripped() -> TestResult {
    let fixtures_directory = format!("{}/Iterator/../Fixtures/", upstream_tests());
    let mut actual = Vec::new();
    for file in
        RecursiveDirectoryIterator::new(fixtures_directory.as_bytes(), Flags::default(), false)?
    {
        actual.push(String::from_utf8_lossy(file?.get_pathname()).into_owned());
    }
    actual.sort();
    let expected: Vec<String> = [
        ".dot",
        "A",
        "copy",
        "dolor.txt",
        "gitignore",
        "ipsum.txt",
        "lorem.txt",
        "one",
        "r+e.gex[c]a(r)s",
        "with space",
    ]
    .iter()
    .map(|f| format!("{fixtures_directory}{f}"))
    .collect();
    assert_eq!(expected, actual);
    Ok(())
}

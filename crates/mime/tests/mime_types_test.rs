//! MimeTypesTest.php at symfony/mime
//! 8d1232052c9cdef170ed9608b3147bca4f61ff69, with the tests it inherits
//! from AbstractMimeTypeGuesserTestCase (`common`). Upstream requires
//! the fileinfo extension for the class; here the default guessers are
//! the finfo port (PLAN.md decision 18), which is always supported, and
//! the `file` command.
//!
//! Upstream: symfony/mime Tests/MimeTypesTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{TestResult, fixture};
use symfony_mime::{Error, MimeTypeGuesser, MimeTypes, Result};

fn guesser() -> MimeTypes {
    MimeTypes::default()
}

/// `#[RequiresPhpExtension('fileinfo')]`: a guesser that runs here.
fn requires_guesser(types: &MimeTypes) -> bool {
    common::skipped_unsupported(types)
}

#[test]
fn testGuessWithLeadingDash() -> TestResult {
    common::guess_with_leading_dash(&guesser())
}

#[test]
fn testGuessImageWithoutExtension() -> TestResult {
    common::guess_image_without_extension(&guesser())
}

#[test]
fn testGuessImageWithDirectory() {
    common::guess_image_with_directory(&guesser())
}

#[test]
fn testGuessImageWithKnownExtension() -> TestResult {
    common::guess_image_with_known_extension(&guesser())
}

#[test]
fn testGuessFileWithUnknownExtension() -> TestResult {
    common::guess_file_with_unknown_extension(&guesser())
}

#[test]
fn testGuessWithDuplicatedFileType() -> TestResult {
    common::guess_with_duplicated_file_type(&guesser())
}

#[test]
fn testGuessWithIncorrectPath() {
    common::guess_with_incorrect_path(&guesser())
}

#[test]
fn testGuessWithNonReadablePath() -> TestResult {
    common::guess_with_non_readable_path(&guesser())
}

/// The anonymous class: never supported, never asked.
#[derive(Debug)]
struct Unsupported;

impl MimeTypeGuesser for Unsupported {
    fn is_guesser_supported(&self) -> bool {
        false
    }

    fn guess_mime_type(&self, _path: &[u8]) -> Result<Option<String>> {
        Err(Error::new(
            symfony_mime::Class::Runtime,
            "Should never be called.",
        ))
    }
}

#[test]
fn testUnsupportedGuesser() -> TestResult {
    let mut guesser = guesser();
    if requires_guesser(&guesser) {
        return Ok(());
    }
    guesser.register_guesser(Arc::new(Unsupported));
    assert_eq!(
        Some("image/gif".to_string()),
        guesser.guess_mime_type(&fixture("mimetypes/test"))?
    );
    Ok(())
}

#[test]
fn testGetExtensions() {
    let mt = MimeTypes::default();
    assert_eq!(vec!["mbox"], mt.get_extensions("application/mbox"));
    assert_eq!(
        vec!["ai", "eps", "ps"],
        mt.get_extensions("application/postscript")
    );
    assert!(mt.get_extensions("image/svg+xml").contains(&"svg".into()));
    assert!(mt.get_extensions("image/svg").contains(&"svg".into()));
    assert!(mt.get_extensions("application/whatever-symfony").is_empty());
}

#[test]
fn testGetMimeTypes() {
    let mt = MimeTypes::default();
    assert_eq!(vec!["application/mbox"], mt.get_mime_types("mbox"));
    assert!(
        mt.get_mime_types("ai")
            .contains(&"application/postscript".into())
    );
    assert!(
        mt.get_mime_types("ps")
            .contains(&"application/postscript".into())
    );
    assert!(mt.get_mime_types("svg").contains(&"image/svg+xml".into()));
    assert!(mt.get_mime_types("svg").contains(&"image/svg".into()));
    assert!(mt.get_mime_types("symfony").is_empty());
}

#[test]
fn testCustomMimeTypes() {
    let mt = MimeTypes::new([("text/bar", vec!["foo"]), ("text/baz", vec!["foo", "moof"])]);
    assert!(mt.get_mime_types("foo").contains(&"text/bar".into()));
    assert!(mt.get_mime_types("foo").contains(&"text/baz".into()));
    assert_eq!(vec!["foo", "moof"], mt.get_extensions("text/baz"));
}

#[test]
fn testCsvExtension() -> TestResult {
    let mt = MimeTypes::default();
    if requires_guesser(&mt) {
        return Ok(());
    }
    let mime = mt
        .guess_mime_type(&fixture("mimetypes/abc.csv"))?
        .unwrap_or_default();
    assert!(
        ["application/csv", "text/csv"].contains(&mime.as_str()),
        "{mime}"
    );
    assert_eq!(vec!["csv"], mt.get_extensions(&mime));
    Ok(())
}

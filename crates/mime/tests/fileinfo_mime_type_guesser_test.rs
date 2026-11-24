//! FileinfoMimeTypeGuesserTest.php at symfony/mime
//! 8d1232052c9cdef170ed9608b3147bca4f61ff69: the tests it inherits from
//! AbstractMimeTypeGuesserTestCase (`common`), on the finfo port
//! (`pure-magic` with `magic-db`; PLAN.md decision 18).
//!
//! Upstream: symfony/mime Tests/FileinfoMimeTypeGuesserTest.php
#![allow(non_snake_case)]

mod common;

use common::TestResult;
use symfony_mime::FileinfoMimeTypeGuesser;

fn guesser() -> FileinfoMimeTypeGuesser {
    FileinfoMimeTypeGuesser::new()
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

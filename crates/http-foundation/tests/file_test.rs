//! FileTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Moves run on a copy of the
//! fixtures (`files::Scratch`). The MIME tests need a guesser (upstream:
//! `#[RequiresPhpExtension('fileinfo')]`).
//!
//! Upstream: symfony/http-foundation Tests/File/FileTest.php
#![allow(non_snake_case)]

mod files;

use files::{Scratch, TestResult, exists, fixture, realpath, skipped_without_guesser};
use symfony_http_foundation::file::File;

#[test]
fn testGetMimeTypeUsesMimeTypeGuessers() -> TestResult {
    if skipped_without_guesser() {
        return Ok(());
    }
    let file = File::new(fixture("test.gif"), true)?;
    assert_eq!(Some("image/gif".to_string()), file.get_mime_type()?);
    Ok(())
}

#[test]
fn testGuessExtensionWithoutGuesser() -> TestResult {
    if skipped_without_guesser() {
        return Ok(());
    }
    let file = File::new(fixture("directory/.empty"), true)?;
    assert_eq!(None, file.guess_extension()?);
    Ok(())
}

#[test]
fn testGuessExtensionIsBasedOnMimeType() -> TestResult {
    if skipped_without_guesser() {
        return Ok(());
    }
    let file = File::new(fixture("test"), true)?;
    assert_eq!(Some("gif".to_string()), file.guess_extension()?);
    Ok(())
}

#[test]
fn testConstructWhenFileNotExists() {
    let error = File::new(fixture("not_here"), true).expect_err("FileNotFoundException");
    assert!(
        error.is_a("Symfony\\Component\\HttpFoundation\\File\\Exception\\FileNotFoundException"),
        "{error}"
    );
}

#[test]
fn testMove() -> TestResult {
    let dir = Scratch::new("file-move")?;
    let path = dir.path("test.copy.gif");
    let target_dir = dir.path("directory");
    let target_path = format!("{target_dir}/test.copy.gif");
    std::fs::copy(dir.path("test.gif"), &path)?;

    let file = File::new(path.clone(), true)?;
    let moved = file.move_to(target_dir.as_bytes(), None)?;

    assert!(exists(&target_path));
    assert!(!exists(&path));
    assert_eq!(realpath(&target_path), moved.get_real_path());
    Ok(())
}

#[test]
fn testMoveWithNewName() -> TestResult {
    let dir = Scratch::new("file-move-new-name")?;
    let path = dir.path("test.copy.gif");
    let target_dir = dir.path("directory");
    let target_path = format!("{target_dir}/test.newname.gif");
    std::fs::copy(dir.path("test.gif"), &path)?;

    let file = File::new(path.clone(), true)?;
    let moved = file.move_to(target_dir.as_bytes(), Some(b"test.newname.gif"))?;

    assert!(exists(&target_path));
    assert!(!exists(&path));
    assert_eq!(realpath(&target_path), moved.get_real_path());
    Ok(())
}

#[test]
fn testGetContent() -> TestResult {
    let this = format!("{}/tests/file_test.rs", env!("CARGO_MANIFEST_DIR"));
    let file = File::new(this.clone(), true)?;
    assert_eq!(std::fs::read(&this)?, file.get_content()?);
    Ok(())
}

/// `getFilenameFixtures()`.
const FILENAME_FIXTURES: [(&str, &str); 6] = [
    ("original.gif", "original.gif"),
    ("..\\..\\original.gif", "original.gif"),
    ("../../original.gif", "original.gif"),
    ("файлfile.gif", "файлfile.gif"),
    ("..\\..\\файлfile.gif", "файлfile.gif"),
    ("../../файлfile.gif", "файлfile.gif"),
];

#[test]
fn testMoveWithNonLatinName() -> TestResult {
    for (n, (filename, sanitized)) in FILENAME_FIXTURES.iter().enumerate() {
        let dir = Scratch::new(&format!("file-non-latin-{n}"))?;
        let path = dir.path(sanitized);
        let target_dir = format!("{}/", dir.path("directory"));
        let target_path = format!("{target_dir}{sanitized}");
        std::fs::copy(dir.path("test.gif"), &path)?;

        let file = File::new(path.clone(), true)?;
        let moved = file.move_to(target_dir.as_bytes(), Some(filename.as_bytes()))?;

        assert!(exists(&target_path), "{filename}");
        assert!(!exists(&path), "{filename}");
        assert_eq!(realpath(&target_path), moved.get_real_path(), "{filename}");
    }
    Ok(())
}

#[test]
fn testMoveToAnUnexistentDirectory() -> TestResult {
    let dir = Scratch::new("file-unexistent-dir")?;
    let source_path = dir.path("test.copy.gif");
    let target_dir = dir.path("directory/sub");
    let target_path = format!("{target_dir}/test.copy.gif");
    std::fs::copy(dir.path("test.gif"), &source_path)?;

    let file = File::new(source_path.clone(), true)?;
    let moved = file.move_to(target_dir.as_bytes(), None)?;

    assert!(exists(&target_path));
    assert!(!exists(&source_path));
    assert_eq!(realpath(&target_path), moved.get_real_path());
    Ok(())
}

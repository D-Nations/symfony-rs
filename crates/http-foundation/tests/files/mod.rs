//! Fixtures for the File, UploadedFile and FileBag tests: upstream's
//! `Tests/File/Fixtures`, copied to `tests/fixtures/file`. Tests that
//! move files work on their own copy of that directory, so tests running
//! in parallel do not share `test.copy.gif`.
//!
//! Upstream: symfony/http-foundation Tests/File/FileTest.php
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `__DIR__.'/Fixtures'` (read only).
pub fn fixtures() -> String {
    format!("{}/tests/fixtures/file", env!("CARGO_MANIFEST_DIR"))
}

pub fn fixture(name: &str) -> Vec<u8> {
    format!("{}/{name}", fixtures()).into_bytes()
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// A fresh copy of the fixtures for one test, removed on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!(
            "symfony-rs-http-foundation-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(Path::new(&fixtures()), &dir)?;
        Ok(Self(dir))
    }

    /// The copy's path for `name` (`""` for the directory itself).
    pub fn path(&self, name: &str) -> String {
        if name.is_empty() {
            self.0.display().to_string()
        } else {
            format!("{}/{name}", self.0.display())
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `realpath($path)`.
pub fn realpath(path: &str) -> Option<Vec<u8>> {
    symfony_finder::fs::realpath(path.as_bytes())
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// `#[RequiresPhpExtension('fileinfo')]`: a MIME type guesser runs here
/// (the `file` command; PLAN.md decision 18). Otherwise the test is
/// skipped, as upstream without fileinfo.
pub fn skipped_without_guesser() -> bool {
    use symfony_mime::{MimeTypeGuesser, MimeTypes};
    let unsupported = !MimeTypes::get_default().is_guesser_supported();
    if unsupported {
        eprintln!("skipped: no MIME type guesser (fileinfo) here");
    }
    unsupported
}

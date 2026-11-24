//! Port of Symfony\Component\Mime\FileinfoMimeTypeGuesser.php: the MIME
//! type that PHP's `finfo` (the libmagic copy in ext/fileinfo) gives.
//!
//! PHP's libmagic is not in this port. Its place is taken by `pure-magic`,
//! a libmagic written in safe Rust, with the rules of `magic-db`, both
//! under their BSD-2-Clause option (PLAN.md decision 18, the owner's
//! option (c)). Recorded PHP 8.4 answers (`tests/data/fileinfo.json`)
//! show where the two differ; most of the differences are in text
//! files (C++, JavaScript and JSON source read as `text/plain`, or the
//! other way).
//!
//! As libmagic, and unlike `pure-magic`'s defaults:
//!
//! - the file's name and extension are not read, only its bytes (up to
//!   the first 7 MiB, as libmagic's `bytes_max`);
//! - an empty file is `application/x-empty`, and a file of one byte is
//!   `application/octet-stream` (libmagic's "very short file (no
//!   magic)").
//!
//! Upstream's `finfo` cache is one rule database per process, loaded on
//! first use. A load failure is upstream's `RuntimeException`. A file that
//! cannot be read after the `is_readable()` check gives `None`, as
//! `$finfo->file()` returns `false` there.
//!
//! Not ported: the `$magicFile` constructor parameter (a compiled
//! libmagic `.mgc` file, which `pure-magic` does not read), and paths
//! through a stream wrapper (`Class::NotPorted`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/mime FileinfoMimeTypeGuesser.php

use std::sync::OnceLock;

use pure_magic::{MagicDb, readers::DataReader};
use symfony_finder::fs;

use crate::{
    exception::{Class, Error, Result},
    mime_types::MimeTypeGuesser,
};

const CLASS: &str = "Symfony\\Component\\Mime\\FileinfoMimeTypeGuesser";

/// libmagic's answer for a file of zero bytes.
const EMPTY: &str = "application/x-empty";
/// libmagic's answer for a file of one byte, which it does not test.
const VERY_SHORT: &str = "application/octet-stream";

/// `new FileinfoMimeTypeGuesser()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileinfoMimeTypeGuesser;

/// `self::$finfoCache['']`: the rules, loaded once.
fn database() -> Result<&'static MagicDb> {
    static DB: OnceLock<std::result::Result<MagicDb, String>> = OnceLock::new();
    DB.get_or_init(|| magic_db::load().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|message| Error::new(Class::Runtime, message.clone()))
}

#[cfg(unix)]
fn open(path: &[u8]) -> std::io::Result<std::fs::File> {
    use std::os::unix::ffi::OsStrExt;
    std::fs::File::open(std::ffi::OsStr::new(std::ffi::OsStr::from_bytes(path)))
}

#[cfg(not(unix))]
fn open(path: &[u8]) -> std::io::Result<std::fs::File> {
    let path = std::str::from_utf8(path)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "not UTF-8"))?;
    std::fs::File::open(path)
}

/// `$finfo->file($path)`: `None` where PHP gives `false`.
fn finfo_file(db: &MagicDb, path: &[u8]) -> Option<String> {
    // `file://` names a local file, as for is_file()
    let local = match fs::scheme(path) {
        Some(scheme) if scheme.eq_ignore_ascii_case(b"file") => &path[7..],
        _ => path,
    };
    let file = open(local).ok()?;
    match file.metadata().ok()?.len() {
        0 => return Some(EMPTY.to_string()),
        1 => return Some(VERY_SHORT.to_string()),
        _ => {}
    }
    let mut reader = DataReader::from_file(file).ok()?;
    let magic = db.first_magic(&mut reader, None).ok()?;
    Some(magic.mime_type().to_string())
}

/// A MIME type written twice in a row (`a/ba/b`) is written once.
fn undouble(mime_type: String) -> String {
    let half = mime_type.len() / 2;
    if mime_type.len().is_multiple_of(2)
        && mime_type.is_char_boundary(half)
        && mime_type[..half] == mime_type[half..]
    {
        mime_type[..half].to_string()
    } else {
        mime_type
    }
}

impl FileinfoMimeTypeGuesser {
    /// `new FileinfoMimeTypeGuesser()`: libmagic's own rules.
    pub fn new() -> Self {
        Self
    }
}

impl MimeTypeGuesser for FileinfoMimeTypeGuesser {
    /// `function_exists('finfo_open')`: the rules are part of this crate.
    fn is_guesser_supported(&self) -> bool {
        true
    }

    fn guess_mime_type(&self, path: &[u8]) -> Result<Option<String>> {
        if !fs::is_file(path) || !fs::is_readable(path) {
            return Err(Error::new(
                Class::InvalidArgument,
                format!(
                    "The \"{}\" file does not exist or is not readable.",
                    String::from_utf8_lossy(path)
                ),
            ));
        }
        if !self.is_guesser_supported() {
            return Err(Error::new(
                Class::Logic,
                format!("The \"{CLASS}\" guesser is not supported."),
            ));
        }
        if fs::is_url(path) {
            return Err(Error::new(
                Class::NotPorted,
                format!(
                    "FileinfoMimeTypeGuesser on a stream wrapper path \"{}\"",
                    String::from_utf8_lossy(path)
                ),
            ));
        }
        let db = database()?;
        // `?: null`: an empty answer is null
        Ok(finfo_file(db, path).filter(|m| !m.is_empty()).map(undouble))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_doubled_type_once() {
        assert_eq!(undouble("a/ba/b".into()), "a/b");
        assert_eq!(undouble("a/b".into()), "a/b");
        assert_eq!(undouble("ab".into()), "ab");
        assert_eq!(undouble("aa".into()), "a");
        assert_eq!(undouble("éé".into()), "é");
    }
}

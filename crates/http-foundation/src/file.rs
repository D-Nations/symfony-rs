//! Port of Symfony\Component\HttpFoundation\File\File.php: a file in the
//! file system, with the `\SplFileInfo` methods it inherits that callers
//! use.
//!
//! Paths are bytes, as PHP's. As `\SplFileInfo`, the path keeps its text
//! without trailing `/`; nothing is cached, so a moved file is a new
//! `File`, as upstream returns. The MIME type comes from symfony/mime's
//! default `MimeTypes` (crate `symfony-mime`), whose guesser is the
//! `file` command, not PHP's `finfo` (PLAN.md decision 18).
//!
//! `move()` (here [`File::move_to`], `move` being a Rust keyword) reports
//! the warning of the failed `rename()` as a web SAPI gives it
//! ([`crate::php::html_warning`]).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation File/File.php

use symfony_filesystem::php as fs_php;
use symfony_finder::fs;
use symfony_mime::{MimeTypeGuesser, MimeTypes};

use crate::{
    exception::{Class, Error, Result},
    php::{self, DIRECTORY_SEPARATOR},
};

/// `new File($path, $checkPath)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pathname: Vec<u8>,
}

fn shown(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
}

/// `File::getName($name)`: the part after the last `/` or `\`.
pub fn get_name(name: &[u8]) -> Vec<u8> {
    let original: Vec<u8> = name
        .iter()
        .map(|&c| if c == b'\\' { b'/' } else { c })
        .collect();
    match original.iter().rposition(|&c| c == b'/') {
        Some(at) => original[at + 1..].to_vec(),
        None => original,
    }
}

impl File {
    /// `new File($path, $checkPath)`: with `check_path`, a path that is
    /// not a file is `FileNotFoundException`.
    pub fn new(path: impl Into<Vec<u8>>, check_path: bool) -> Result<Self> {
        let path = path.into();
        if check_path && !fs::is_file(&path) {
            return Err(Error::new(
                Class::FileNotFound,
                format!("The file \"{}\" does not exist", shown(&path)),
            ));
        }
        Ok(Self::unchecked(path))
    }

    /// `\SplFileInfo::__construct($path)`.
    pub(crate) fn unchecked(mut pathname: Vec<u8>) -> Self {
        while pathname.len() > 1 && pathname.ends_with(b"/") {
            pathname.pop();
        }
        Self { pathname }
    }

    /// `getPathname()`.
    pub fn get_pathname(&self) -> &[u8] {
        &self.pathname
    }

    /// `getFilename()`: the path after its last `/`.
    pub fn get_filename(&self) -> &[u8] {
        match self.pathname.iter().rposition(|&c| c == b'/') {
            Some(at) if self.pathname.len() > 1 => &self.pathname[at + 1..],
            _ => &self.pathname,
        }
    }

    /// `getBasename()`.
    pub fn get_basename(&self) -> &[u8] {
        php::basename(&self.pathname)
    }

    /// `getExtension()`.
    pub fn get_extension(&self) -> &[u8] {
        symfony_finder::php::extension(self.get_filename())
    }

    /// `getRealPath()`; `None` is PHP's `false`.
    pub fn get_real_path(&self) -> Option<Vec<u8>> {
        fs::realpath(&self.pathname)
    }

    /// `isReadable()`.
    pub fn is_readable(&self) -> bool {
        fs::is_readable(&self.pathname)
    }

    fn stat(&self, method: &str) -> Result<fs::Stat> {
        fs::stat(&self.pathname).ok_or_else(|| {
            Error::new(
                Class::PhpRuntime,
                format!(
                    "SplFileInfo::{method}(): stat failed for {}",
                    shown(&self.pathname)
                ),
            )
        })
    }

    /// `getSize()`; a file that cannot be read is PHP's RuntimeException.
    pub fn get_size(&self) -> Result<i64> {
        Ok(self.stat("getSize")?.size)
    }

    /// `getMTime()`.
    pub fn get_m_time(&self) -> Result<i64> {
        Ok(self.stat("getMTime")?.mtime)
    }

    /// `guessExtension()`: the first extension of the guessed MIME type.
    pub fn guess_extension(&self) -> Result<Option<String>> {
        let Some(mime_type) = self.get_mime_type()? else {
            // MimeTypes::getExtensions(string $mimeType) given null
            return Err(Error::new(
                Class::PhpTypeError,
                "Symfony\\Component\\Mime\\MimeTypes::getExtensions(): Argument #1 ($mimeType) must be of type string, null given",
            ));
        };
        Ok(MimeTypes::get_default()
            .get_extensions(&mime_type)
            .into_iter()
            .next())
    }

    /// `getMimeType()`: the type the default `MimeTypes` guesses.
    pub fn get_mime_type(&self) -> Result<Option<String>> {
        Ok(MimeTypes::get_default().guess_mime_type(&self.pathname)?)
    }

    /// `move($directory, $name)`: renames the file into `directory`
    /// (created if needed), under `name` without its directories, or its
    /// own name; the new file's mode is `0666 & ~umask()`.
    pub fn move_to(&self, directory: &[u8], name: Option<&[u8]>) -> Result<File> {
        let target = self.get_target_file(directory, name)?;
        let renamed = fs_php::rename(&self.pathname, target.get_pathname());
        if !renamed.value {
            return Err(self.move_failed(&target, renamed.warning.as_deref()));
        }
        chmod_moved(&target);
        Ok(target)
    }

    /// The FileException of a failed move, with the warning PHP gave.
    pub(crate) fn move_failed(&self, target: &File, warning: Option<&str>) -> Error {
        let warning = warning.map(php::html_warning).unwrap_or_default();
        Error::new(
            Class::File,
            format!(
                "Could not move the file \"{}\" to \"{}\" ({warning}).",
                shown(&self.pathname),
                shown(target.get_pathname())
            ),
        )
    }

    /// `getContent()`. A directory reads as `""`, as PHP's
    /// `file_get_contents()` gives it (with a notice).
    pub fn get_content(&self) -> Result<Vec<u8>> {
        if fs::is_dir(&self.pathname) {
            return Ok(Vec::new());
        }
        fs_php::file_get_contents(&self.pathname).map_err(|_| {
            Error::new(
                Class::File,
                format!(
                    "Could not get the content of the file \"{}\".",
                    shown(&self.pathname)
                ),
            )
        })
    }

    /// `getTargetFile($directory, $name)`.
    pub(crate) fn get_target_file(&self, directory: &[u8], name: Option<&[u8]>) -> Result<File> {
        if !fs::is_dir(directory)
            && !fs_php::mkdir_recursive(directory, 0o777).value
            && !fs::is_dir(directory)
        {
            if fs::is_file(directory) {
                return Err(Error::new(
                    Class::File,
                    format!(
                        "Unable to create the \"{}\" directory: a similarly-named file exists.",
                        shown(directory)
                    ),
                ));
            }
            return Err(Error::new(
                Class::File,
                format!("Unable to create the \"{}\" directory.", shown(directory)),
            ));
        } else if !fs_php::is_writable(directory) {
            return Err(Error::new(
                Class::File,
                format!("Unable to write in the \"{}\" directory.", shown(directory)),
            ));
        }
        // rtrim($directory, '/\\')
        let end = directory
            .iter()
            .rposition(|&c| c != b'/' && c != b'\\')
            .map_or(0, |i| i + 1);
        let mut target = directory[..end].to_vec();
        target.push(DIRECTORY_SEPARATOR);
        match name {
            None => target.extend_from_slice(self.get_basename()),
            Some(name) => target.extend_from_slice(&get_name(name)),
        }
        Ok(File::unchecked(target))
    }
}

/// `@chmod($target, 0o666 & ~umask())`.
pub(crate) fn chmod_moved(target: &File) {
    let _ = fs_php::chmod(target.get_pathname(), 0o666 & !fs_php::umask());
}

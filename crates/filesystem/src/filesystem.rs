//! Port of Symfony\Component\Filesystem\Filesystem.php: `dumpFile()`
//! with what it calls (`mkdir()`, `tempnam()`, `rename()`, `readlink()`),
//! `readFile()` and `chmod()`, on local paths and `file://` URLs.
//!
//! `dumpFile()` is upstream's atomic write: a temporary file in the
//! target's directory (`tempnam()`, mode 0600), the content, the target's
//! mode (or `0666 & ~umask()`), then `rename()` over the target; the
//! temporary file is removed when a step fails. A symbolic link is
//! followed and its target written. Content is a string (`&[u8]`); a
//! stream resource has no counterpart here.
//!
//! Other stream wrappers (`gs://`, `compress.zlib://`, user wrappers) and
//! renaming a directory (upstream's mirror() fallback) are refused with
//! [`Class::Unsupported`]. `PHP_MAXPATHLEN` is 4096 (Linux).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/filesystem Filesystem.php

use symfony_finder::fs;

use crate::{
    Class, Error,
    path::Path,
    php::{self, Boxed},
};

/// `PHP_MAXPATHLEN` on Linux.
pub const PHP_MAXPATHLEN: usize = 4096;

/// `new Filesystem()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Filesystem;

fn quoted(prefix: &str, path: &[u8], suffix: &str) -> Vec<u8> {
    let mut message = prefix.as_bytes().to_vec();
    message.extend_from_slice(path);
    message.extend_from_slice(suffix.as_bytes());
    message
}

fn with_warning(mut message: Vec<u8>, warning: Option<String>) -> Vec<u8> {
    message.extend_from_slice(warning.unwrap_or_default().as_bytes());
    message
}

/// `getSchemeAndHierarchy($filename)`.
fn scheme_and_hierarchy(filename: &[u8]) -> (Option<&[u8]>, &[u8]) {
    match filename.windows(3).position(|w| w == b"://") {
        Some(at) => (Some(&filename[..at]), &filename[at + 3..]),
        None => (None, filename),
    }
}

/// Refuses a path on a stream wrapper this port has no counterpart for.
fn local_only(path: &[u8]) -> Result<(), Error> {
    match fs::scheme(path) {
        Some(s) if !s.eq_ignore_ascii_case(b"file") => Err(Error::new(
            Class::Unsupported,
            quoted(
                "Stream wrappers other than file:// are not ported: \"",
                path,
                "\".",
            ),
        )),
        _ => Ok(()),
    }
}

impl Filesystem {
    pub fn new() -> Self {
        Self
    }

    /// `mkdir($dirs, $mode)` for one directory, recursively.
    pub fn mkdir(&self, dir: &[u8], mode: u32) -> Result<(), Error> {
        local_only(dir)?;
        if fs::is_dir(dir) {
            return Ok(());
        }
        let Boxed { value, warning } = php::mkdir_recursive(dir, mode);
        if !value && !fs::is_dir(dir) {
            return Err(Error::io(
                with_warning(quoted("Failed to create \"", dir, "\": "), warning),
                Some(dir),
            ));
        }
        Ok(())
    }

    /// `chmod($files, $mode, $umask)` (not recursive): `mode & ~umask`.
    pub fn chmod(&self, files: &[&[u8]], mode: u32, umask: u32) -> Result<(), Error> {
        for &file in files {
            local_only(file)?;
            let Boxed { value, warning } = php::chmod(file, mode & !umask);
            if !value {
                return Err(Error::io(
                    with_warning(quoted("Failed to chmod file \"", file, "\": "), warning),
                    Some(file),
                ));
            }
        }
        Ok(())
    }

    /// `rename($origin, $target, $overwrite)`.
    pub fn rename(&self, origin: &[u8], target: &[u8], overwrite: bool) -> Result<(), Error> {
        local_only(origin)?;
        local_only(target)?;
        if !overwrite && self.is_readable(target)? {
            return Err(Error::io(
                quoted(
                    "Cannot rename because the target \"",
                    target,
                    "\" already exists.",
                ),
                Some(target),
            ));
        }
        let Boxed { value, warning } = php::rename(origin, target);
        if !value {
            if fs::is_dir(origin) {
                return Err(Error::new(
                    Class::Unsupported,
                    quoted(
                        "Renaming a directory by mirroring it is not ported: \"",
                        origin,
                        "\".",
                    ),
                ));
            }
            let mut message = quoted("Cannot rename \"", origin, "\" to \"");
            message.extend_from_slice(target);
            message.extend_from_slice(b"\": ");
            return Err(Error::io(with_warning(message, warning), Some(target)));
        }
        Ok(())
    }

    /// `isReadable($filename)`.
    fn is_readable(&self, filename: &[u8]) -> Result<bool, Error> {
        let max = PHP_MAXPATHLEN - 2;
        if filename.len() > max {
            return Err(Error::io(
                format!(
                    "Could not check if file is readable because path length exceeds {max} characters."
                ),
                Some(filename),
            ));
        }
        Ok(fs::is_readable(filename))
    }

    /// `readlink($path, false)`: the target as stored, or `None` when
    /// `path` is not a link.
    pub fn readlink(&self, path: &[u8]) -> Option<Vec<u8>> {
        if !fs::is_link(path) {
            return None;
        }
        php::readlink(path)
    }

    /// `tempnam($dir, $prefix)` (no suffix) for a local directory or a
    /// `file://` one, whose scheme is kept on the result.
    pub fn tempnam(&self, dir: &[u8], prefix: &[u8]) -> Result<Vec<u8>, Error> {
        let (scheme, hierarchy) = scheme_and_hierarchy(dir);
        match scheme {
            None => {}
            Some(s) if s == b"file" => {}
            Some(_) => {
                return Err(Error::new(
                    Class::Unsupported,
                    quoted(
                        "Stream wrappers other than file:// are not ported: \"",
                        dir,
                        "\".",
                    ),
                ));
            }
        }
        // rtrim(substr($prefix, 0, 63))
        let prefix = &prefix[..prefix.len().min(63)];
        let end = prefix
            .iter()
            .rposition(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0b))
            .map_or(0, |i| i + 1);
        let Boxed { value, warning } = php::tempnam(hierarchy, &prefix[..end]);
        match value {
            Some(file) => match scheme {
                Some(s) => {
                    let mut out = s.to_vec();
                    out.extend_from_slice(b"://");
                    out.extend_from_slice(&file);
                    Ok(out)
                }
                None => Ok(file),
            },
            None => Err(Error::io(
                with_warning(b"A temporary file could not be created: ".to_vec(), warning),
                None,
            )),
        }
    }

    /// `dumpFile($filename, $content)`: writes atomically.
    pub fn dump_file(&self, filename: &[u8], content: &[u8]) -> Result<(), Error> {
        local_only(filename)?;
        let dir = fs::dirname(filename);
        if fs::is_link(filename)
            && let Some(link) = self
                .readlink(filename)
                .filter(|t| !t.is_empty() && t != b"0")
        {
            let target = Path::make_absolute(&link, &dir)?;
            return self.dump_file(&target, content);
        }
        if !fs::is_dir(&dir) {
            self.mkdir(&dir, 0o777)?;
        }
        let base = basename(filename);
        let tmp = self.tempnam(&dir, &base)?;
        let result = (|| {
            let Boxed { value, warning } = php::file_put_contents(&tmp, content);
            if value.is_none() {
                return Err(Error::io(
                    with_warning(quoted("Failed to write file \"", filename, "\": "), warning),
                    Some(filename),
                ));
            }
            let mode = php::fileperms(filename)
                .filter(|&m| m != 0)
                .unwrap_or(0o666 & !php::umask());
            php::chmod(&tmp, mode);
            self.rename(&tmp, filename, true)
        })();
        if fs::file_exists(&tmp) {
            if cfg!(windows) && !php::is_writable(&tmp) {
                php::chmod(&tmp, php::fileperms(&tmp).unwrap_or(0) | 0o200);
            }
            php::unlink(&tmp);
        }
        result
    }

    /// `readFile($filename)`.
    pub fn read_file(&self, filename: &[u8]) -> Result<Vec<u8>, Error> {
        local_only(filename)?;
        if fs::is_dir(filename) {
            return Err(Error::io(
                quoted(
                    "Failed to read file \"",
                    filename,
                    "\": File is a directory.",
                ),
                None,
            ));
        }
        php::file_get_contents(filename).map_err(|warning| {
            Error::io(
                with_warning(
                    quoted("Failed to read file \"", filename, "\": "),
                    Some(warning),
                ),
                Some(filename),
            )
        })
    }
}

/// PHP's `basename($path)` (no suffix), with `/` as the separator.
pub fn basename(path: &[u8]) -> Vec<u8> {
    php_value::path::basename(path).to_vec()
}

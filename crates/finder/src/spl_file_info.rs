//! Port of Symfony\Component\Finder\SplFileInfo.php, with the parts of
//! PHP's `\SplFileInfo` it inherits that Finder uses. The file's status
//! is read once, on first use, as PHP's stat cache serves repeated calls.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder SplFileInfo.php

use std::{fmt, sync::OnceLock};

use crate::{exception::Error, fs, php};

/// A file found by Finder, with its path relative to the searched
/// directory.
#[derive(Clone)]
pub struct SplFileInfo {
    pathname: Vec<u8>,
    relative_path: Vec<u8>,
    relative_pathname: Vec<u8>,
    stat: OnceLock<Option<fs::Stat>>,
    lstat: OnceLock<Option<fs::Stat>>,
}

impl fmt::Debug for SplFileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplFileInfo")
            .field("pathname", &String::from_utf8_lossy(&self.pathname))
            .field(
                "relative_pathname",
                &String::from_utf8_lossy(&self.relative_pathname),
            )
            .finish()
    }
}

impl PartialEq for SplFileInfo {
    fn eq(&self, other: &Self) -> bool {
        self.pathname == other.pathname
            && self.relative_path == other.relative_path
            && self.relative_pathname == other.relative_pathname
    }
}

impl fmt::Display for SplFileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.pathname))
    }
}

impl SplFileInfo {
    /// `new SplFileInfo($file, $relativePath, $relativePathname)`.
    pub fn new(
        file: impl Into<Vec<u8>>,
        relative_path: impl Into<Vec<u8>>,
        relative_pathname: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            pathname: file.into(),
            relative_path: relative_path.into(),
            relative_pathname: relative_pathname.into(),
            stat: OnceLock::new(),
            lstat: OnceLock::new(),
        }
    }

    /// A PHP `\SplFileInfo` turned into this class as Finder::append()
    /// does: `new SplFileInfo($path, $file->getPath(), $path)`.
    pub fn from_path(path: impl Into<Vec<u8>>) -> Self {
        let path = path.into();
        let dir = php_get_path(&path).to_vec();
        Self::new(path.clone(), dir, path)
    }

    /// `getRelativePath()`: the relative path, without the file name.
    pub fn get_relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    /// `getRelativePathname()`: the relative path with the file name.
    pub fn get_relative_pathname(&self) -> &[u8] {
        &self.relative_pathname
    }

    /// `getPathname()`.
    pub fn get_pathname(&self) -> &[u8] {
        &self.pathname
    }

    /// `getPath()`: the path without the file name.
    pub fn get_path(&self) -> &[u8] {
        php_get_path(&self.pathname)
    }

    /// `getFilename()`.
    pub fn get_filename(&self) -> &[u8] {
        let path = self.get_path();
        if path.is_empty() {
            &self.pathname
        } else {
            let start = (path.len() + 1).min(self.pathname.len());
            &self.pathname[start..]
        }
    }

    /// `getExtension()`.
    pub fn get_extension(&self) -> &[u8] {
        php::extension(self.get_filename())
    }

    /// `getFilenameWithoutExtension()`.
    pub fn get_filename_without_extension(&self) -> &[u8] {
        php::filename_without_extension(self.get_filename())
    }

    /// `getRealPath()`; None is PHP's `false`.
    pub fn get_real_path(&self) -> Option<Vec<u8>> {
        fs::realpath(&self.pathname)
    }

    fn stat(&self) -> Option<fs::Stat> {
        *self.stat.get_or_init(|| fs::stat(&self.pathname))
    }

    fn lstat(&self) -> Option<fs::Stat> {
        *self.lstat.get_or_init(|| fs::lstat(&self.pathname))
    }

    pub fn is_dir(&self) -> bool {
        self.stat().is_some_and(|s| s.is_dir())
    }

    pub fn is_file(&self) -> bool {
        self.stat().is_some_and(|s| s.is_file())
    }

    pub fn is_link(&self) -> bool {
        if fs::is_url(&self.pathname) {
            return self.stat().is_some_and(|s| s.is_link());
        }
        self.lstat().is_some_and(|s| s.is_link())
    }

    pub fn is_readable(&self) -> bool {
        fs::is_readable(&self.pathname)
    }

    /// Whether the file exists (`file_exists($this->getPathname())`).
    pub fn exists(&self) -> bool {
        self.stat().is_some()
    }

    fn stat_or_throw(&self, method: &str) -> Result<fs::Stat, Error> {
        self.stat().ok_or_else(|| {
            Error::Runtime(format!(
                "SplFileInfo::{method}(): stat failed for {}",
                String::from_utf8_lossy(&self.pathname)
            ))
        })
    }

    pub fn get_size(&self) -> Result<i64, Error> {
        Ok(self.stat_or_throw("getSize")?.size)
    }

    pub fn get_m_time(&self) -> Result<i64, Error> {
        Ok(self.stat_or_throw("getMTime")?.mtime)
    }

    pub fn get_a_time(&self) -> Result<i64, Error> {
        Ok(self.stat_or_throw("getATime")?.atime)
    }

    pub fn get_c_time(&self) -> Result<i64, Error> {
        Ok(self.stat_or_throw("getCTime")?.ctime)
    }

    /// `getContents()`.
    pub fn get_contents(&self) -> Result<Vec<u8>, Error> {
        fs::read(&self.pathname).map_err(Error::Runtime)
    }
}

/// `SplFileInfo::getPath()` of a path: everything before the last `/`.
fn php_get_path(path: &[u8]) -> &[u8] {
    let mut end = path.len();
    while end > 1 && path[end - 1] == b'/' {
        end -= 1;
    }
    match path[..end].iter().rposition(|&c| c == b'/') {
        Some(at) => &path[..at],
        None => b"",
    }
}

/// What the filter iterators read from a file; Finder's files and the
/// upstream tests' mocks implement it.
pub trait FileInfo {
    fn get_pathname(&self) -> &[u8];
    fn get_filename(&self) -> &[u8];
    fn get_relative_path(&self) -> &[u8];
    fn get_relative_pathname(&self) -> &[u8];
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_link(&self) -> bool;
    fn is_readable(&self) -> bool;
    fn exists(&self) -> bool;
    fn get_size(&self) -> Result<i64, Error>;
    fn get_m_time(&self) -> Result<i64, Error>;
    /// `getContents()`; None is a mock's `null`.
    fn get_contents(&self) -> Result<Option<Vec<u8>>, Error>;
}

impl FileInfo for SplFileInfo {
    fn get_pathname(&self) -> &[u8] {
        SplFileInfo::get_pathname(self)
    }
    fn get_filename(&self) -> &[u8] {
        SplFileInfo::get_filename(self)
    }
    fn get_relative_path(&self) -> &[u8] {
        SplFileInfo::get_relative_path(self)
    }
    fn get_relative_pathname(&self) -> &[u8] {
        SplFileInfo::get_relative_pathname(self)
    }
    fn is_dir(&self) -> bool {
        SplFileInfo::is_dir(self)
    }
    fn is_file(&self) -> bool {
        SplFileInfo::is_file(self)
    }
    fn is_link(&self) -> bool {
        SplFileInfo::is_link(self)
    }
    fn is_readable(&self) -> bool {
        SplFileInfo::is_readable(self)
    }
    fn exists(&self) -> bool {
        SplFileInfo::exists(self)
    }
    fn get_size(&self) -> Result<i64, Error> {
        SplFileInfo::get_size(self)
    }
    fn get_m_time(&self) -> Result<i64, Error> {
        SplFileInfo::get_m_time(self)
    }
    fn get_contents(&self) -> Result<Option<Vec<u8>>, Error> {
        SplFileInfo::get_contents(self).map(Some)
    }
}

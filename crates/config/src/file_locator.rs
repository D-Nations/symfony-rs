//! Port of Symfony\Component\Config\FileLocator.php, with
//! FileLocatorInterface.php.
//!
//! Paths are bytes, as PHP strings are, and the filesystem is the one
//! symfony-finder presents (`symfony_finder::fs`): local paths with `/`
//! as `DIRECTORY_SEPARATOR` on every system (Linux, the oracle's), and
//! `scheme://` URLs served by registered stream wrappers.
//!
//! `isAbsolutePath()` calls `parse_url($file, PHP_URL_SCHEME)`; that
//! function is written here from PHP's answers on a recorded corpus
//! (`oracle/recorders/config_loaders.php`), not from its source.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config FileLocator.php FileLocatorInterface.php

use symfony_finder::fs;

use crate::{Class, Error, exception::file_locator_file_not_found_exception, php::implode};

/// What `locate()` returns: a path (`$first`), or every path found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Path(Vec<u8>),
    Paths(Vec<Vec<u8>>),
}

impl Located {
    /// The value as PHP holds it (`string|string[]`).
    pub fn to_value(&self) -> php_value::Value {
        use php_value::{PhpArray, Value};
        match self {
            Self::Path(p) => Value::string(p.clone()),
            Self::Paths(ps) => {
                Value::Array(PhpArray::list(ps.iter().map(|p| Value::string(p.clone()))))
            }
        }
    }
}

/// `FileLocatorInterface`.
pub trait FileLocatorInterface: Send + Sync {
    /// `locate($name, $currentPath, $first)`: the full path of a file
    /// name, or with `first == false` every match.
    fn locate(
        &self,
        name: &[u8],
        current_path: Option<&[u8]>,
        first: bool,
    ) -> Result<Located, Error>;
}

/// `FileLocator`: looks for files in pre-defined paths.
#[derive(Debug, Clone, Default)]
pub struct FileLocator {
    paths: Vec<Vec<u8>>,
}

impl FileLocator {
    /// `new FileLocator($paths)`: one path or several.
    pub fn new<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        Self {
            paths: paths.into_iter().map(|p| p.as_ref().to_vec()).collect(),
        }
    }

    /// `isAbsolutePath()` (private upstream; upstream's test reaches it
    /// by reflection).
    pub fn is_absolute_path(file: &[u8]) -> bool {
        let first = file.first().copied().unwrap_or_default();
        first == b'/'
            || first == b'\\'
            || (file.len() > 3
                && first.is_ascii_alphabetic()
                && file[1] == b':'
                && (file[2] == b'\\' || file[2] == b'/'))
            || parse_url_scheme(file).is_some_and(|s| s.as_slice() != b"0")
            // "parse_url()" doesn't handle absolute phar path, despite
            // being valid
            || file.starts_with(b"phar:///")
    }
}

impl FileLocatorInterface for FileLocator {
    fn locate(
        &self,
        name: &[u8],
        current_path: Option<&[u8]>,
        first: bool,
    ) -> Result<Located, Error> {
        if name.is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                "An empty file name is not valid to be located.",
            ));
        }

        if Self::is_absolute_path(name) {
            if !fs::file_exists(name) {
                let mut message = b"The file \"".to_vec();
                message.extend_from_slice(name);
                message.extend_from_slice(b"\" does not exist.");
                return Err(file_locator_file_not_found_exception(
                    message,
                    vec![name.to_vec()],
                ));
            }

            return Ok(Located::Path(name.to_vec()));
        }

        let mut paths = self.paths.clone();

        if let Some(current) = current_path {
            paths.insert(0, current.to_vec());
        }

        // array_unique(): the first of equal strings stays.
        let mut unique: Vec<Vec<u8>> = Vec::new();
        for p in paths {
            if !unique.contains(&p) {
                unique.push(p);
            }
        }
        let paths = unique;
        let mut filepaths = Vec::new();
        let mut notfound = Vec::new();

        for path in &paths {
            let mut file = path.clone();
            file.push(b'/');
            file.extend_from_slice(name);
            if fs::file_exists(&file) {
                if first {
                    return Ok(Located::Path(file));
                }
                filepaths.push(file);
            } else {
                notfound.push(file);
            }
        }

        if filepaths.is_empty() {
            let mut message = b"The file \"".to_vec();
            message.extend_from_slice(name);
            message.extend_from_slice(b"\" does not exist (in: \"");
            message.extend_from_slice(&implode(b"\", \"", paths.iter().cloned()));
            message.extend_from_slice(b"\").");
            return Err(file_locator_file_not_found_exception(message, notfound));
        }

        Ok(Located::Paths(filepaths))
    }
}

/// `parse_url($url, PHP_URL_SCHEME)`, shared with symfony-filesystem's
/// `Path::isAbsolute()` (see [`symfony_filesystem::php::parse_url_scheme`]).
pub use symfony_filesystem::php::parse_url_scheme;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_basics() {
        assert_eq!(
            parse_url_scheme(b"https://server/foo.xml"),
            Some(b"https".to_vec())
        );
        assert_eq!(parse_url_scheme(b"foo.xml"), None);
        assert_eq!(parse_url_scheme(b"localhost:80"), None);
    }
}

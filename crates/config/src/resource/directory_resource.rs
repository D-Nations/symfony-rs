//! Port of Symfony\Component\Config\Resource\DirectoryResource.php: a
//! directory and its contents (optionally only the files whose basename
//! matches a pattern), fresh while nothing in it is newer than the
//! timestamp.
//!
//! `isFresh()` walks the tree as `RecursiveDirectoryIterator` with
//! `SELF_FIRST` does: every entry, symbolic links to directories listed
//! but not entered, a broken link skipped. The `.` entries it also lists
//! carry the mtime of a directory already checked, so they are not
//! stated again. A directory that cannot be opened is SPL's
//! `UnexpectedValueException`. The pattern is a PHP regex run by
//! `php-pcre`; one it cannot compile matches nothing, as `preg_match()`
//! returning false does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/DirectoryResource.php

use php_value::{Key, PhpArray, Value, php_array, serialize};
use symfony_finder::fs;

use super::{ResourceInterface, SelfCheckingResourceInterface, resolve, string_property, xxh128};
use crate::{Class, Error, php};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\DirectoryResource";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryResource {
    resource: Vec<u8>,
    pattern: Option<Vec<u8>>,
}

impl DirectoryResource {
    /// `new DirectoryResource($resource, $pattern)`.
    pub fn new(resource: &[u8], pattern: Option<&[u8]>) -> Result<Self, Error> {
        match resolve(resource) {
            Some(resolved) if fs::is_dir(&resolved) => Ok(Self {
                resource: resolved,
                pattern: pattern.map(<[u8]>::to_vec),
            }),
            _ => {
                let mut message = b"The directory \"".to_vec();
                message.extend_from_slice(resource);
                message.extend_from_slice(b"\" does not exist.");
                Err(Error::new(Class::InvalidArgument, message))
            }
        }
    }

    /// The resource `unserialize()` restores from [`ResourceInterface::serialize`].
    pub fn from_serialized(data: &PhpArray) -> Result<Self, Error> {
        let pattern = match data.get(&Key::from("pattern")) {
            None | Some(Value::Null) => None,
            Some(_) => Some(string_property(data, CLASS, "pattern")?),
        };
        Ok(Self {
            resource: string_property(data, CLASS, "resource")?,
            pattern,
        })
    }

    /// `getResource()`: the canonicalized, absolute path.
    pub fn get_resource(&self) -> &[u8] {
        &self.resource
    }

    /// `getPattern()`.
    pub fn get_pattern(&self) -> Option<&[u8]> {
        self.pattern.as_deref()
    }

    fn pattern_value(&self) -> Value {
        self.pattern
            .as_ref()
            .map_or(Value::Null, |p| Value::string(p.clone()))
    }

    /// Whether nothing under `dir` is newer than `timestamp`.
    fn fresh_under(
        &self,
        dir: &[u8],
        regex: Option<&Option<php_pcre::Pcre>>,
        timestamp: i64,
    ) -> Result<bool, Error> {
        let names = fs::read_dir(dir).map_err(|reason| {
            Error::new(
                Class::UnexpectedValue,
                format!(
                    "RecursiveDirectoryIterator::__construct({}): Failed to open directory: {reason}",
                    String::from_utf8_lossy(dir)
                ),
            )
        })?;
        for name in names {
            let mut path = dir.to_vec();
            if !path.ends_with(b"/") {
                path.push(b'/');
            }
            path.extend_from_slice(&name);

            let stat = fs::stat(&path);
            let is_file = stat.is_some_and(|s| s.is_file());
            // if regex filtering is enabled only check matching files
            if let (Some(regex), true) = (regex, is_file) {
                let matched = regex
                    .as_ref()
                    .is_some_and(|r| r.is_match(&name).unwrap_or(false));
                if !matched {
                    continue;
                }
            }

            // for broken links
            let Some(stat) = stat else {
                continue;
            };

            // early return if a file's mtime exceeds the passed timestamp
            if timestamp < stat.mtime {
                return Ok(false);
            }

            // RecursiveDirectoryIterator enters directories, not links to
            // them.
            if stat.is_dir() && !fs::is_link(&path) && !self.fresh_under(&path, regex, timestamp)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl ResourceInterface for DirectoryResource {
    fn class(&self) -> &str {
        CLASS
    }

    /// `hash('xxh128', serialize([$this->resource, $this->pattern]))`.
    fn to_string(&self) -> Vec<u8> {
        let data = Value::Array(PhpArray::list([
            Value::string(self.resource.clone()),
            self.pattern_value(),
        ]));
        xxh128(serialize::serialize(&data).as_bytes())
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        Ok(php_array![
            "resource" => Value::string(self.resource.clone()),
            "pattern" => self.pattern_value(),
        ])
    }

    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl SelfCheckingResourceInterface for DirectoryResource {
    fn is_fresh(&self, timestamp: i64) -> Result<bool, Error> {
        if !fs::is_dir(&self.resource) {
            return Ok(false);
        }

        match fs::stat(&self.resource) {
            Some(s) if timestamp < s.mtime => return Ok(false),
            _ => {}
        }

        // `$this->pattern &&`: a pattern PHP finds truthy.
        let regex = self
            .pattern
            .as_ref()
            .filter(|p| php::truthy(&Value::string((*p).clone())))
            .map(|p| php_pcre::Pcre::new(p).ok());
        self.fresh_under(&self.resource, regex.as_ref(), timestamp)
    }
}

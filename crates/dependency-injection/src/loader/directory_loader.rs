//! Port of Symfony\Component\DependencyInjection\Loader\DirectoryLoader.php:
//! imports every entry of a directory, recursively.
//!
//! `scandir()` lists the entries sorted by their bytes, as PHP's default
//! order does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/DirectoryLoader.php

use std::sync::{Arc, Weak};

use php_value::Value;
use symfony_config::file_locator::FileLocatorInterface;
use symfony_config::loader::{
    FileLoader as BaseFileLoader, LoaderInterface, LoaderResolverInterface,
};
use symfony_config::{Class, Error as ConfigError};

use super::{FileLoader, IgnoreErrors, SharedContainer, cfg, lock};
use crate::container_builder::TrackContents;

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\DirectoryLoader";

/// `DirectoryLoader`.
pub struct DirectoryLoader {
    file: FileLoader,
}

impl DirectoryLoader {
    /// `new DirectoryLoader($container, $locator, $env, $prepend)`.
    pub fn new(
        container: SharedContainer,
        locator: Arc<dyn FileLocatorInterface>,
        env: Option<&str>,
        prepend: bool,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                file: FileLoader::new(container, locator, env, prepend, this),
            }
        })
    }

    /// The embedded `FileLoader`.
    pub fn file_loader(&self) -> &FileLoader {
        &self.file
    }
}

/// `scandir($path)`: the entry names with `.` and `..`, sorted.
fn scandir(path: &[u8]) -> Result<Vec<Vec<u8>>, ConfigError> {
    let shown = String::from_utf8_lossy(path);
    let failed = || {
        ConfigError::new(
            Class::TypeError,
            format!(
                "foreach() argument must be of type array|object, bool given (scandir({shown}) failed)"
            ),
        )
    };
    let dir = php_value::path::to_path(path).ok_or_else(failed)?;
    let entries = std::fs::read_dir(dir).map_err(|_| failed())?;
    let mut names = vec![b".".to_vec(), b"..".to_vec()];
    for entry in entries {
        let entry = entry.map_err(|_| failed())?;
        names.push(php_value::path::from_os_str(&entry.file_name()));
    }
    names.sort();
    Ok(names)
}

fn is_dir(path: &[u8]) -> bool {
    php_value::path::to_path(path).is_some_and(|p| p.is_dir())
}

impl LoaderInterface for DirectoryLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, ConfigError> {
        let file = symfony_config::php::string_value(resource)?;
        let mut end = file.len();
        while end > 0 && file[end - 1] == b'/' {
            end -= 1;
        }
        let file = &file[..end];
        let path = match self.file.base().get_locator().locate(file, None, true)? {
            symfony_config::Located::Path(p) => p,
            symfony_config::Located::Paths(mut ps) => ps.remove(0),
        };
        lock(self.file.container())
            .file_exists(&String::from_utf8_lossy(&path), &TrackContents::No)
            .map_err(cfg)?;

        for dir in scandir(&path)? {
            if dir.first() != Some(&b'.') {
                let mut dir = dir;
                let mut full = path.clone();
                full.push(b'/');
                full.extend_from_slice(&dir);
                if is_dir(&full) {
                    dir.push(b'/'); // append / to allow recursion
                }

                self.file.base().set_current_dir(&path);

                self.file.import(
                    &Value::string(dir),
                    None,
                    IgnoreErrors::Bool(false),
                    Some(&path),
                    &Value::Null,
                )?;
            }
        }

        Ok(Value::Null)
    }

    fn supports(&self, resource: &Value, r#type: Option<&[u8]>) -> bool {
        if r#type == Some(b"directory") {
            return true;
        }

        r#type.is_none() && matches!(resource, Value::String(s) if s.as_bytes().ends_with(b"/"))
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, ConfigError> {
        self.file.base().loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.base().loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&BaseFileLoader> {
        Some(self.file.base())
    }
}

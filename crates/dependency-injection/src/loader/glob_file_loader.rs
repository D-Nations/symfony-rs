//! Port of Symfony\Component\DependencyInjection\Loader\GlobFileLoader.php:
//! imports each file a glob pattern matches and tracks the pattern.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/GlobFileLoader.php

use std::sync::{Arc, Weak};

use php_value::{PhpArray, Value};
use symfony_config::Error as ConfigError;
use symfony_config::file_locator::FileLocatorInterface;
use symfony_config::loader::{
    FileLoader as BaseFileLoader, GlobResult, LoaderInterface, LoaderResolverInterface,
};

use super::{FileLoader, IgnoreErrors, SharedContainer, lock};

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\GlobFileLoader";

/// `GlobFileLoader`.
pub struct GlobFileLoader {
    file: FileLoader,
}

impl GlobFileLoader {
    /// `new GlobFileLoader($container, $locator, $env, $prepend)`.
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

impl LoaderInterface for GlobFileLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, ConfigError> {
        let pattern = symfony_config::php::string_value(resource)?;
        let (paths, glob_resource) =
            self.file
                .base()
                .glob(&pattern, false, false, false, PhpArray::new())?;
        for path in paths {
            self.file.import(
                &Value::string(path),
                None,
                IgnoreErrors::Bool(false),
                None,
                &Value::Null,
            )?;
        }

        let mut container = lock(self.file.container());
        match glob_resource {
            GlobResult::Glob(resource) => {
                container.add_resource(Arc::new(resource));
            }
            GlobResult::Existence(resources) => {
                for resource in resources {
                    container.add_resource(Arc::new(resource));
                }
            }
        }

        Ok(Value::Null)
    }

    fn supports(&self, _resource: &Value, r#type: Option<&[u8]>) -> bool {
        r#type == Some(b"glob")
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

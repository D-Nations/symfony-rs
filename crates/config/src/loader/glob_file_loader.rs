//! Port of Symfony\Component\Config\Loader\GlobFileLoader.php: loads the
//! `glob` type by importing the pattern.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/GlobFileLoader.php

use std::sync::{Arc, Weak};

use php_value::Value;

use super::{FileLoader, LoaderInterface, LoaderResolverInterface};
use crate::{Error, file_locator::FileLocatorInterface};

pub const CLASS: &str = "Symfony\\Component\\Config\\Loader\\GlobFileLoader";

pub struct GlobFileLoader {
    file: FileLoader,
}

impl GlobFileLoader {
    /// `new GlobFileLoader($locator, $env)`.
    pub fn new(locator: Arc<dyn FileLocatorInterface>, env: Option<&[u8]>) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            Self {
                file: FileLoader::new(locator, env, this),
            }
        })
    }
}

impl LoaderInterface for GlobFileLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, Error> {
        self.file
            .import(resource, None, false, None, &Value::Null)
    }

    fn supports(&self, _resource: &Value, r#type: Option<&[u8]>) -> bool {
        r#type == Some(b"glob")
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.file.loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&FileLoader> {
        Some(&self.file)
    }
}

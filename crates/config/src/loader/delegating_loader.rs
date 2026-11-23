//! Port of Symfony\Component\Config\Loader\DelegatingLoader.php: loads
//! with the loader its resolver selects.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/DelegatingLoader.php

use std::sync::Arc;

use php_value::Value;

use super::{Loader, LoaderInterface, LoaderResolverInterface};
use crate::{Error, exception::loader_load_exception};

pub const CLASS: &str = "Symfony\\Component\\Config\\Loader\\DelegatingLoader";

pub struct DelegatingLoader {
    loader: Loader,
}

impl DelegatingLoader {
    /// `new DelegatingLoader($resolver)`.
    pub fn new(resolver: Arc<dyn LoaderResolverInterface>) -> Arc<Self> {
        let loader = Arc::new_cyclic(|this: &std::sync::Weak<Self>| {
            let this: std::sync::Weak<dyn LoaderInterface> = this.clone();
            Self {
                loader: Loader::new(None, this),
            }
        });
        loader.loader.set_resolver(resolver);
        loader
    }

    /// The resolver (always set: upstream assigns it in the constructor).
    fn resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.loader.get_resolver()
    }
}

impl LoaderInterface for DelegatingLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, r#type: Option<&[u8]>) -> Result<Value, Error> {
        let Some(loader) = self.resolver()?.resolve(resource, r#type) else {
            return Err(loader_load_exception(resource, None, 0, None, r#type));
        };

        loader.load(resource, r#type)
    }

    fn supports(&self, resource: &Value, r#type: Option<&[u8]>) -> bool {
        self.resolver()
            .is_ok_and(|r| r.resolve(resource, r#type).is_some())
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.loader.get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.loader.set_resolver(resolver);
    }
}

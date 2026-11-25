//! Port of Symfony\Component\Config\Loader\Loader.php: the resolver a
//! loader is given, `resolve()` and `import()`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/Loader.php

use std::sync::{Arc, RwLock, Weak};

use php_value::Value;

use super::{LoaderInterface, LoaderResolverInterface};
use crate::{Class, Error, exception::loader_load_exception};

/// `abstract class Loader`, embedded in a concrete loader.
pub struct Loader {
    env: Option<Vec<u8>>,
    resolver: RwLock<Option<Arc<dyn LoaderResolverInterface>>>,
    this: Weak<dyn LoaderInterface>,
}

impl Loader {
    /// `parent::__construct($env)`; `this` is the embedding loader (from
    /// `Arc::new_cyclic`).
    pub fn new(env: Option<&[u8]>, this: Weak<dyn LoaderInterface>) -> Self {
        Self {
            env: env.map(<[u8]>::to_vec),
            resolver: RwLock::new(None),
            this,
        }
    }

    /// The protected `$env`.
    pub fn env(&self) -> Option<&[u8]> {
        self.env.as_deref()
    }

    /// The embedding loader, PHP's `$this`.
    pub fn this(&self) -> Result<Arc<dyn LoaderInterface>, Error> {
        self.this.upgrade().ok_or_else(|| {
            Error::new(
                Class::Error,
                "The loader was not created with Arc::new_cyclic",
            )
        })
    }

    /// The protected `$resolver`.
    pub fn resolver(&self) -> Option<Arc<dyn LoaderResolverInterface>> {
        self.resolver
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// `getResolver()`.
    pub fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error> {
        self.resolver()
            .ok_or_else(|| Error::new(Class::Logic, "Cannot get a resolver if none was set."))
    }

    /// `setResolver($resolver)`.
    pub fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        *self
            .resolver
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(resolver);
    }

    /// `import($resource, $type)`: imports a resource.
    pub fn import(&self, resource: &Value, r#type: Option<&[u8]>) -> Result<Value, Error> {
        self.resolve(resource, r#type)?.load(resource, r#type)
    }

    /// `resolve($resource, $type)`: this loader if it supports the
    /// resource, else the resolver's.
    pub fn resolve(
        &self,
        resource: &Value,
        r#type: Option<&[u8]>,
    ) -> Result<Arc<dyn LoaderInterface>, Error> {
        let this = self.this()?;
        if this.supports(resource, r#type) {
            return Ok(this);
        }

        let loader = self
            .resolver()
            .and_then(|resolver| resolver.resolve(resource, r#type));

        loader.ok_or_else(|| loader_load_exception(resource, None, 0, None, r#type))
    }
}

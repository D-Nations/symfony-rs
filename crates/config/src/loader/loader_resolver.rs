//! Port of Symfony\Component\Config\Loader\LoaderResolver.php: selects
//! the first loader that supports a resource.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/LoaderResolver.php

use std::sync::{Arc, RwLock, Weak};

use php_value::Value;

use super::{LoaderInterface, LoaderResolverInterface};

pub struct LoaderResolver {
    loaders: RwLock<Vec<Arc<dyn LoaderInterface>>>,
    this: Weak<LoaderResolver>,
}

impl LoaderResolver {
    /// `new LoaderResolver($loaders)`.
    pub fn new(loaders: Vec<Arc<dyn LoaderInterface>>) -> Arc<Self> {
        let resolver = Arc::new_cyclic(|this| Self {
            loaders: RwLock::new(Vec::new()),
            this: this.clone(),
        });
        for loader in loaders {
            resolver.add_loader(loader);
        }
        resolver
    }

    /// `addLoader($loader)`: appends it and sets this as its resolver.
    pub fn add_loader(&self, loader: Arc<dyn LoaderInterface>) {
        self.loaders
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Arc::clone(&loader));
        if let Some(this) = self.this.upgrade() {
            loader.set_resolver(this);
        }
    }

    /// `getLoaders()`.
    pub fn get_loaders(&self) -> Vec<Arc<dyn LoaderInterface>> {
        self.loaders
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl LoaderResolverInterface for LoaderResolver {
    fn resolve(&self, resource: &Value, r#type: Option<&[u8]>) -> Option<Arc<dyn LoaderInterface>> {
        self.get_loaders()
            .into_iter()
            .find(|loader| loader.supports(resource, r#type))
    }
}

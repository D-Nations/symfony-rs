//! Port of Symfony\Component\Config\ConfigCacheFactoryInterface.php,
//! ConfigCacheFactory.php and ResourceCheckerConfigCacheFactory.php:
//! `cache($file, $callable)` makes the cache and calls `callable` with it
//! when it is not fresh. A callable is a Rust closure by type, so
//! upstream's TypeError for a non-callable cannot occur.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config ConfigCacheFactoryInterface.php ConfigCacheFactory.php ResourceCheckerConfigCacheFactory.php

use std::sync::Arc;

use crate::{
    Error,
    config_cache::{
        ConfigCache, ConfigCacheInterface, ResourceCheckerConfigCache, ResourceCheckerInterface,
        ResourceClasses,
    },
};

/// The callback a factory calls to fill a cache that is not fresh.
pub type Fill<'a> = &'a mut dyn FnMut(&mut dyn ConfigCacheInterface) -> Result<(), Error>;

/// `ConfigCacheFactoryInterface`.
pub trait ConfigCacheFactoryInterface {
    /// `cache($file, $callable)`.
    fn cache(
        &self,
        file: &[u8],
        callable: Fill<'_>,
    ) -> Result<Box<dyn ConfigCacheInterface>, Error>;
}

/// `new ConfigCacheFactory($debug)`.
#[derive(Debug, Clone)]
pub struct ConfigCacheFactory {
    debug: bool,
    classes: ResourceClasses,
}

impl ConfigCacheFactory {
    pub fn new(debug: bool) -> Self {
        Self {
            debug,
            classes: ResourceClasses::default(),
        }
    }

    /// The classes the caches' meta files may name.
    pub fn with_classes(mut self, classes: ResourceClasses) -> Self {
        self.classes = classes;
        self
    }
}

impl ConfigCacheFactoryInterface for ConfigCacheFactory {
    fn cache(
        &self,
        file: &[u8],
        callable: Fill<'_>,
    ) -> Result<Box<dyn ConfigCacheInterface>, Error> {
        let mut cache =
            ConfigCache::new(file, self.debug, None, None).with_classes(self.classes.clone());
        if !cache.is_fresh()? {
            callable(&mut cache)?;
        }
        Ok(Box::new(cache))
    }
}

/// `new ResourceCheckerConfigCacheFactory($resourceCheckers)`.
#[derive(Clone)]
pub struct ResourceCheckerConfigCacheFactory {
    resource_checkers: Vec<Arc<dyn ResourceCheckerInterface>>,
    classes: ResourceClasses,
}

impl ResourceCheckerConfigCacheFactory {
    pub fn new(
        resource_checkers: impl IntoIterator<Item = Arc<dyn ResourceCheckerInterface>>,
    ) -> Self {
        Self {
            resource_checkers: resource_checkers.into_iter().collect(),
            classes: ResourceClasses::default(),
        }
    }

    /// The classes the caches' meta files may name.
    pub fn with_classes(mut self, classes: ResourceClasses) -> Self {
        self.classes = classes;
        self
    }
}

impl ConfigCacheFactoryInterface for ResourceCheckerConfigCacheFactory {
    fn cache(
        &self,
        file: &[u8],
        callable: Fill<'_>,
    ) -> Result<Box<dyn ConfigCacheInterface>, Error> {
        let mut cache = ResourceCheckerConfigCache::new(file, self.resource_checkers.clone(), None)
            .with_classes(self.classes.clone());
        if !cache.is_fresh()? {
            callable(&mut cache)?;
        }
        Ok(Box::new(cache))
    }
}

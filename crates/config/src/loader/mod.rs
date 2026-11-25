//! Port of Symfony\Component\Config\Loader: LoaderInterface.php,
//! LoaderResolverInterface.php, DirectoryAwareLoaderInterface.php and
//! the loaders that do not parse a format (Loader, `LoaderResolver`,
//! `DelegatingLoader`, `FileLoader`, `GlobFileLoader`), with
//! ParamConfigurator.php.
//!
//! Resources are PHP values ([`Value`]), as upstream's `mixed`. A loader
//! is an `Arc<dyn LoaderInterface>`; the abstract classes are structs a
//! concrete loader embeds ([`Loader`], [`FileLoader`]) and delegates to.
//! PHP's `$this` is a weak handle the embedding loader gives them when it
//! is created with `Arc::new_cyclic` (see [`GlobFileLoader::new`]).
//! `LoaderResolver::addLoader()` gives each loader the resolver, which
//! holds the loaders: the two keep each other alive, as in PHP, until the
//! process ends. Loaders are built once, at build time.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Loader/LoaderInterface.php Loader/LoaderResolverInterface.php Loader/DirectoryAwareLoaderInterface.php Loader/ParamConfigurator.php

use std::sync::Arc;

use php_value::Value;

use crate::Error;

pub mod delegating_loader;
pub mod file_loader;
pub mod glob_file_loader;
#[allow(clippy::module_inception)]
pub mod loader;
pub mod loader_resolver;

pub use delegating_loader::DelegatingLoader;
pub use file_loader::{FileLoader, GlobResult};
pub use glob_file_loader::GlobFileLoader;
pub use loader::Loader;
pub use loader_resolver::LoaderResolver;

/// `LoaderInterface`.
pub trait LoaderInterface: Send + Sync {
    /// The class name, as `$loader::class` gives it.
    fn class(&self) -> &str;

    /// `load($resource, $type)`.
    fn load(&self, resource: &Value, r#type: Option<&[u8]>) -> Result<Value, Error>;

    /// `supports($resource, $type)`.
    fn supports(&self, resource: &Value, r#type: Option<&[u8]>) -> bool;

    /// `getResolver()`.
    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, Error>;

    /// `setResolver($resolver)`.
    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>);

    /// `$loader instanceof FileLoader`: the embedded `FileLoader`.
    fn as_file_loader(&self) -> Option<&FileLoader> {
        None
    }

    /// `$loader instanceof DirectoryAwareLoaderInterface`.
    fn as_directory_aware(&self) -> Option<&dyn DirectoryAwareLoaderInterface> {
        None
    }
}

/// `LoaderResolverInterface`.
pub trait LoaderResolverInterface: Send + Sync {
    /// `resolve($resource, $type)`: a loader able to load the resource,
    /// or `None` (PHP's `false`).
    fn resolve(&self, resource: &Value, r#type: Option<&[u8]>) -> Option<Arc<dyn LoaderInterface>>;
}

/// `DirectoryAwareLoaderInterface`: a loader that can be given the
/// directory an import is made from.
pub trait DirectoryAwareLoaderInterface: Send + Sync {
    /// `forDirectory($currentDirectory)`.
    fn for_directory(&self, current_directory: &[u8]) -> Arc<dyn LoaderInterface>;
}

/// `ParamConfigurator`: a parameter reference, `%name%` as a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamConfigurator {
    name: Vec<u8>,
}

impl ParamConfigurator {
    pub fn new(name: impl Into<Vec<u8>>) -> Self {
        Self { name: name.into() }
    }

    /// `__toString()`.
    pub fn to_string(&self) -> Vec<u8> {
        let mut s = vec![b'%'];
        s.extend_from_slice(&self.name);
        s.push(b'%');
        s
    }
}

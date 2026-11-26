//! Port of Symfony\Component\DependencyInjection\Extension\ExtensionInterface.php.
//!
//! An extension is a shared trait object (`Arc<dyn ExtensionInterface>`);
//! `ContainerBuilder::registerExtension()` keeps the handle, so
//! `getExtension()` returns the same object (`Arc::ptr_eq`). `load()`
//! takes `&self`: an extension that keeps state keeps it behind interior
//! mutability.
//!
//! Not yet ported: `Extension`, `ConfigurationExtensionInterface`,
//! `PrependExtensionInterface` and `AbstractExtension`.
//!
//! Upstream: symfony/dependency-injection Extension/ExtensionInterface.php

use php_value::PhpArray;

use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

/// `ExtensionInterface`.
pub trait ExtensionInterface: Send + Sync {
    /// `$extension::class`.
    fn class(&self) -> &str;

    /// `load($configs, $container)`.
    fn load(&self, configs: &[PhpArray], container: &mut ContainerBuilder) -> Result<()>;

    /// `getAlias()`.
    fn get_alias(&self) -> String;
}

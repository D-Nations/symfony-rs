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

use crate::compiler::Pass;
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

    /// `$extension instanceof PrependExtensionInterface`, then
    /// `prepend($container)`: `None` for an extension that is not one.
    fn prepend(&self, _container: &mut ContainerBuilder) -> Option<Result<()>> {
        None
    }

    /// `$extension instanceof Extension`.
    fn is_extension(&self) -> bool {
        false
    }

    /// `$extension instanceof Extension`, then `getProcessedConfigs()`
    /// (which also clears them): `None` for an extension that does not
    /// extend `Extension`.
    fn get_processed_configs(&self) -> Option<Vec<PhpArray>> {
        None
    }

    /// `$extension instanceof ConfigurationExtensionInterface`, then
    /// `getConfiguration($config, $container)`: the class of the
    /// configuration object, `None` for `null` or another extension.
    fn get_configuration_class(
        &self,
        _config: &[PhpArray],
        _container: &mut ContainerBuilder,
    ) -> Option<String> {
        None
    }

    /// `$extension instanceof ConfigurationExtensionInterface` (or an
    /// extension that is a `ConfigurationInterface` itself), then
    /// `getConfiguration($config, $container)`: the configuration, `None`
    /// for `null` or another extension.
    fn get_configuration(
        &self,
        _config: &[PhpArray],
        _container: &ContainerBuilder,
    ) -> Option<Box<dyn symfony_config::definition::ConfigurationInterface>> {
        None
    }

    /// `$extension instanceof CompilerPassInterface`: the extension as a
    /// pass. An extension that is also added with `addCompilerPass()`
    /// returns that same handle, so `ExtensionCompilerPass` can tell it
    /// is in the pass list (`in_array($extension, $passes, true)`).
    fn as_compiler_pass(&self) -> Option<Pass> {
        None
    }
}

//! Port of Symfony\Component\DependencyInjection\Compiler\ExtensionCompilerPass.php:
//! a pass to automatically process extensions if they implement
//! `CompilerPassInterface`.
//!
//! An extension is a pass when [`ExtensionInterface::as_compiler_pass`]
//! returns its handle; one that is in the pass list already (the same
//! handle) runs at its own priority instead.
//!
//! Upstream: symfony/dependency-injection Compiler/ExtensionCompilerPass.php

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;
#[cfg(doc)]
use crate::extension::ExtensionInterface;

/// `ExtensionCompilerPass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExtensionCompilerPass;

impl CompilerPassInterface for ExtensionCompilerPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\ExtensionCompilerPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let passes = container.get_compiler_pass_config().get_passes();
        let extensions: Vec<_> = container.get_extensions().values().cloned().collect();
        for extension in extensions {
            let Some(pass) = extension.as_compiler_pass() else {
                continue;
            };
            if passes.iter().any(|p| p.ptr_eq(&pass)) {
                continue;
            }
            pass.process(container)?;
        }
        Ok(())
    }
}

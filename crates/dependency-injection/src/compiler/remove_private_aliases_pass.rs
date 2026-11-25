//! Port of Symfony\Component\DependencyInjection\Compiler\RemovePrivateAliasesPass.php.
//!
//! Removes the private aliases.
//!
//! Upstream: symfony/dependency-injection Compiler/RemovePrivateAliasesPass.php

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

/// `RemovePrivateAliasesPass`.
#[derive(Debug, Default)]
pub struct RemovePrivateAliasesPass;

impl RemovePrivateAliasesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\RemovePrivateAliasesPass";
}

impl CompilerPassInterface for RemovePrivateAliasesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let private: Vec<String> = container
            .get_aliases()
            .iter()
            .filter(|(_, alias)| !alias.is_public())
            .map(|(id, _)| id.to_string())
            .collect();
        for id in private {
            container.remove_alias(&id);
            container.log(
                Self::CLASS,
                &format!("Removed service \"{id}\"; reason: private alias."),
            )?;
        }
        Ok(())
    }
}

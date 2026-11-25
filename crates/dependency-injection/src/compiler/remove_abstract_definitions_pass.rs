//! Port of Symfony\Component\DependencyInjection\Compiler\RemoveAbstractDefinitionsPass.php.
//!
//! Removes the abstract definitions, after counting the env vars they
//! use (`resolveEnvPlaceholders()`).
//!
//! Upstream: symfony/dependency-injection Compiler/RemoveAbstractDefinitionsPass.php

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::definition::Definition;
use crate::exception::Result;

/// `RemoveAbstractDefinitionsPass`.
#[derive(Debug, Default)]
pub struct RemoveAbstractDefinitionsPass;

impl RemoveAbstractDefinitionsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\RemoveAbstractDefinitionsPass";
}

impl CompilerPassInterface for RemoveAbstractDefinitionsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let abstract_: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .filter(|(_, definition)| definition.is_abstract())
            .map(|(id, definition)| (id.to_string(), definition.clone()))
            .collect();
        for (id, definition) in abstract_ {
            container.resolve_env_placeholders(
                definition.to_value(),
                &EnvFormat::Placeholder,
                None,
            )?;
            container.remove_definition(&id);
            container.log(
                Self::CLASS,
                &format!("Removed service \"{id}\"; reason: abstract."),
            )?;
        }
        Ok(())
    }
}

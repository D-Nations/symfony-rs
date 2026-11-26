//! Port of Symfony\Component\DependencyInjection\Compiler\CheckReferenceValidityPass.php.
//!
//! Checks that no reference targets an abstract definition.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckReferenceValidityPass.php

use php_value::Value;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `CheckReferenceValidityPass`.
#[derive(Debug)]
pub struct CheckReferenceValidityPass {
    state: RecursivePassState,
}

impl Default for CheckReferenceValidityPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

impl CheckReferenceValidityPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckReferenceValidityPass";

    pub fn new() -> Self {
        Self::default()
    }
}

impl CompilerPassInterface for CheckReferenceValidityPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for CheckReferenceValidityPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if is_root
            && let Some(definition) = Definition::from_value(&value)
            && (definition.is_synthetic() || definition.is_abstract())
        {
            return Ok(value);
        }
        if let Some(reference) = Reference::from_value(&value)
            && container.has_definition(reference.id())
        {
            let target_definition = container.get_definition(reference.id())?;

            if target_definition.is_abstract() {
                let current_id = self.state.current_id.clone().unwrap_or_default();
                return Err(Error::runtime(format!(
                    "The definition \"{current_id}\" has a reference to an abstract definition \"{}\". Abstract definitions cannot be the target of references.",
                    reference.id()
                )));
            }
        }
        self.parent_process_value(container, value, is_root)
    }
}

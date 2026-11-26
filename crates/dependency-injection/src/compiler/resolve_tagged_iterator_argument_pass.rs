//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveTaggedIteratorArgumentPass.php:
//! resolves all `TaggedIteratorArgument` arguments.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveTaggedIteratorArgumentPass.php

use php_value::Value;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::priority_tagged_service_trait::find_and_sort_tagged_services;
use crate::argument::IteratorArgument;
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

/// `ResolveTaggedIteratorArgumentPass`.
#[derive(Debug, Clone)]
pub struct ResolveTaggedIteratorArgumentPass {
    state: RecursivePassState,
}

impl Default for ResolveTaggedIteratorArgumentPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveTaggedIteratorArgumentPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

impl CompilerPassInterface for ResolveTaggedIteratorArgumentPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveTaggedIteratorArgumentPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveTaggedIteratorArgumentPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(argument) =
            IteratorArgument::from_value(&value).filter(IteratorArgument::is_tagged)
        else {
            return self.parent_process_value(container, value, is_root);
        };

        let mut exclude = argument.exclude();
        if argument.exclude_self() {
            exclude.push(self.state.current_id.clone().unwrap_or_default());
        }
        let values = find_and_sort_tagged_services(&argument, container, &exclude)?;
        argument.set_values(values);

        Ok(value)
    }
}

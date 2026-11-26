//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveFactoryClassPass.php.
//!
//! A factory `[null, 'method']` takes the definition's class as its
//! class.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveFactoryClassPass.php

use php_value::{Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};

/// `ResolveFactoryClassPass`.
#[derive(Debug, Clone)]
pub struct ResolveFactoryClassPass {
    state: RecursivePassState,
}

impl Default for ResolveFactoryClassPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveFactoryClassPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveFactoryClassPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

impl CompilerPassInterface for ResolveFactoryClassPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveFactoryClassPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(definition) = Definition::from_value(&value)
            && let Value::Array(mut factory) = definition.factory()
            && factory.get(&Key::Int(0)).is_none_or(Value::is_null)
        {
            let Some(class) = definition.class() else {
                return Err(Error::runtime(format!(
                    "The \"{}\" service is defined to be created by a factory, but is missing the factory class. Did you forget to define the factory or service class?",
                    self.state.current_id.as_deref().unwrap_or("")
                )));
            };
            factory.insert(Key::Int(0), Value::from(class.as_str()));
            definition.set_factory(Value::Array(factory));
        }

        self.parent_process_value(container, value, is_root)
    }
}

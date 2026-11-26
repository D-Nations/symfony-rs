//! Port of Symfony\Component\DependencyInjection\Compiler\RemoveUnusedDefinitionsPass.php.
//!
//! Removes the definitions no public service or alias reaches. The env
//! vars a removed definition uses are counted as upstream counts them,
//! by resolving the placeholders of its serialized form.
//!
//! Upstream: symfony/dependency-injection Compiler/RemoveUnusedDefinitionsPass.php

use std::collections::HashSet;

use php_value::Value;

use super::abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::Result;
use crate::reference::Reference;

/// `RemoveUnusedDefinitionsPass`.
#[derive(Debug)]
pub struct RemoveUnusedDefinitionsPass {
    state: RecursivePassState,
    connected_ids: Vec<String>,
}

impl Default for RemoveUnusedDefinitionsPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            connected_ids: Vec::new(),
        }
    }
}

impl RemoveUnusedDefinitionsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\RemoveUnusedDefinitionsPass";

    pub fn new() -> Self {
        Self::default()
    }

    fn run(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.enable_expression_processing();
        let mut connected_ids: HashSet<String> = HashSet::new();

        for (_, alias) in container.get_aliases().iter() {
            if alias.is_public() {
                self.connected_ids.push(alias.id());
            }
        }

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, definition)| (id.to_string(), definition.clone()))
            .collect();
        for (id, definition) in &definitions {
            if definition.is_public() {
                connected_ids.insert(id.clone());
                self.process_value(container, definition.to_value(), false)?;
            }
        }

        while !self.connected_ids.is_empty() {
            let ids = std::mem::take(&mut self.connected_ids);
            for id in ids {
                if !connected_ids.contains(&id) && container.has_definition(&id) {
                    connected_ids.insert(id.clone());
                    let definition = container.get_definition(&id)?;
                    self.process_value(container, definition.to_value(), false)?;
                }
            }
        }

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, definition)| (id.to_string(), definition.clone()))
            .collect();
        for (id, definition) in definitions {
            if !connected_ids.contains(&id) {
                container.remove_definition(&id);
                let value = if definition.has_errors() {
                    definition.to_value()
                } else {
                    Value::String(php_value::serialize::serialize(&definition.to_value()))
                };
                container.resolve_env_placeholders(value, &EnvFormat::Placeholder, None)?;
                container.log(
                    Self::CLASS,
                    &format!("Removed service \"{id}\"; reason: unused."),
                )?;
            }
        }
        Ok(())
    }
}

impl CompilerPassInterface for RemoveUnusedDefinitionsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let result = self.run(container);
        self.connected_ids.clear();
        result
    }
}

impl AbstractRecursivePass for RemoveUnusedDefinitionsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(reference) = Reference::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        if reference.invalid_behavior() != InvalidBehavior::IgnoreUninitialized {
            self.connected_ids.push(reference.id().to_string());
        }
        Ok(value)
    }
}

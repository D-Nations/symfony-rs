//! Port of Symfony\Component\DependencyInjection\Compiler\DefinitionErrorExceptionPass.php.
//!
//! Fails with the first error of a definition that has errors and still
//! exists, unless it is only reached through references that fail at
//! run time.
//!
//! Upstream: symfony/dependency-injection Compiler/DefinitionErrorExceptionPass.php

use php_value::Value;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::Argument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `DefinitionErrorExceptionPass`.
#[derive(Debug)]
pub struct DefinitionErrorExceptionPass {
    state: RecursivePassState,
    errored_definitions: IdMap<Definition>,
    /// Target id, then source id: whether the reference fails at run time.
    source_references: IdMap<IdMap<bool>>,
}

impl Default for DefinitionErrorExceptionPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            errored_definitions: IdMap::new(),
            source_references: IdMap::new(),
        }
    }
}

impl DefinitionErrorExceptionPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\DefinitionErrorExceptionPass";

    pub fn new() -> Self {
        Self::default()
    }

    fn run(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)?;

        let mut visited_ids: IdMap<bool> = IdMap::new();

        let errored: Vec<(String, Definition)> = self
            .errored_definitions
            .iter()
            .map(|(id, definition)| (id.to_string(), definition.clone()))
            .collect();
        for (id, definition) in errored {
            if self.is_error_for_runtime(&id, &mut visited_ids) {
                continue;
            }

            // only show the first error so the user can focus on it
            let errors = definition.errors();

            return Err(Error::runtime(
                errors.into_iter().next().unwrap_or_default(),
            ));
        }
        Ok(())
    }

    /// `isErrorForRuntime($id, &$visitedIds)`.
    fn is_error_for_runtime(&self, id: &str, visited_ids: &mut IdMap<bool>) -> bool {
        let Some(sources) = self.source_references.get(id) else {
            return false;
        };

        if let Some(visited) = visited_ids.get(id) {
            return *visited;
        }

        visited_ids.insert(id, true);

        for (source_id, is_runtime) in sources.iter() {
            let runtime_source = if let Some(visited) = visited_ids.get(source_id) {
                *visited
            } else {
                let visited = self.is_error_for_runtime(source_id, visited_ids);
                visited_ids.insert(source_id, visited);
                visited
            };
            if runtime_source {
                continue;
            }

            if !is_runtime {
                return false;
            }
        }

        true
    }
}

impl CompilerPassInterface for DefinitionErrorExceptionPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let result = self.run(container);
        self.errored_definitions = IdMap::new();
        self.source_references = IdMap::new();
        result
    }
}

impl AbstractRecursivePass for DefinitionErrorExceptionPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(argument) = Argument::from_value(&value) {
            self.parent_process_value(container, Value::Array(argument.values()), false)?;
            return Ok(value);
        }

        let current_id = self.state.current_id.clone();
        if let Some(reference) = Reference::from_value(&value)
            && current_id.as_deref() != Some(reference.id())
        {
            let sources = self
                .source_references
                .get_or_insert_with(reference.id(), IdMap::new);
            let source = current_id.as_deref().unwrap_or("");
            if matches!(
                reference.invalid_behavior(),
                InvalidBehavior::RuntimeException | InvalidBehavior::IgnoreUninitialized
            ) {
                sources.get_or_insert_with(source, || true);
            } else {
                sources.insert(source, false);
            }
            return Ok(value);
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        if !definition.has_errors() || definition.has_tag("container.error") {
            return self.parent_process_value(container, value, is_root);
        }

        self.errored_definitions
            .insert(current_id.unwrap_or_default(), definition);

        self.parent_process_value(container, value, false)
    }
}

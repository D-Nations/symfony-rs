//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveHotPathPass.php.
//!
//! Propagates the `container.hot_path` tag to the services that hot
//! services reference (not through argument objects, which are lazy).
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveHotPathPass.php

use std::collections::HashSet;

use php_value::{PhpArray, Value};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::Argument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::Result;
use crate::reference::Reference;

/// `ResolveHotPathPass`.
#[derive(Debug, Clone)]
pub struct ResolveHotPathPass {
    state: RecursivePassState,
    resolved_ids: HashSet<String>,
}

impl Default for ResolveHotPathPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveHotPathPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveHotPathPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            resolved_ids: HashSet::new(),
        }
    }
}

impl CompilerPassInterface for ResolveHotPathPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let result = recursive_process(self, container).and_then(|()| {
            container
                .get_definition("service_container")?
                .clear_tag("container.hot_path");
            Ok(())
        });
        self.resolved_ids.clear();
        result
    }
}

impl AbstractRecursivePass for ResolveHotPathPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if Argument::from_value(&value).is_some() {
            return Ok(value);
        }

        if is_root && let Some(definition) = Definition::from_value(&value) {
            if definition.is_deprecated() {
                definition.clear_tag("container.hot_path");
                return Ok(value);
            }

            self.resolved_ids
                .insert(self.state.current_id.clone().unwrap_or_default());

            if !definition.has_tag("container.hot_path") {
                return Ok(value);
            }
        }

        if let Some(reference) = Reference::from_value(&value)
            && reference.invalid_behavior() != InvalidBehavior::IgnoreUninitialized
            && container.has_definition(reference.id())
        {
            let id = reference.id().to_string();
            let definition = container.get_definition(&id)?;

            if definition.is_deprecated() || definition.has_tag("container.hot_path") {
                return Ok(value);
            }

            definition.add_tag("container.hot_path", PhpArray::new());

            if self.resolved_ids.contains(&id) {
                self.parent_process_value(container, definition.to_value(), false)?;
            }

            return Ok(value);
        }

        self.parent_process_value(container, value, is_root)
    }
}

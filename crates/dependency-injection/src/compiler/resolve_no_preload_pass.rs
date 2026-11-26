//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveNoPreloadPass.php.
//!
//! Propagates the `container.no_preload` tag: every service that no
//! public service reaches without crossing a `container.no_preload` one
//! gets the tag.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveNoPreloadPass.php

use std::collections::HashSet;

use php_value::{PhpArray, Value};

use super::abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::Result;
use crate::reference::Reference;

const DO_PRELOAD_TAG: &str = ".container.do_preload";

/// `ResolveNoPreloadPass`.
#[derive(Debug, Clone)]
pub struct ResolveNoPreloadPass {
    state: RecursivePassState,
    resolved_ids: HashSet<String>,
}

impl Default for ResolveNoPreloadPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveNoPreloadPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveNoPreloadPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            resolved_ids: HashSet::new(),
        }
    }

    fn walk(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            if definition.is_public() && !self.resolved_ids.contains(&id) {
                self.resolved_ids.insert(id);
                self.process_value(container, definition.to_value(), true)?;
            }
        }

        let aliases: Vec<_> = container.get_aliases().values().cloned().collect();
        for alias in aliases {
            let id = alias.id();
            if alias.is_public()
                && !self.resolved_ids.contains(&id)
                && container.has_definition(&id)
            {
                self.resolved_ids.insert(id.clone());
                let definition = container.get_definition(&id)?;
                self.process_value(container, definition.to_value(), true)?;
            }
        }
        Ok(())
    }
}

impl CompilerPassInterface for ResolveNoPreloadPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let result = self.walk(container);
        self.resolved_ids.clear();
        result?;

        for definition in container.get_definitions().values() {
            if definition.has_tag(DO_PRELOAD_TAG) {
                definition.clear_tag(DO_PRELOAD_TAG);
            } else if !definition.is_deprecated() && !definition.has_errors() {
                definition.add_tag("container.no_preload", PhpArray::new());
            }
        }
        Ok(())
    }
}

impl AbstractRecursivePass for ResolveNoPreloadPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(reference) = Reference::from_value(&value)
            && reference.invalid_behavior() != InvalidBehavior::IgnoreUninitialized
            && container.has_definition(reference.id())
        {
            let id = reference.id().to_string();
            let definition = container.get_definition(&id)?;

            if !self.resolved_ids.contains(&id) && definition.is_private() {
                self.resolved_ids.insert(id);
                self.process_value(container, definition.to_value(), true)?;
            }

            return Ok(value);
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };

        if definition.has_tag("container.no_preload")
            || definition.is_deprecated()
            || definition.has_errors()
        {
            return Ok(value);
        }

        if is_root {
            definition.add_tag(DO_PRELOAD_TAG, PhpArray::new());
        }

        self.parent_process_value(container, value, is_root)
    }
}

//! Port of Symfony\Component\DependencyInjection\Compiler\CheckFactoryBuilderCircularReferencePass.php.
//!
//! Detects cycles where a service's factory is `[$b, $method]`, with `$b`
//! either an inlined definition or a reference to another service, and
//! `$b`'s own properties, method calls or configurator transitively
//! require that same service through constructor references. Such a
//! cycle cannot be resolved by deferring `$b`'s setup, so it fails.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckFactoryBuilderCircularReferencePass.php

use std::collections::HashSet;

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `CheckFactoryBuilderCircularReferencePass`.
#[derive(Debug, Default)]
pub struct CheckFactoryBuilderCircularReferencePass;

impl CheckFactoryBuilderCircularReferencePass {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Compiler\\CheckFactoryBuilderCircularReferencePass";
}

/// The state of one search (`$visited`, `$path`, `$currentId`).
struct Search<'a> {
    container: &'a ContainerBuilder,
    visited: HashSet<String>,
    path: Vec<String>,
    current_id: String,
}

fn resolve_alias(container: &ContainerBuilder, id: &str) -> Result<String> {
    let mut id = id.to_string();
    while container.has_alias(&id) {
        id = container.get_alias(&id)?.id();
    }
    Ok(id)
}

impl Search<'_> {
    /// `setupReferencesCurrent($value)`.
    fn setup_references_current(&mut self, value: &Value) -> Result<bool> {
        if let Value::Array(array) = value {
            for v in array.values() {
                if self.setup_references_current(v)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }

        if let Some(reference) = Reference::from_value(value) {
            let id = resolve_alias(self.container, reference.id())?;

            if id == self.current_id {
                return Ok(true);
            }

            if self.visited.contains(&id) || !self.container.has_definition(&id) {
                return Ok(false);
            }

            let def = self.container.get_definition(&id)?;

            // Lazy services break the eager-construction chain through a proxy.
            if def.is_lazy() || def.is_synthetic() {
                return Ok(false);
            }

            self.visited.insert(id.clone());
            self.path.push(id);

            // Only constructor edges propagate the "needed during construction" reachability.
            let constructor = list([Value::Array(def.arguments()), def.factory()]);
            if self.setup_references_current(&constructor)? {
                return Ok(true);
            }

            self.path.pop();
            return Ok(false);
        }

        if let Some(definition) = Definition::from_value(value) {
            // Inlined sub-definition: every part of it runs while the outer service
            // is still being constructed, so walk its full structure.
            return self.setup_references_current(&list([
                Value::Array(definition.arguments()),
                definition.factory(),
                Value::Array(definition.properties()),
                Value::Array(definition.method_calls()),
                definition.configurator(),
            ]));
        }

        Ok(false)
    }
}

fn list(values: impl IntoIterator<Item = Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

impl CompilerPassInterface for CheckFactoryBuilderCircularReferencePass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let container: &ContainerBuilder = container;
        for (id, definition) in container.get_definitions().iter() {
            let Value::Array(factory) = definition.factory() else {
                continue;
            };
            let first = factory.get(&Key::Int(0)).cloned().unwrap_or_default();

            let mut builder_id = None;
            let builder = if let Some(reference) = Reference::from_value(&first) {
                let resolved = resolve_alias(container, reference.id())?;
                if !container.has_definition(&resolved) {
                    continue;
                }
                let builder = container.get_definition(&resolved)?;
                builder_id = Some(resolved);
                builder
            } else if let Some(inline) = Definition::from_value(&first) {
                inline
            } else {
                continue;
            };

            if builder.method_calls().is_empty()
                && builder.properties().is_empty()
                && builder.configurator().is_null()
            {
                continue;
            }

            let mut search = Search {
                container,
                visited: HashSet::from([id.to_string()]),
                path: match builder_id {
                    None => vec![id.to_string()],
                    Some(builder_id) => vec![id.to_string(), builder_id],
                },
                current_id: id.to_string(),
            };

            let setup = list([
                Value::Array(builder.properties()),
                Value::Array(builder.method_calls()),
                builder.configurator(),
            ]);
            if search.setup_references_current(&setup)? {
                search.path.push(id.to_string());

                return Err(Error::service_circular_reference(id, search.path));
            }
        }
        Ok(())
    }
}

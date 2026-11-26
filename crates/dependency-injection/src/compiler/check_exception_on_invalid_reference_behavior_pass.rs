//! Port of Symfony\Component\DependencyInjection\Compiler\CheckExceptionOnInvalidReferenceBehaviorPass.php.
//!
//! Checks that every reference that must exist points to a service. The
//! anonymous pass upstream uses to name the service a missing
//! reference comes from is [`ReferenceFinder`]; it finds the reference
//! by identity, as upstream's `!==` does.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckExceptionOnInvalidReferenceBehaviorPass.php

use php_value::{Value, array::Key, string::levenshtein};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `CheckExceptionOnInvalidReferenceBehaviorPass`.
#[derive(Debug)]
pub struct CheckExceptionOnInvalidReferenceBehaviorPass {
    state: RecursivePassState,
    service_locator_context_ids: IdMap<String>,
}

impl Default for CheckExceptionOnInvalidReferenceBehaviorPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            service_locator_context_ids: IdMap::new(),
        }
    }
}

/// `EXCEPTION_ON_INVALID_REFERENCE < $behavior`.
fn is_lenient(behavior: InvalidBehavior) -> bool {
    behavior.as_int() > InvalidBehavior::Exception.as_int()
}

fn value_string(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl CheckExceptionOnInvalidReferenceBehaviorPass {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Compiler\\CheckExceptionOnInvalidReferenceBehaviorPass";

    pub fn new() -> Self {
        Self::default()
    }

    /// `throwServiceNotFoundException($ref, $sourceId, $value)`: fails
    /// when `value` holds `reference`, and returns otherwise.
    fn throw_service_not_found_exception(
        &self,
        container: &mut ContainerBuilder,
        reference: &Value,
        source_id: &str,
        value: Value,
    ) -> Result<()> {
        let id = Reference::from_value(reference)
            .map(|r| r.id().to_string())
            .unwrap_or_default();
        let current_id = self.state.current_id.as_deref();
        let mut alternatives = Vec::new();
        for known_id in container.get_service_ids() {
            if known_id.is_empty()
                || known_id.starts_with('.')
                || Some(known_id.as_str()) == current_id
            {
                continue;
            }

            let lev = levenshtein(id.as_bytes(), known_id.as_bytes());
            #[expect(clippy::cast_precision_loss, reason = "PHP's int-to-float division")]
            let close = (lev as f64) <= id.len() as f64 / 3.0;
            if close || known_id.contains(&id) {
                alternatives.push(known_id);
            }
        }

        let mut pass = ReferenceFinder {
            state: RecursivePassState::default(),
            reference: reference.clone(),
            source_id: source_id.to_string(),
            alternatives,
        };
        pass.process_value(container, value, true)?;
        Ok(())
    }
}

impl CompilerPassInterface for CheckExceptionOnInvalidReferenceBehaviorPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.service_locator_context_ids = IdMap::new();
        let tagged =
            container.find_tagged_service_ids("container.service_locator_context", false)?;
        for (id, tags) in tagged.iter() {
            let context = tags
                .get(&Key::Int(0))
                .and_then(Value::as_array)
                .and_then(|t| t.get(&Key::from("id")))
                .map(value_string)
                .unwrap_or_default();
            self.service_locator_context_ids.insert(id, context);
            container
                .get_definition(id)?
                .clear_tag("container.service_locator_context");
        }

        let result = recursive_process(self, container);
        self.service_locator_context_ids = IdMap::new();
        result
    }
}

impl AbstractRecursivePass for CheckExceptionOnInvalidReferenceBehaviorPass {
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
        if is_lenient(reference.invalid_behavior()) || container.has(reference.id()) {
            return Ok(value);
        }

        let this_current_id = self.state.current_id.clone().unwrap_or_default();
        let mut current_id = this_current_id.clone();

        if let Some(context) = self.service_locator_context_ids.get(&current_id).cloned() {
            current_id = context;
            let factory = container.get_definition(&this_current_id)?.factory();
            let locator = factory
                .as_array()
                .and_then(|f| f.get(&Key::Int(0)))
                .and_then(Definition::from_value);
            let argument = match locator {
                Some(locator) => locator.argument(0)?,
                None => Value::Null,
            };
            self.throw_service_not_found_exception(container, &value, &current_id, argument)?;
        }

        let graph = container.get_compiler().get_service_reference_graph();
        let in_edges: Vec<(Option<InvalidBehavior>, String)> = match graph.get_node(&current_id) {
            Ok(node) if current_id.starts_with('.') => graph
                .node(node)
                .get_in_edges()
                .iter()
                .map(|edge| {
                    let edge = graph.edge(*edge);
                    (
                        edge.get_value().map(Reference::invalid_behavior),
                        graph.node(edge.get_source_node()).get_id().to_string(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };
        for (behavior, source_id) in in_edges {
            if behavior.is_none_or(is_lenient) {
                continue;
            }

            if !source_id.starts_with('.') {
                current_id = source_id;
                break;
            }

            if let Some(context) = self.service_locator_context_ids.get(&source_id).cloned() {
                current_id = context;
                let locator = container.get_definition(&this_current_id)?;
                let argument = locator.argument(0)?;
                self.throw_service_not_found_exception(container, &value, &current_id, argument)?;
            }
        }

        self.throw_service_not_found_exception(container, &value, &current_id, value.clone())?;
        Ok(value)
    }
}

/// The anonymous pass of `throwServiceNotFoundException()`.
#[derive(Debug)]
struct ReferenceFinder {
    state: RecursivePassState,
    reference: Value,
    source_id: String,
    alternatives: Vec<String>,
}

impl CompilerPassInterface for ReferenceFinder {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AbstractRecursivePass@anonymous"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ReferenceFinder {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if !self.reference.identical(&value) {
            return self.parent_process_value(container, value, is_root);
        }
        let id = Reference::from_value(&value)
            .map(|r| r.id().to_string())
            .unwrap_or_default();
        let mut source_id = self.source_id.clone();
        if let Some(current_id) = &self.state.current_id
            && *current_id != id
        {
            source_id = format!("{current_id}\" in the container provided to \"{source_id}");
        }

        Err(Error::service_not_found(
            id,
            Some(source_id),
            self.alternatives.clone(),
            None,
        ))
    }
}

//! Port of Symfony\Component\DependencyInjection\Compiler\ServiceLocatorTagPass.php:
//! applies the `container.service_locator` tag by wrapping references
//! into `ServiceClosureArgument` instances.
//!
//! The static `register()` is [`ServiceLocatorTagPass::register`].
//!
//! Upstream: symfony/dependency-injection Compiler/ServiceLocatorTagPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::priority_tagged_service_trait::find_and_sort_tagged_services;
use crate::alias::Alias;
use crate::argument::{
    BoundArgument, IteratorArgument, ServiceClosureArgument, ServiceLocatorArgument,
};
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;
use crate::service_locator::SERVICE_LOCATOR_CLASS;

/// `ServiceLocatorTagPass`.
#[derive(Debug, Clone)]
pub struct ServiceLocatorTagPass {
    state: RecursivePassState,
}

impl Default for ServiceLocatorTagPass {
    fn default() -> Self {
        Self::new()
    }
}

fn into_array(value: Value) -> PhpArray {
    match value {
        Value::Array(a) => a,
        _ => PhpArray::new(),
    }
}

impl ServiceLocatorTagPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }

    /// `ServiceLocatorTagPass::register($container, $map, $callerId)`.
    pub fn register(
        container: &mut ContainerBuilder,
        map: &PhpArray,
        caller_id: Option<&str>,
    ) -> Result<Reference> {
        let mut wrapped = PhpArray::new();
        for (k, v) in map.iter() {
            wrapped.insert(k.clone(), ServiceClosureArgument::new(v.clone()).to_value());
        }

        let locator = Definition::new(Some(SERVICE_LOCATOR_CLASS), PhpArray::new());
        locator
            .add_argument(Value::Array(wrapped))
            .add_tag("container.service_locator", PhpArray::new());

        if let Some(caller_id) = caller_id
            && container.has_definition(caller_id)
        {
            locator.set_bindings(&container.get_definition(caller_id)?.bindings());
        }

        let mut id = Self::get_locator_id(container, &locator, caller_id)?;
        if !container.has_definition(&id) {
            container.set_definition(&id, locator)?;
        }

        if let Some(caller_id) = caller_id {
            let locator_id = id.clone();
            // Locators are shared when they hold the exact same list of factories;
            // to have them specialized per consumer service, we use a cloning factory
            // to derivate customized instances from the prototype one.
            id = format!("{id}.{caller_id}");
            let mut context = PhpArray::new();
            context.insert("id", Value::from(caller_id));
            container
                .register(&id, Some(SERVICE_LOCATOR_CLASS))?
                .set_factory(Value::Array(PhpArray::list([
                    Value::from(Reference::new(locator_id)),
                    Value::from("withContext"),
                ])))
                .add_tag("container.service_locator_context", context)
                .add_argument(Value::from(caller_id))
                .add_argument(Reference::new("service_container"));
        }

        Ok(Reference::new(id))
    }

    /// `getLocatorId($container, $locator, $consumerId)`.
    fn get_locator_id(
        container: &ContainerBuilder,
        locator: &Definition,
        consumer_id: Option<&str>,
    ) -> Result<String> {
        let mut values = PhpArray::new();
        let mut binding_ids = PhpArray::new();
        for (key, binding) in locator.bindings().iter() {
            let binding_values = BoundArgument::from_value(binding)
                .map(|b| b.values())
                .unwrap_or_default();
            values.insert(
                key.clone(),
                binding_values
                    .get(&Key::Int(0))
                    .cloned()
                    .unwrap_or_default(),
            );
            binding_ids.insert(
                key.clone(),
                binding_values
                    .get(&Key::Int(1))
                    .cloned()
                    .unwrap_or_default(),
            );
        }

        // The file declaring a binding and its usage-tracking id are not hashed, so that the id depends neither on the project directory nor on what else the process compiled
        let hashed = if values.is_empty() {
            locator.to_value()
        } else {
            let clone = locator.clone_object();
            clone.set_bindings(&PhpArray::new());
            Value::Array(PhpArray::list([clone.to_value(), Value::Array(values)]))
        };
        let hash = ContainerBuilder::hash(&hashed);
        let mut id = format!(".service_locator.{hash}");

        if let Some(consumer_id) = consumer_id
            && container.has_definition(&id)
        {
            let mut existing = PhpArray::new();
            for (key, binding) in container.get_definition(&id)?.bindings().iter() {
                let binding_values = BoundArgument::from_value(binding)
                    .map(|b| b.values())
                    .unwrap_or_default();
                existing.insert(
                    key.clone(),
                    binding_values
                        .get(&Key::Int(1))
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            if !Value::Array(binding_ids).identical(&Value::Array(existing)) {
                let pair = PhpArray::list([Value::from(hash), Value::from(consumer_id)]);
                id = format!(
                    ".service_locator.{}",
                    ContainerBuilder::hash(&Value::Array(pair))
                );
            }
        }

        Ok(id)
    }
}

impl CompilerPassInterface for ServiceLocatorTagPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\ServiceLocatorTagPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ServiceLocatorTagPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let current_id = self.state.current_id.clone().unwrap_or_default();

        if let Some(argument) = ServiceLocatorArgument::from_value(&value) {
            if let Some(tagged_iterator) = argument.tagged_iterator_argument() {
                let mut exclude = tagged_iterator.exclude();
                if tagged_iterator.exclude_self() {
                    exclude.push(current_id.clone());
                }
                let values = find_and_sort_tagged_services(&tagged_iterator, container, &exclude)?;
                argument.set_values(values);
            }

            let values = self.process_value(container, Value::Array(argument.values()), false)?;
            return Ok(Value::from(Self::register(
                container,
                &into_array(values),
                None,
            )?));
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        let bindings =
            self.parent_process_value(container, Value::Array(definition.bindings()), false)?;
        definition.set_bindings(&into_array(bindings));

        if !definition.has_tag("container.service_locator") {
            return self.parent_process_value(container, value, is_root);
        }

        if definition.class().is_none_or(|c| c.is_empty() || c == "0") {
            definition.set_class(Some(SERVICE_LOCATOR_CLASS));
        }

        let values = definition.arguments().get(&Key::Int(0)).cloned();
        let mut services = PhpArray::new();

        if let Some(tagged) = values
            .as_ref()
            .and_then(IteratorArgument::from_value)
            .filter(IteratorArgument::is_tagged)
        {
            for (k, v) in find_and_sort_tagged_services(&tagged, container, &[])?.iter() {
                services.insert(k.clone(), ServiceClosureArgument::new(v.clone()).to_value());
            }
        } else if let Some(Value::Array(values)) = values {
            let mut i: Option<i64> = Some(0);

            for (k, v) in values.iter() {
                if ServiceClosureArgument::from_value(v).is_some() {
                    let processed = self.process_value(container, v.clone(), false)?;
                    services.insert(k.clone(), processed);
                    continue;
                }

                let mut k = k.clone();
                if i.is_some() && k.as_int() == i {
                    if let Some(reference) = Reference::from_value(v) {
                        k = Key::from(reference.id());
                    }
                    i = i.map(|i| i + 1);
                } else if k.as_int().is_some() {
                    i = None;
                }

                let processed = self.process_value(container, v.clone(), false)?;
                services.insert(k, ServiceClosureArgument::new(processed).to_value());
            }
            if i.is_some_and(|i| usize::try_from(i).is_ok_and(|i| i == services.len())) {
                services.ksort();
            }
        } else {
            return Err(Error::invalid_argument(format!(
                "Invalid definition for service \"{current_id}\": an array of references is expected as first argument when the \"container.service_locator\" tag is set."
            )));
        }

        definition.set_argument(0, Value::Array(services));

        let id = Self::get_locator_id(container, &definition, Some(&current_id))?;

        if is_root {
            if id != current_id {
                container.set_alias(&id, Alias::new(current_id, false))?;
            }

            return Ok(value);
        }

        definition.set_public(false);
        container.set_definition(&id, definition)?;

        Ok(Value::from(Reference::new(id)))
    }
}

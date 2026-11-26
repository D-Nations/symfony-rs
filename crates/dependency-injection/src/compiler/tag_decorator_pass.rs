//! Port of Symfony\Component\DependencyInjection\Compiler\TagDecoratorPass.php:
//! turns each `container.tag_decorator` resource into one decorator per
//! service of the tag it decorates.
//!
//! Upstream's `DeepCloner` (symfony/var-exporter) copies the definition
//! with the definitions and arrays it holds; here [`deep_clone`] does
//! that for definitions, arrays and iterator arguments, and keeps the
//! other argument objects shared.
//!
//! Upstream: symfony/dependency-injection Compiler/TagDecoratorPass.php

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::IteratorArgument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};

/// `TagDecoratorPass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TagDecoratorPass;

fn deep_clone_value(value: &Value) -> Value {
    if let Value::Array(array) = value {
        let mut out = PhpArray::new();
        for (k, v) in array.iter() {
            out.insert(k.clone(), deep_clone_value(v));
        }
        return Value::Array(out);
    }
    if let Some(definition) = Definition::from_value(value) {
        return deep_clone(&definition).to_value();
    }
    if let Some(iterator) = IteratorArgument::from_value(value) {
        let clone = iterator.clone_object();
        let values = deep_clone_value(&Value::Array(iterator.values()));
        if let Value::Array(values) = values {
            clone.set_values(values);
        }
        return clone.to_value();
    }
    value.clone()
}

fn into_array(value: Value) -> PhpArray {
    match value {
        Value::Array(a) => a,
        _ => PhpArray::new(),
    }
}

/// `(new DeepCloner($definition))->clone()`.
pub fn deep_clone(definition: &Definition) -> Definition {
    let clone = definition.clone_object();
    clone.set_arguments(into_array(deep_clone_value(&Value::Array(
        definition.arguments(),
    ))));
    clone.set_properties(into_array(deep_clone_value(&Value::Array(
        definition.properties(),
    ))));
    let calls = into_array(deep_clone_value(&Value::Array(definition.method_calls())));
    if clone.set_method_calls(&calls).is_err() {
        // The calls were valid on the original.
    }
    let changes = clone.changes();
    let factory = definition.factory();
    if !factory.is_null() {
        clone.set_factory(deep_clone_value(&factory));
    }
    let configurator = definition.configurator();
    if !configurator.is_null() {
        clone.set_configurator(deep_clone_value(&configurator));
    }
    clone.set_changes(changes);
    clone
}

fn behavior_from(value: &Value) -> Option<InvalidBehavior> {
    match value {
        Value::Int(0) => Some(InvalidBehavior::RuntimeException),
        Value::Int(1) => Some(InvalidBehavior::Exception),
        Value::Int(2) => Some(InvalidBehavior::Null),
        Value::Int(3) => Some(InvalidBehavior::Ignore),
        Value::Int(4) => Some(InvalidBehavior::IgnoreUninitialized),
        _ => None,
    }
}

impl CompilerPassInterface for TagDecoratorPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\TagDecoratorPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let resources = container.find_tagged_resource_ids("container.tag_decorator", false)?;
        for (id, tags) in resources.iter() {
            let definition = container.get_definition(id)?;

            for tag in tags.values() {
                let tag = match tag {
                    Value::Array(a) => a.clone(),
                    _ => PhpArray::new(),
                };
                let decorates_tag = tag
                    .get(&Key::from("decorates_tag"))
                    .cloned()
                    .unwrap_or_default();
                if !decorates_tag.is_truthy() {
                    continue;
                }
                let decorates_tag = decorates_tag
                    .to_php_string()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

                let priority = match tag.get(&Key::from("priority")) {
                    Some(Value::Int(p)) => *p,
                    _ => 0,
                };
                let on_invalid = tag
                    .get(&Key::from("on_invalid"))
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or(Value::Int(InvalidBehavior::Exception.as_int()));
                let invalid_behavior = behavior_from(&on_invalid);
                let tagged_services = container.find_tagged_service_ids(&decorates_tag, false)?;

                if tagged_services.is_empty() {
                    if invalid_behavior == Some(InvalidBehavior::Ignore) {
                        continue;
                    }

                    if invalid_behavior == Some(InvalidBehavior::Exception) {
                        return Err(Error::service_not_found(
                            decorates_tag,
                            Some(id.to_string()),
                            Vec::new(),
                            None,
                        ));
                    }
                }

                for (tagged_service_id, _) in tagged_services.iter() {
                    let decorator = container.set_definition(
                        &format!(".decorator.{tagged_service_id}.{id}"),
                        deep_clone(&definition),
                    )?;
                    decorator
                        .clear_tag("container.tag_decorator")
                        .clear_tag("container.excluded");
                    decorator.set_decorated_service(
                        Some(tagged_service_id),
                        None,
                        priority,
                        invalid_behavior.unwrap_or_default(),
                    )?;
                }
            }

            container.remove_definition(id);
        }
        Ok(())
    }
}

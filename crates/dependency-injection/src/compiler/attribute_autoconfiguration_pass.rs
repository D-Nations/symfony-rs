//! Port of Symfony\Component\DependencyInjection\Compiler\AttributeAutoconfigurationPass.php:
//! calls the configurators registered with
//! `registerAttributeForAutoconfiguration()` for the attributes of
//! autoconfigured classes, their public methods and properties and their
//! parameters.
//!
//! Upstream reflects each configurator to read the type of its third
//! parameter; a Rust closure cannot be reflected, so the type is the
//! [`AttributeConfiguratorReflector`] it was registered with
//! (`registerAttributeForAutoconfiguration()` alone registers a
//! configurator of two parameters, called for class attributes only).
//! Configurators get the attribute metadata, not an attribute instance.
//!
//! Upstream: symfony/dependency-injection Compiler/AttributeAutoconfigurationPass.php

use std::collections::HashMap;
use std::sync::Arc;

use php_value::{Value, array::Key};

use super::abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState};
use super::autowire_required_methods_pass::class_methods;
use super::autowire_required_properties_pass::class_properties;
use super::compiler_pass_interface::CompilerPassInterface;
use crate::class_metadata::{ClassRegistry, Reflector};
use crate::container_builder::{
    AttributeConfigurator, AttributeConfiguratorReflector, ContainerBuilder,
};
use crate::definition::Definition;
use crate::exception::{Error, Result};

type Configurators = HashMap<String, Vec<AttributeConfigurator>>;

/// `AttributeAutoconfigurationPass`.
#[derive(Clone)]
pub struct AttributeAutoconfigurationPass {
    state: RecursivePassState,
    class_attribute_configurators: Configurators,
    method_attribute_configurators: Configurators,
    property_attribute_configurators: Configurators,
    parameter_attribute_configurators: Configurators,
}

impl std::fmt::Debug for AttributeAutoconfigurationPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeAutoconfigurationPass")
            .finish_non_exhaustive()
    }
}

impl Default for AttributeAutoconfigurationPass {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributeAutoconfigurationPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            class_attribute_configurators: HashMap::new(),
            method_attribute_configurators: HashMap::new(),
            property_attribute_configurators: HashMap::new(),
            parameter_attribute_configurators: HashMap::new(),
        }
    }
}

impl CompilerPassInterface for AttributeAutoconfigurationPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AttributeAutoconfigurationPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        if container.get_attribute_autoconfigurators().is_empty() {
            return Ok(());
        }

        let autoconfigurators: Vec<(String, Vec<AttributeConfigurator>)> = container
            .get_attribute_autoconfigurators()
            .iter()
            .map(|(name, callables)| (name.to_string(), callables.clone()))
            .collect();
        for (attribute_name, callables) in autoconfigurators {
            for callable in callables {
                let types = match container.attribute_configurator_reflector(&callable) {
                    AttributeConfiguratorReflector::None => {
                        self.class_attribute_configurators
                            .entry(attribute_name.clone())
                            .or_default()
                            .push(callable);
                        continue;
                    }
                    AttributeConfiguratorReflector::Typed(types) => types
                        .iter()
                        .map(|t| t.trim_start_matches('\\').to_string())
                        .collect::<Vec<_>>(),
                    AttributeConfiguratorReflector::Untyped { name, file, line } => {
                        return Err(Error::logic(format!(
                            "Argument \"${name}\" of attribute autoconfigurator should have a type, use one or more of \"\\ReflectionClass|\\ReflectionMethod|\\ReflectionProperty|\\ReflectionParameter|\\Reflector\" in \"{file}\" on line \"{line}\"."
                        )));
                    }
                };

                let only_reflector = types.len() == 1 && types[0] == "Reflector";
                for (symbol, configurators) in [
                    ("Class", &mut self.class_attribute_configurators),
                    ("Method", &mut self.method_attribute_configurators),
                    ("Property", &mut self.property_attribute_configurators),
                    ("Parameter", &mut self.parameter_attribute_configurators),
                ] {
                    if only_reflector || types.iter().any(|t| *t == format!("Reflection{symbol}")) {
                        configurators
                            .entry(attribute_name.clone())
                            .or_default()
                            .push(callable.clone());
                    }
                }
            }
        }

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            self.state.current_id = Some(id);
            self.process_value(container, definition.to_value(), true)?;
        }
        Ok(())
    }
}

/// `findConfigurators($configurators, $attributeName)`.
fn find_configurators(
    registry: &ClassRegistry,
    configurators: &mut Configurators,
    attribute_name: &str,
) -> Vec<AttributeConfigurator> {
    if let Some(found) = configurators.get(attribute_name) {
        return found.clone();
    }

    let found = match registry
        .class(attribute_name)
        .and_then(|c| c.parent.clone())
    {
        Some(parent) => find_configurators(registry, configurators, &parent),
        None => Vec::new(),
    };
    configurators.insert(attribute_name.to_string(), found.clone());
    found
}

/// `callConfigurators($configurators, $conditionals, $reflector)`.
fn call_configurators(
    registry: &ClassRegistry,
    configurators: &mut Configurators,
    conditionals: &Definition,
    reflector: &Reflector,
) -> Result<()> {
    if configurators.is_empty() {
        return Ok(());
    }

    let attributes = match reflector {
        Reflector::Class(class) => &class.attributes,
        Reflector::Method { method, .. } => &method.attributes,
        Reflector::Property { property, .. } => &property.attributes,
        Reflector::Parameter { parameter, .. } => &parameter.attributes,
    };
    for attribute in attributes {
        let name = attribute.class.trim_start_matches('\\');
        for configurator in find_configurators(registry, configurators, name) {
            configurator(conditionals, attribute, reflector)?;
        }
    }
    Ok(())
}

impl AbstractRecursivePass for AttributeAutoconfigurationPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        if !definition.is_autoconfigured()
            || (definition.is_abstract() && !definition.has_tag("container.excluded"))
            || definition.has_tag("container.ignore_attributes")
        {
            return self.parent_process_value(container, value, is_root);
        }
        let class = definition.class();
        let Some(class_reflector) = container.get_reflection_class(class.as_deref(), false)? else {
            return self.parent_process_value(container, value, is_root);
        };
        let registry = container.class_registry();

        let mut instanceof = definition.instanceof_conditionals();
        let existing = instanceof
            .get(&Key::from(class_reflector.name.as_str()))
            .and_then(Definition::from_value);
        let has_existing = existing.is_some();
        let conditionals = existing.unwrap_or_else(|| Definition::child(""));

        call_configurators(
            &registry,
            &mut self.class_attribute_configurators,
            &conditionals,
            &Reflector::Class(class_reflector.clone()),
        )?;

        if !self.parameter_attribute_configurators.is_empty() {
            let constructor_reflector = match self.get_constructor(container, &definition, false) {
                Ok(constructor) => constructor,
                Err(e)
                    if e.instance_of(
                        "Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException",
                    ) =>
                {
                    None
                }
                Err(e) => return Err(e),
            };

            if let Some(function) = constructor_reflector {
                for parameter in function.parameters() {
                    call_configurators(
                        &registry,
                        &mut self.parameter_attribute_configurators,
                        &conditionals,
                        &Reflector::Parameter {
                            function: function.clone(),
                            parameter,
                        },
                    )?;
                }
            }
        }

        if !self.method_attribute_configurators.is_empty()
            || !self.parameter_attribute_configurators.is_empty()
        {
            for (declaring, method) in class_methods(&registry, &class_reflector) {
                if !method.is_public()
                    || method.name.eq_ignore_ascii_case("__construct")
                    || method.name.eq_ignore_ascii_case("__destruct")
                {
                    continue;
                }

                let declaring_class = registry
                    .class(&declaring)
                    .unwrap_or_else(|| class_reflector.clone());
                call_configurators(
                    &registry,
                    &mut self.method_attribute_configurators,
                    &conditionals,
                    &Reflector::Method {
                        class: declaring_class,
                        method: method.clone(),
                    },
                )?;

                let function = crate::class_metadata::FunctionReflector::Method {
                    class: declaring.clone(),
                    method: method.clone(),
                };
                for parameter in &method.parameters {
                    call_configurators(
                        &registry,
                        &mut self.parameter_attribute_configurators,
                        &conditionals,
                        &Reflector::Parameter {
                            function: function.clone(),
                            parameter: parameter.clone(),
                        },
                    )?;
                }
            }
        }

        if !self.property_attribute_configurators.is_empty() {
            for property in class_properties(&registry, &class_reflector) {
                if !matches!(
                    property.visibility,
                    crate::class_metadata::Visibility::Public
                ) || property.is_static
                {
                    continue;
                }

                call_configurators(
                    &registry,
                    &mut self.property_attribute_configurators,
                    &conditionals,
                    &Reflector::Property {
                        class: Arc::clone(&class_reflector),
                        property,
                    },
                )?;
            }
        }

        if !has_existing
            && !php_value::compare::loose_equals(
                &Definition::child("").to_value(),
                &conditionals.to_value(),
            )
        {
            instanceof.insert(class_reflector.name.as_str(), conditionals.to_value());
            definition.set_instanceof_conditionals(instanceof);
        }

        self.parent_process_value(container, value, is_root)
    }
}

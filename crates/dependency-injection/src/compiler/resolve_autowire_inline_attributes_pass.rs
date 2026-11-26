//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveAutowireInlineAttributesPass.php:
//! inspects existing autowired services for `#[AutowireInline]`
//! attributes and registers the definitions for reuse.
//!
//! The attributes come from the class metadata and are instantiated as
//! `AutowirePass` does ([`super::new_autowire_attribute`]);
//! `buildDefinition()` is the port of `AutowirePass`'s module. The
//! parameter resolver container shares the container's parameter bag,
//! as upstream's does: the bag is lent to it while
//! `ResolveParameterPlaceHoldersPass` runs.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveAutowireInlineAttributesPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::autowire_pass::{
    AUTOWIRE_INLINE, attribute_is_a, build_definition, new_autowire_attribute,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::export_type::{DeclaringClass, export_type};
use super::resolve_parameter_place_holders_pass::ResolveParameterPlaceHoldersPass;
use crate::class_metadata::FunctionReflector;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{ErrorKind, Result};
use crate::reference::Reference;

/// `ResolveAutowireInlineAttributesPass`.
#[derive(Debug, Clone)]
pub struct ResolveAutowireInlineAttributesPass {
    state: RecursivePassState,
    counter: u64,
}

impl Default for ResolveAutowireInlineAttributesPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveAutowireInlineAttributesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveAutowireInlineAttributesPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state, counter: 0 }
    }

    /// `registerAutowireInlineAttributes($method, $arguments,
    /// $isChildDefinition)`.
    fn register_autowire_inline_attributes(
        &mut self,
        container: &mut ContainerBuilder,
        method: &FunctionReflector,
        mut arguments: PhpArray,
        is_child_definition: bool,
    ) -> Result<PhpArray> {
        let mut parameters = method.parameters();
        if parameters.last().is_some_and(|p| p.is_variadic) {
            parameters.pop();
        }
        let registry = container.class_registry();
        let declaring_parent = method
            .class()
            .and_then(|c| registry.class(c))
            .and_then(|c| c.parent.clone());
        let declaring = DeclaringClass {
            name: method.class(),
            parent: declaring_parent.as_deref(),
        };
        let mut param_resolver =
            ContainerBuilder::with_parameter_bag(std::mem::take(container.get_parameter_bag_mut()));
        let result = (|| -> Result<PhpArray> {
            for (index, parameter) in parameters.iter().enumerate() {
                let index_key = if is_child_definition {
                    Key::from(format!("index_{index}"))
                } else {
                    Key::Int(i64::try_from(index).unwrap_or(i64::MAX))
                };
                let named = Key::from(format!("${}", parameter.name));

                let attribute = if arguments.contains_key(&named)
                    || arguments
                        .get(&index_key)
                        .is_some_and(|v| !matches!(v, Value::String(s) if s.is_empty()))
                {
                    let attribute = arguments
                        .get(&named)
                        .or_else(|| arguments.get(&index_key))
                        .cloned()
                        .unwrap_or_default();
                    if !attribute
                        .as_object()
                        .is_some_and(|o| attribute_is_a(&registry, o.class(), AUTOWIRE_INLINE))
                    {
                        continue;
                    }
                    attribute
                } else if let Some(attribute) = parameter
                    .attributes
                    .iter()
                    .find(|a| attribute_is_a(&registry, &a.class, AUTOWIRE_INLINE))
                {
                    new_autowire_attribute(attribute)?
                } else {
                    continue;
                };

                let type_ = export_type(parameter, true, declaring).unwrap_or_default();

                if type_.is_empty() && arguments.get(&index_key).is_some_and(|v| !v.is_null()) {
                    continue;
                }

                let value = attribute
                    .as_object()
                    .and_then(|o| o.properties().get(&Key::from("value")).cloned())
                    .unwrap_or_default();
                let definition =
                    build_definition(&registry, &attribute, value, &type_, parameter, method)?;

                param_resolver.set_definition(".autowire_inline", definition.clone())?;
                ResolveParameterPlaceHoldersPass::new(false, false).process(&mut param_resolver)?;

                self.counter += 1;
                let id = format!(
                    ".autowire_inline.{}.{}",
                    self.state.current_id.clone().unwrap_or_default(),
                    self.counter
                );

                // The bag is back in the container while it is changed.
                std::mem::swap(
                    container.get_parameter_bag_mut(),
                    param_resolver.get_parameter_bag_mut(),
                );
                let processed = (|| -> Result<()> {
                    container.set_definition(&id, definition.clone())?;
                    if definition.is_autowired() {
                        self.process_value(container, definition.to_value(), false)?;
                    }
                    Ok(())
                })();
                std::mem::swap(
                    container.get_parameter_bag_mut(),
                    param_resolver.get_parameter_bag_mut(),
                );
                processed?;

                arguments.insert(
                    if is_child_definition {
                        named
                    } else {
                        index_key
                    },
                    Value::from(Reference::new(id)),
                );
            }
            Ok(arguments)
        })();
        *container.get_parameter_bag_mut() = std::mem::take(param_resolver.get_parameter_bag_mut());
        result
    }
}

impl CompilerPassInterface for ResolveAutowireInlineAttributesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveAutowireInlineAttributesPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let value = self.parent_process_value(container, value, is_root)?;

        let Some(definition) = Definition::from_value(&value) else {
            return Ok(value);
        };
        if !definition.is_autowired()
            || !definition
                .class()
                .is_some_and(|c| !c.is_empty() && c != "0")
            || definition.has_tag("container.ignore_attributes")
        {
            return Ok(value);
        }

        if is_root {
            self.counter = 0;
        }

        let is_child_definition = definition.is_child();

        let constructor = match self.get_constructor(container, &definition, false) {
            Ok(c) => c,
            Err(e) if e.is(&ErrorKind::Runtime) => return Ok(value),
            Err(e) => return Err(e),
        };

        if let Some(constructor) = constructor {
            let arguments = self.register_autowire_inline_attributes(
                container,
                &constructor,
                definition.arguments(),
                is_child_definition,
            )?;
            if arguments != definition.arguments() {
                definition.set_arguments(arguments);
            }
        }

        let original_calls = definition.method_calls();
        let mut method_calls = original_calls.clone();

        for (i, call) in original_calls.iter() {
            let call = call.as_array().cloned().unwrap_or_default();
            let name = call
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let arguments = call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            let method = match self.get_reflection_method(container, &definition, &name) {
                Ok(m) => m,
                Err(e) if e.is(&ErrorKind::Runtime) => continue,
                Err(e) => return Err(e),
            };

            let new_arguments = self.register_autowire_inline_attributes(
                container,
                &method,
                arguments.clone(),
                is_child_definition,
            )?;

            if new_arguments != arguments {
                let mut call = call;
                call.insert(1, Value::Array(new_arguments));
                method_calls.insert(i.clone(), Value::Array(call));
            }
        }

        if method_calls != original_calls {
            definition.set_method_calls(&method_calls)?;
        }

        Ok(value)
    }
}

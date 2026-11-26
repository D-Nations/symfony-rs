//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveInvalidReferencesPass.php.
//!
//! Applies the invalid behaviour of references to missing services:
//! `null` for `NULL_ON_INVALID_REFERENCE`, removal of the argument,
//! property or method call (or `null`) for `IGNORE_ON_INVALID_REFERENCE`,
//! and an errored definition for a `TypedReference` with
//! `RUNTIME_EXCEPTION_ON_INVALID_REFERENCE`.
//!
//! The signal upstream throws to remove an entry is a `RuntimeException`
//! with upstream's message; any `RuntimeException` is caught as upstream
//! catches it.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveInvalidReferencesPass.php

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::Argument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::{Definition, DefinitionError};
use crate::exception::{Error, ErrorKind, Result};
use crate::reference::Reference;

/// `ResolveInvalidReferencesPass`.
#[derive(Debug, Clone, Default)]
pub struct ResolveInvalidReferencesPass {
    current_id: String,
}

fn signaling_exception() -> Error {
    Error::runtime("Invalid reference.")
}

impl ResolveInvalidReferencesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveInvalidReferencesPass";

    pub fn new() -> Self {
        Self::default()
    }

    fn process_array(
        &mut self,
        container: &mut ContainerBuilder,
        array: &PhpArray,
        root_level: i64,
        level: i64,
    ) -> Result<PhpArray> {
        let mut value = array.clone();
        let mut i: Option<i64> = Some(0);

        for (k, v) in array.iter() {
            if let Some(n) = i {
                i = (*k == Key::Int(n)).then_some(n + 1);
            }
            match self.process_value(container, v.clone(), root_level, 1 + level) {
                Ok(processed) => {
                    if !v.identical(&processed) {
                        value.insert(k.clone(), processed);
                    }
                }
                Err(e) if e.is(&ErrorKind::Runtime) => {
                    if root_level < level || (root_level != 0 && level == 0) {
                        value.remove(k);
                    } else if root_level != 0 {
                        return Err(e);
                    } else {
                        value.insert(k.clone(), Value::Null);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Ensure numerically indexed arguments have sequential numeric keys.
        if i.is_some() {
            value = PhpArray::list(value.values().cloned());
        }
        Ok(value)
    }

    /// `processValue($value, $rootLevel, $level)`.
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        root_level: i64,
        level: i64,
    ) -> Result<Value> {
        if let Some(argument) = Argument::from_value(&value) {
            let (root_level, level) = if matches!(argument, Argument::ServiceClosure(_)) {
                (1, 1)
            } else {
                (root_level, 1 + level)
            };
            let values = self.process_array(container, &argument.values(), root_level, level)?;
            argument.set_values(values)?;
        } else if let Some(definition) = Definition::from_value(&value) {
            if definition.is_synthetic()
                || definition.is_abstract()
                || definition.has_tag("container.excluded")
            {
                return Ok(value);
            }
            let arguments = self.process_array(container, &definition.arguments(), 0, 0)?;
            definition.set_arguments(arguments);
            let properties = self.process_array(container, &definition.properties(), 1, 0)?;
            definition.set_properties(properties);
            let calls = self.process_array(container, &definition.method_calls(), 2, 0)?;
            definition.set_method_calls(&calls)?;
        } else if let Value::Array(array) = value {
            return Ok(Value::Array(
                self.process_array(container, &array, root_level, level)?,
            ));
        } else if let Some(reference) = Reference::from_value(&value) {
            let id = reference.id().to_string();
            let valid = if container.has_definition(&id) {
                !container.get_definition(&id)?.has_tag("container.excluded")
            } else {
                container.has_alias(&id)
            };
            if valid {
                return Ok(value);
            }

            let current_definition = container.get_definition(&self.current_id)?;

            // resolve decorated service behavior depending on decorator service
            if current_definition.inner_service_id().as_deref() == Some(id.as_str())
                && current_definition.decoration_on_invalid() == Some(InvalidBehavior::Null)
            {
                return Ok(Value::Null);
            }

            let invalid_behavior = reference.invalid_behavior();

            if invalid_behavior == InvalidBehavior::RuntimeException
                && let Some(type_) = reference.type_()
                && !container.has(&id)
            {
                let e = Error::service_not_found(
                    id.as_str(),
                    Some(self.current_id.clone()),
                    Vec::new(),
                    None,
                );

                // since the error message varies by $id and $this->currentId, so should the id of the dummy errored definition
                let errored_id = format!(".errored.{}.{id}", self.current_id);
                container
                    .register(&errored_id, Some(type_))?
                    .add_error(DefinitionError::Text(e.message()));

                return Ok(Reference::typed(
                    errored_id,
                    type_,
                    invalid_behavior,
                    None,
                    PhpArray::new(),
                )
                .into());
            }

            // resolve invalid behavior
            if invalid_behavior == InvalidBehavior::Null {
                return Ok(Value::Null);
            } else if invalid_behavior == InvalidBehavior::Ignore {
                if 0 < level || root_level != 0 {
                    return Err(signaling_exception());
                }
                return Ok(Value::Null);
            }
        }

        Ok(value)
    }
}

impl CompilerPassInterface for ResolveInvalidReferencesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let definitions: Vec<_> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            self.current_id = id;
            self.process_value(container, definition.to_value(), 0, 0)?;
        }
        Ok(())
    }
}

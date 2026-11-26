//! Port of Symfony\Component\DependencyInjection\Compiler\RegisterReverseContainerPass.php:
//! fills the locator of the `reverse_container` service with the
//! reversible services.
//!
//! Upstream: symfony/dependency-injection Compiler/RegisterReverseContainerPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::{Argument, ServiceClosureArgument};
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `RegisterReverseContainerPass`.
#[derive(Debug, Clone, Copy)]
pub struct RegisterReverseContainerPass {
    before_removing: bool,
}

impl RegisterReverseContainerPass {
    /// `new RegisterReverseContainerPass($beforeRemoving)`.
    pub fn new(before_removing: bool) -> Self {
        Self { before_removing }
    }
}

impl CompilerPassInterface for RegisterReverseContainerPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\RegisterReverseContainerPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        if !container.has_definition("reverse_container") {
            return Ok(());
        }

        let ref_type = if self.before_removing {
            InvalidBehavior::IgnoreUninitialized
        } else {
            InvalidBehavior::Exception
        };
        let mut services = PhpArray::new();
        for (id, _tags) in container
            .find_tagged_service_ids("container.reversible", false)?
            .iter()
        {
            services.insert(id, Value::from(Reference::with_behavior(id, ref_type)));
        }

        if self.before_removing {
            // prevent inlining of the reverse container
            services.insert(
                "reverse_container",
                Value::from(Reference::with_behavior("reverse_container", ref_type)),
            );
        }
        let mut locator = container.get_definition("reverse_container")?.argument(1)?;

        if let Some(reference) = Reference::from_value(&locator) {
            locator = container.get_definition(reference.id())?.to_value();
        }
        if let Some(definition) = Definition::from_value(&locator) {
            let mut wrapped = PhpArray::new();
            for (id, reference) in services.iter() {
                wrapped.insert(
                    id.clone(),
                    ServiceClosureArgument::new(reference.clone()).to_value(),
                );
            }
            definition.replace_argument(0, Value::Array(wrapped))?;
        } else if let Some(argument) = Argument::from_value(&locator) {
            argument.set_values(services)?;
        } else {
            return Err(Error::new(
                crate::exception::ErrorKind::Other("Error".to_string()),
                format!(
                    "Call to a member function setValues() on {}",
                    locator.debug_type()
                ),
            ));
        }
        Ok(())
    }
}

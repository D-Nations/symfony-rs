//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveServiceSubscribersPass.php.
//!
//! Inside a definition tagged `container.service_subscriber.locator`,
//! references to PSR-11's `ContainerInterface` or to
//! `ServiceProviderInterface` become references to the subscriber's
//! locator.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveServiceSubscribersPass.php

use php_value::{Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::Result;
use crate::reference::Reference;

const PSR_CONTAINER_INTERFACE: &str = "Psr\\Container\\ContainerInterface";
const SERVICE_PROVIDER_INTERFACE: &str = "Symfony\\Contracts\\Service\\ServiceProviderInterface";

/// `ResolveServiceSubscribersPass`.
#[derive(Debug, Clone)]
pub struct ResolveServiceSubscribersPass {
    state: RecursivePassState,
    service_locator: Option<String>,
}

impl Default for ResolveServiceSubscribersPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveServiceSubscribersPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveServiceSubscribersPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            service_locator: None,
        }
    }
}

impl CompilerPassInterface for ResolveServiceSubscribersPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

/// `(string) $value` of the tag attribute `id`.
fn locator_id(definition: &Definition) -> Option<String> {
    definition
        .tag("container.service_subscriber.locator")
        .get(&Key::Int(0))
        .and_then(Value::as_array)
        .and_then(|attributes| attributes.get(&Key::from("id")))
        .and_then(Value::to_php_string)
        .map(|s| s.to_string_lossy().into_owned())
}

impl AbstractRecursivePass for ResolveServiceSubscribersPass {
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
            && let Some(locator) = &self.service_locator
            // `$this->serviceLocator` is truthy: not "" nor "0".
            && !locator.is_empty()
            && locator != "0"
            && [PSR_CONTAINER_INTERFACE, SERVICE_PROVIDER_INTERFACE].contains(&reference.id())
        {
            return Ok(Reference::new(locator.clone()).into());
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };

        let service_locator = self.service_locator.take();

        if definition.has_tag("container.service_subscriber.locator") {
            self.service_locator = locator_id(&definition);
            definition.clear_tag("container.service_subscriber.locator");
        }

        let result = self.parent_process_value(container, value, false);
        self.service_locator = service_locator;
        result
    }
}

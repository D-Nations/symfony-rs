//! Port of Symfony\Component\HttpKernel\Controller\ContainerControllerResolver.php.
//!
//! A [`ControllerResolver`] that, for the `service::method` notation (and
//! for a class name or `[class, method]`), takes the object from a PSR-11
//! container before it tries `new $class()`. PHP's subclass overrides
//! `instantiateController()`; here the parent resolver holds the container
//! and calls [`instantiate_controller`] wherever PHP's `$this->
//! instantiateController()` runs, so every other rule of the parent is
//! unchanged. [`ContainerControllerResolver`] dereferences to it.
//!
//! PHP checks `$this->container instanceof
//! Symfony\Component\DependencyInjection\Container` before it reads
//! `getRemovedIds()`. That is the closed choice [`ControllerContainer`]:
//! any PSR-11 container, or a Symfony container, which also reports the
//! ids the compiler removed.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Controller/ContainerControllerResolver.php

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use php_value::{Key, PhpArray, PhpString, Value};
use psr_container::ContainerInterface;
use psr_log::LoggerInterface;
use symfony_http_foundation::request_stack::RequestHandle;

use crate::{
    controller::{Controller, ControllerResolverInterface},
    controller_resolver::{ClassRegistry, ControllerResolver},
    exception::{PhpClass, PhpException, Throwable, instance_of},
};

/// `Symfony\Component\DependencyInjection\Container`, as far as the
/// resolver reads it: a PSR-11 container that knows the ids the compiler
/// removed.
pub trait DependencyInjectionContainer: ContainerInterface {
    /// `getRemovedIds()`: the removed ids as keys.
    fn get_removed_ids(&self) -> PhpArray;
}

/// The resolver's `$container`.
#[derive(Clone)]
pub enum ControllerContainer {
    /// Any other `Psr\Container\ContainerInterface`.
    Psr(Arc<dyn ContainerInterface>),
    /// A `Symfony\Component\DependencyInjection\Container`.
    DependencyInjection(Arc<dyn DependencyInjectionContainer>),
}

impl ControllerContainer {
    fn has(&self, id: &[u8]) -> bool {
        match self {
            Self::Psr(container) => container.has(id),
            Self::DependencyInjection(container) => container.has(id),
        }
    }

    fn get(&self, id: &[u8]) -> Result<Value, Throwable> {
        match self {
            Self::Psr(container) => container.get(id),
            Self::DependencyInjection(container) => container.get(id),
        }
    }
}

/// `ContainerControllerResolver`.
pub struct ContainerControllerResolver(ControllerResolver);

impl ContainerControllerResolver {
    /// `new ContainerControllerResolver($container, $logger)` over the
    /// declared program.
    pub fn new(
        container: ControllerContainer,
        classes: Arc<ClassRegistry>,
        logger: Option<Arc<dyn LoggerInterface>>,
    ) -> Self {
        let mut resolver = ControllerResolver::new(classes, logger);
        resolver.container = Some(container);
        Self(resolver)
    }

    /// With the path of `ControllerResolver.php` that a constructor's
    /// `\ArgumentCountError` names ([`ControllerResolver::with_source_file`]).
    #[must_use]
    pub fn with_source_file(self, file: impl Into<PhpString>) -> Self {
        Self(self.0.with_source_file(file))
    }

    /// The resolver as its parent class; it keeps the container.
    pub fn into_inner(self) -> ControllerResolver {
        self.0
    }
}

impl Deref for ContainerControllerResolver {
    type Target = ControllerResolver;

    fn deref(&self) -> &ControllerResolver {
        &self.0
    }
}

impl DerefMut for ContainerControllerResolver {
    fn deref_mut(&mut self) -> &mut ControllerResolver {
        &mut self.0
    }
}

impl ControllerResolverInterface for ContainerControllerResolver {
    fn get_controller(&self, request: &RequestHandle) -> Result<Option<Controller>, Throwable> {
        self.0.get_controller(request)
    }
}

/// `instantiateController($class)`: the container's entry when it has
/// one, else `new $class()`, with the container's messages when that
/// fails with an `\Error`.
pub(crate) fn instantiate_controller(
    resolver: &ControllerResolver,
    container: &ControllerContainer,
    class: &[u8],
) -> Result<Value, Throwable> {
    // ltrim($class, '\\')
    let start = class.iter().take_while(|&&b| b == b'\\').count();
    let class = &class[start..];

    if container.has(class) {
        return container.get(class);
    }

    let e = match resolver.instantiate_class(class) {
        Ok(object) => return Ok(object),
        Err(e) if instance_of(e.as_ref(), "Error") => e,
        Err(e) => return Err(e),
    };

    throw_exception_if_controller_was_removed(container, class, &e)?;

    let message = if instance_of(e.as_ref(), "ArgumentCountError") {
        "\" has required constructor arguments and does not exist in the container. Did you forget to define the controller as a service?"
    } else {
        "\" does neither exist as service nor as class."
    };
    Err(invalid_argument(class, message, e))
}

/// `throwExceptionIfControllerWasRemoved($controller, $previous)`.
fn throw_exception_if_controller_was_removed(
    container: &ControllerContainer,
    controller: &[u8],
    previous: &Throwable,
) -> Result<(), Throwable> {
    let ControllerContainer::DependencyInjection(container) = container else {
        return Ok(());
    };
    // isset($removedIds[$controller]): a key whose value is not null.
    let removed = container
        .get_removed_ids()
        .get(&Key::from_bytes(controller))
        .is_some_and(|value| !value.is_null());
    if removed {
        return Err(invalid_argument(
            controller,
            "\" cannot be fetched from the container because it is private. Did you forget to tag the service with \"controller.service_arguments\"?",
            previous.clone(),
        ));
    }
    Ok(())
}

/// `new \InvalidArgumentException(sprintf('Controller "%s...', $class), 0, $previous)`.
fn invalid_argument(class: &[u8], rest: &str, previous: Throwable) -> Throwable {
    let message = PhpString::from([b"Controller \"".as_slice(), class, rest.as_bytes()].concat());
    PhpException::new(PhpClass::InvalidArgumentException, message)
        .with_previous(previous)
        .throwable()
}

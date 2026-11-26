//! Port of Symfony\Component\DependencyInjection\Dumper\Dumper.php.
//!
//! The dumpers borrow the builder mutably: resolving env placeholders
//! counts the env vars it meets (`getEnvCounters()`), as upstream's
//! shared builder does.
//!
//! Upstream: symfony/dependency-injection Dumper/Dumper.php

use crate::container_builder::ContainerBuilder;

/// `Dumper`: the base of the built-in dumpers, holding the container.
#[derive(Debug)]
pub struct Dumper<'a> {
    container: &'a mut ContainerBuilder,
}

impl<'a> Dumper<'a> {
    /// `new Dumper($container)`.
    pub fn new(container: &'a mut ContainerBuilder) -> Self {
        Self { container }
    }

    /// `$this->container`.
    pub fn container(&self) -> &ContainerBuilder {
        self.container
    }

    /// `$this->container`, to change.
    pub fn container_mut(&mut self) -> &mut ContainerBuilder {
        self.container
    }
}

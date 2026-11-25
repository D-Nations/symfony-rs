//! Port of Symfony\Component\DependencyInjection\TaggedContainerInterface.php.
//!
//! Upstream: symfony/dependency-injection TaggedContainerInterface.php

use php_value::PhpArray;

use crate::container_interface::ContainerInterface;

/// `Symfony\Component\DependencyInjection\TaggedContainerInterface`.
pub const TAGGED_CONTAINER_INTERFACE: &str =
    "Symfony\\Component\\DependencyInjection\\TaggedContainerInterface";

/// `TaggedContainerInterface`: a container that knows its services' tags.
pub trait TaggedContainerInterface: ContainerInterface {
    /// `findTaggedServiceIds($name)`: service id => the attribute arrays
    /// of its tags of that name.
    fn find_tagged_service_ids(&self, name: &str) -> PhpArray;
}

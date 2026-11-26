//! Port of symfony/dependency-injection 8.1 for a build-time container
//! compiler (PLAN.md, "M3: DependencyInjection"): the definition model,
//! parameter bags and, in later stages, the `ContainerBuilder`, compiler
//! passes, loaders and the Rust emitter that replaces `PhpDumper`.
//!
//! Values are php-value [`php_value::Value`]s. The component's mutable
//! classes (`Definition`, `Alias`, the argument objects) are shared PHP
//! objects with identity ([`object`]); `Reference`, `Parameter` and
//! `Variable` are immutable objects. Identifiers (service ids, class
//! names, parameter names) are `String`s.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod alias;
pub mod argument;
pub mod class_metadata;
pub mod compiler;
pub mod container_builder;
pub mod container_interface;
pub mod definition;
pub mod env_var_loader_interface;
pub mod env_var_processor;
pub mod env_var_processor_interface;
pub mod exception;
pub mod extension;
pub mod id_map;
pub mod loader;
pub mod object;
pub mod parameter_bag;
pub mod reference;
pub mod static_env_var_loader;

// The run-time container.
pub mod container;
mod ordered_map;
pub mod reverse_container;
pub mod service_locator;
pub mod service_object;
pub mod services_resetter;
pub mod tagged_container_interface;

pub use alias::{Alias, Deprecation};
pub use class_metadata::{ClassMetadata, ClassRegistry};
pub use container_builder::ContainerBuilder;
pub use container_interface::{ContainerInterface, InvalidBehavior};
pub use definition::{DecoratedService, Definition, DefinitionError, Index};
pub use env_var_loader_interface::EnvVarLoaderInterface;
pub use env_var_processor::{EnvVarLoaders, EnvVarProcessor};
pub use env_var_processor_interface::EnvVarProcessorInterface;
pub use exception::{Error, ErrorKind, Result};
pub use id_map::IdMap;
pub use parameter_bag::ParameterBag;
pub use reference::{Parameter, Reference, Variable};

pub use container::{Container, ContainerClass, EnvProcessor, EnvProcessorObject, ServiceFactory};
pub use reverse_container::ReverseContainer;
pub use service_locator::{Caller, LocatorEntry, ServiceLocator};
pub use service_object::ServiceObject;
pub use services_resetter::{ResetMap, ServicesResetter, ServicesResetterInterface};
pub use static_env_var_loader::StaticEnvVarLoader;
pub use tagged_container_interface::TaggedContainerInterface;

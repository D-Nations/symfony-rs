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
pub mod container_interface;
pub mod definition;
pub mod exception;
pub mod object;
pub mod parameter_bag;
pub mod reference;

pub use alias::{Alias, Deprecation};
pub use container_interface::{ContainerInterface, InvalidBehavior};
pub use definition::{DecoratedService, Definition, DefinitionError, Index};
pub use exception::{Error, ErrorKind, Result};
pub use parameter_bag::ParameterBag;
pub use reference::{Parameter, Reference, Variable};

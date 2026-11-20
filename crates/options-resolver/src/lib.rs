//! Port of the symfony/options-resolver component. See README.md for the
//! contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod debug;
pub mod exception;
mod option_configurator;
mod options_resolver;
pub mod php;

pub use debug::OptionsResolverIntrospector;
pub use exception::{Class, Error};
pub use option_configurator::OptionConfigurator;
pub use options_resolver::{
    AllowedClosure, AllowedValue, AllowedValues, DEFAULT_DEPRECATION_MESSAGE, DefaultValue,
    Deprecation, DeprecationMessage, Names, NestedClosure, OptionClosure, OptionsResolver,
};

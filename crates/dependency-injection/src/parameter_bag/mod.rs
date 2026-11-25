//! The parameter bags of Symfony\Component\DependencyInjection\ParameterBag.
//!
//! Upstream: none

pub mod container_bag;
#[expect(clippy::module_inception, reason = "one module per upstream file")]
mod parameter_bag;

mod env_placeholder_parameter_bag;

pub use container_bag::{ContainerBag, ContainerBagInterface};
pub use env_placeholder_parameter_bag::{EnvPlaceholderMap, EnvPlaceholders};
pub use parameter_bag::{ParameterBag, ParameterDeprecation, escape_value, unescape_value};

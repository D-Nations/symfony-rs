//! The parameter bags of Symfony\Component\DependencyInjection\ParameterBag.
//!
//! Not yet ported: `EnvPlaceholderParameterBag`.
//!
//! Upstream: none

pub mod container_bag;
#[expect(clippy::module_inception, reason = "one module per upstream file")]
mod parameter_bag;

pub use container_bag::{ContainerBag, ContainerBagInterface};
pub use parameter_bag::{ParameterBag, ParameterDeprecation, escape_value, unescape_value};

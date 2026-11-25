//! The parameter bags of Symfony\Component\DependencyInjection\ParameterBag.
//!
//! Not yet ported: `EnvPlaceholderParameterBag`, `ContainerBag` and
//! `ContainerBagInterface`.
//!
//! Upstream: none

#[expect(clippy::module_inception, reason = "one module per upstream file")]
mod parameter_bag;

pub use parameter_bag::{ParameterBag, ParameterDeprecation, escape_value, unescape_value};

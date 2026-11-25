//! Port of the doctrine/dbal database abstraction layer. See README.md for
//! the contract.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

mod array_parameter_type;
pub mod exception;
mod expand_array_parameters;
mod parameter_type;
pub mod platforms;
pub mod sql;
pub mod types;

pub use array_parameter_type::ArrayParameterType;
pub use expand_array_parameters::{ArrayParametersException, ExpandArrayParameters};
pub use parameter_type::{ParameterType, WrapperParameterType};

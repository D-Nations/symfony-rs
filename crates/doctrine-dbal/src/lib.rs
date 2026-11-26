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
mod connection_stand_in;
pub mod exception;
mod expand_array_parameters;
mod parameter_type;
pub mod platforms;
pub mod query;
pub mod sql;
pub mod types;

// Slice SQLite platform.
pub mod deprecations;
pub mod schema;
mod transaction_isolation_level;

pub use transaction_isolation_level::{LockMode, TransactionIsolationLevel};

pub use array_parameter_type::ArrayParameterType;
pub use connection_stand_in::ConnectionStandIn;
pub use exception::{Exception, InvalidArgumentException};
pub use expand_array_parameters::{ArrayParametersException, ExpandArrayParameters};
pub use parameter_type::{ParameterType, WrapperParameterType};

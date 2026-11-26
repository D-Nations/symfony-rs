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
mod configuration;
mod connection;
mod connection_stand_in;
pub mod driver;
mod driver_manager;
pub mod exception;
mod expand_array_parameters;
mod parameter_type;
pub mod platforms;
pub mod query;
mod result;
mod server_version_provider;
pub mod sql;
mod sql_query;
mod statement;
pub mod tools;
mod transaction_isolation_level;
pub mod types;

pub use array_parameter_type::ArrayParameterType;
pub use configuration::Configuration;
pub use connection::Connection;
pub use connection_stand_in::ConnectionStandIn;
pub use driver_manager::{DriverClass, DriverManager, PDO_SQLITE_DRIVER};
pub use exception::{Exception, InvalidArgumentException};
pub use expand_array_parameters::{ArrayParametersException, ExpandArrayParameters};
pub use parameter_type::{ParameterType, WrapperParameterType};
pub use result::Result;
pub use server_version_provider::{ServerVersionProvider, StaticServerVersionProvider};
pub use sql_query::{ParameterTypes, Query};
pub use statement::Statement;
pub use transaction_isolation_level::TransactionIsolationLevel;

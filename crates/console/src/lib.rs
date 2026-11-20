//! The parts of symfony/console that the Yaml and Dotenv commands use
//! (PLAN.md, M1-B). See README.md for the contract.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod color;
pub mod env;
pub mod exception;
pub mod formatter;
pub mod helper;
pub mod output;
pub mod terminal;

pub use exception::{ConsoleError, Result};

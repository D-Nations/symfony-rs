//! Port of the symfony/dotenv component. See README.md for the contract
//! and what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod dotenv;
pub mod exception;
pub mod globals;
pub mod local_php;

pub use dotenv::Dotenv;
pub use exception::{Error, FormatException, FormatExceptionContext, PathException};
pub use globals::{Commands, Globals, Shell};

//! Port of the symfony/config component. See README.md for the contract
//! and what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod definition;
pub mod php;

pub use definition::{Class, Error, Node, Processor};

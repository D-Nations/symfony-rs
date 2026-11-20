//! Port of the symfony/css-selector component (itself a port of Python's
//! cssselect): CSS selectors to XPath 1.0 expressions. See README.md for
//! the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! cssselect copyright Ian Bicking.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

mod css_selector_converter;
pub mod exception;
pub mod node;
pub mod parser;
mod php;
pub mod xpath;

pub use css_selector_converter::CssSelectorConverter;
pub use exception::{Exception, Kind};

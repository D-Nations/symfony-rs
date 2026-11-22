//! Symfony\Component\Config\Util: XmlUtils and its exceptions, on
//! `php-dom` (PLAN.md decision 9).
//!
//! Upstream: none

pub mod exception;
pub mod xml_utils;

pub use xml_utils::{Validation, ValidationCallable, XmlUtils};

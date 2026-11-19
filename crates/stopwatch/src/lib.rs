//! Port of the symfony/stopwatch component. See README.md for the
//! contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod php;
mod section;
mod stopwatch;
mod stopwatch_event;
mod stopwatch_period;

use std::fmt;

pub use php::{Host, Number, SystemHost};
pub use section::Section;
pub use stopwatch::{ROOT, Stopwatch};
pub use stopwatch_event::StopwatchEvent;
pub use stopwatch_period::StopwatchPeriod;

/// PHP's `\LogicException`, with upstream's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicException {
    pub message: String,
}

impl LogicException {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for LogicException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LogicException {}

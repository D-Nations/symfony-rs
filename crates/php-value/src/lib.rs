//! PHP's values and the engine behaviour symfony-rs components rely on:
//! byte strings, ordered arrays with PHP's key rules, objects, and the
//! string/number conversions. Written from PHP 8.4's observed behaviour
//! (vectors recorded in the oracle container); no PHP source code is used
//! (PLAN.md decisions 3 and 8).
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod array;
pub mod json;
pub mod numeric;
pub mod pcre;
pub mod string;
pub mod value;

pub use array::{Key, NextElementOccupied, PhpArray};
pub use numeric::Number;
pub use string::PhpString;
pub use value::{Object, PhpObject, Value};

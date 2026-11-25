//! PHP's values and the engine behaviour symfony-rs components rely on:
//! byte strings, ordered arrays with PHP's key rules, objects, and the
//! string/number conversions. Written from PHP 8.4's observed behaviour
//! (vectors recorded in the oracle container); no PHP source code is used
//! (PLAN.md decisions 3 and 8).
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod array;
pub mod base64;
pub mod clock;
pub mod compare;
pub mod constants;
pub mod dateparse;
pub mod datetime;
pub mod filter;
pub mod handle;
pub mod html;
pub mod json;
pub mod net;
pub mod numeric;
pub mod object;
pub mod path;
pub mod pcre;
pub mod serialize;
pub mod string;
pub mod url;
pub mod value;
pub mod vars;
#[cfg(feature = "vectors")]
pub mod vectors;

pub use array::{Key, NextElementOccupied, PhpArray};
pub use handle::Handle;
pub use numeric::Number;
pub use string::PhpString;
pub use value::{Object, PhpObject, Thrown, Value};

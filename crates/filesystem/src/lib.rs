//! Port of the symfony/filesystem component: the part `ConfigCache` and
//! `ResourceCheckerConfigCache` use (`dumpFile()`, `readFile()`, `chmod()`
//! and what they call), and `Path`'s canonicalization. See README.md for
//! the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod exception;
pub mod filesystem;
pub mod path;
pub mod php;

pub use exception::{Class, Error};
pub use filesystem::Filesystem;
pub use path::{Env, Path};

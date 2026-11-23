//! Port of the symfony/translation component: the XLIFF loader and what it
//! needs (MessageCatalogue, XliffUtils, the exceptions). See README.md for
//! the contract and what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod exception;
pub mod loader;
pub mod message_catalogue;
pub mod util;

pub use exception::{Class, Error, Previous};
pub use loader::{LoaderInterface, XliffFileLoader};
pub use message_catalogue::{INTL_DOMAIN_SUFFIX, MessageCatalogue, Resource};
pub use util::XliffUtils;

//! Port of the symfony/translation component: the XLIFF and Qt loaders and
//! what they need (MessageCatalogue, XliffUtils, the exceptions). See
//! README.md for the contract and what is ported so far.
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
pub use loader::{LoaderInterface, QtFileLoader, XliffFileLoader};
pub use message_catalogue::{INTL_DOMAIN_SUFFIX, MessageCatalogue};
pub use util::XliffUtils;

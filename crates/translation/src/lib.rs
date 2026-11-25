//! Port of the symfony/translation component: catalogues and catalogue
//! operations, the XLIFF and Qt loaders, `XliffUtils` and exceptions. See
//! README.md for the contract and what is ported so far.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod catalogue;
pub mod exception;
pub mod loader;
pub mod message_catalogue;
pub mod util;

pub use exception::{Class, Error, Previous};
pub use loader::{LoaderInterface, QtFileLoader, XliffFileLoader};
pub use message_catalogue::{INTL_DOMAIN_SUFFIX, MessageCatalogue};
pub use util::XliffUtils;

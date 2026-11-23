//! The subset of PHP's DOM extension that Symfony's XML consumers use
//! (`DOMDocument::loadXML()`, the read-only node API and
//! `schemaValidateSource()`, a `DOMXPath::evaluate()` subset), with the diagnostics libxml reports while
//! parsing and validating. Written from PHP 8.4.27's
//! observed behaviour on libxml 2.9.14 (`scripts/record.sh php-dom`), the
//! XML 1.0 and Namespaces specifications; no PHP or libxml source is used
//! (PLAN.md decisions 8 and 9). See README.md for the contract.
//!
//! Upstream: none

#![forbid(unsafe_code)]

mod chars;
pub mod dom;
mod dtd;
mod encoding;
pub mod error;
pub mod libxml;
mod parser;
pub mod schema;
mod uri;
pub mod xpath;

pub use dom::{Attribute, Document, NodeId, NodeType};
pub use error::{ErrorLevel, LibxmlError, LoadError, Refusal, RefusalReason};
pub use libxml::libxml_use_internal_errors;
pub use parser::{LoadOptions, Loaded, load_xml};
pub use schema::{Schema, SchemaError, SchemaValidation, schema_validate_source};
pub use xpath::XPathError;

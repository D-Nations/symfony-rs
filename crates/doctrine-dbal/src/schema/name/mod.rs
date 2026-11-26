//! Doctrine\DBAL\Schema\Name: names of database objects and their parsers.
//!
//! Upstream: none

mod identifier;
mod names;
mod parser;

pub use identifier::{Identifier, UnquotedIdentifierFolding};
pub use names::{GenericName, Name, OptionallyQualifiedName, UnqualifiedName};
pub use parser::{
    ASSERTION_ERROR, parse_generic_name, parse_optionally_qualified_name, parse_unqualified_name,
};

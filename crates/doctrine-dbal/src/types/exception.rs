//! Port of Doctrine\DBAL\Types\Exception\TypesException.php and the
//! registry's exceptions: `TypeAlreadyRegistered`, `TypeArgumentCountError`,
//! `TypeNotFound`, `TypeNotRegistered`, `TypesAlreadyExists` and
//! `UnknownColumnType`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/Exception/TypesException.php src/Types/Exception/TypeAlreadyRegistered.php
//! Upstream: doctrine/dbal src/Types/Exception/TypeArgumentCountError.php src/Types/Exception/TypeNotFound.php
//! Upstream: doctrine/dbal src/Types/Exception/TypeNotRegistered.php src/Types/Exception/TypesAlreadyExists.php
//! Upstream: doctrine/dbal src/Types/Exception/UnknownColumnType.php

use std::{fmt, sync::Arc};

use php_value::Thrown;

use super::Type;

/// A `Types\Exception\TypesException` thrown by the type registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypesException {
    /// `TypeAlreadyRegistered`: the message names the instance.
    TypeAlreadyRegistered(String),
    /// `TypeArgumentCountError`, with the `ArgumentCountError` as previous.
    TypeArgumentCountError { name: String, previous: Thrown },
    /// `TypeNotFound`.
    TypeNotFound(String),
    /// `TypeNotRegistered`: the message names the instance.
    TypeNotRegistered(String),
    /// `TypesAlreadyExists`.
    TypesAlreadyExists(String),
    /// `UnknownColumnType`.
    UnknownColumnType(String),
}

/// `spl_object_hash($object)`: 32 hexadecimal digits that tell objects
/// apart (here from the instance's address, PHP using its handle).
fn object_hash(type_: &Arc<dyn Type>) -> String {
    let address = Arc::as_ptr(type_).cast::<()>() as usize;
    format!("{address:016x}{:016x}", 0)
}

impl TypesException {
    /// `TypeAlreadyRegistered::new($type)`.
    pub fn type_already_registered(type_: &Arc<dyn Type>) -> Self {
        Self::TypeAlreadyRegistered(format!(
            "Type of the class {}@{} is already registered.",
            type_.class(),
            object_hash(type_)
        ))
    }

    /// `TypeNotRegistered::new($type)`.
    pub fn type_not_registered(type_: &Arc<dyn Type>) -> Self {
        Self::TypeNotRegistered(format!(
            "Type of the class {}@{} is not registered.",
            type_.class(),
            object_hash(type_)
        ))
    }

    /// `TypeArgumentCountError::new($name, $previous)`.
    pub fn type_argument_count_error(name: &str, previous: Thrown) -> Self {
        Self::TypeArgumentCountError {
            name: name.to_owned(),
            previous,
        }
    }

    /// The exception's class.
    pub fn class(&self) -> &'static str {
        match self {
            Self::TypeAlreadyRegistered(_) => {
                "Doctrine\\DBAL\\Types\\Exception\\TypeAlreadyRegistered"
            }
            Self::TypeArgumentCountError { .. } => {
                "Doctrine\\DBAL\\Types\\Exception\\TypeArgumentCountError"
            }
            Self::TypeNotFound(_) => "Doctrine\\DBAL\\Types\\Exception\\TypeNotFound",
            Self::TypeNotRegistered(_) => "Doctrine\\DBAL\\Types\\Exception\\TypeNotRegistered",
            Self::TypesAlreadyExists(_) => "Doctrine\\DBAL\\Types\\Exception\\TypesAlreadyExists",
            Self::UnknownColumnType(_) => "Doctrine\\DBAL\\Types\\Exception\\UnknownColumnType",
        }
    }

    /// `getMessage()`.
    pub fn message(&self) -> String {
        match self {
            Self::TypeAlreadyRegistered(message) | Self::TypeNotRegistered(message) => {
                message.clone()
            }
            Self::TypeArgumentCountError { name, .. } => {
                format!("To register \"{name}\" pass an instance to `Type::addType` instead.")
            }
            Self::TypeNotFound(name) => {
                format!("Type to be overwritten \"{name}\" does not exist.")
            }
            Self::TypesAlreadyExists(name) => format!("Type \"{name}\" already exists."),
            Self::UnknownColumnType(name) => format!(
                "Unknown column type \"{name}\" requested. Any Doctrine type that you use has \
                 to be registered with \\Doctrine\\DBAL\\Types\\Type::addType(). You can get a list of all the \
                 known types with \\Doctrine\\DBAL\\Types\\Type::getTypesMap(). If this error occurs during database \
                 introspection then you might have forgotten to register all database types for a Doctrine Type. \
                 Use AbstractPlatform#registerDoctrineTypeMapping() or have your custom types implement \
                 Type#getMappedDatabaseTypes(). If the type name is empty you might \
                 have a problem with the cache or forgot some mapping information."
            ),
        }
    }

    /// `getPrevious()`.
    pub fn previous(&self) -> Option<&Thrown> {
        match self {
            Self::TypeArgumentCountError { previous, .. } => Some(previous),
            _ => None,
        }
    }
}

impl fmt::Display for TypesException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class(), self.message())
    }
}

impl std::error::Error for TypesException {}

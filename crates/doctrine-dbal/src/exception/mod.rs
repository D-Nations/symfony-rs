//! Port of Doctrine\DBAL\Exception.php (the interface every DBAL exception
//! implements) and of the exceptions the platform's column declarations
//! throw: Exception\InvalidArgumentException.php,
//! Exception\InvalidColumnDeclaration.php, Exception\InvalidColumnType.php
//! and Exception\InvalidColumnType\*.php.
//!
//! One [`Exception`] value stands for a thrown object: its class (one of
//! the constants below, or a PHP engine error such as `TypeError` that a
//! `strict_types` call raises), its message and its previous exception.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Exception.php src/Exception/InvalidArgumentException.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnDeclaration.php src/Exception/InvalidColumnType.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnType/ColumnLengthRequired.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnType/ColumnPrecisionRequired.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnType/ColumnScaleRequired.php
//! Upstream: doctrine/dbal src/Exception/InvalidColumnType/ColumnValuesRequired.php

use std::fmt;

use php_value::PhpString;

/// A thrown DBAL exception (or PHP error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exception {
    class: &'static str,
    message: PhpString,
    previous: Option<Box<Exception>>,
}

impl Exception {
    pub const INVALID_ARGUMENT_EXCEPTION: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidArgumentException";
    pub const INVALID_COLUMN_DECLARATION: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidColumnDeclaration";
    pub const COLUMN_LENGTH_REQUIRED: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidColumnType\\ColumnLengthRequired";
    pub const COLUMN_PRECISION_REQUIRED: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidColumnType\\ColumnPrecisionRequired";
    pub const COLUMN_SCALE_REQUIRED: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidColumnType\\ColumnScaleRequired";
    pub const COLUMN_VALUES_REQUIRED: &'static str =
        "Doctrine\\DBAL\\Exception\\InvalidColumnType\\ColumnValuesRequired";
    /// PHP's `\TypeError`, for an argument of the wrong type under
    /// `strict_types` (its message leaves out PHP's "called in" suffix).
    pub const TYPE_ERROR: &'static str = "TypeError";
    /// Not PHP: input this port does not read. PHP gives an answer for it
    /// that is not reproduced.
    pub const UNSUPPORTED: &'static str = "(unsupported by this port)";

    pub fn new(class: &'static str, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
            previous: None,
        }
    }

    #[must_use]
    pub fn with_previous(mut self, previous: Exception) -> Self {
        self.previous = Some(Box::new(previous));
        self
    }

    /// `$e::class`.
    pub fn class(&self) -> &'static str {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `getPrevious()`.
    pub fn previous(&self) -> Option<&Exception> {
        self.previous.as_deref()
    }

    /// `$e instanceof $class`, for the classes and interfaces above.
    pub fn is_instance_of(&self, class: &str) -> bool {
        if self.class == class {
            return true;
        }
        let column_type = [
            Self::COLUMN_LENGTH_REQUIRED,
            Self::COLUMN_PRECISION_REQUIRED,
            Self::COLUMN_SCALE_REQUIRED,
            Self::COLUMN_VALUES_REQUIRED,
        ]
        .contains(&self.class);
        let dbal = column_type
            || [
                Self::INVALID_ARGUMENT_EXCEPTION,
                Self::INVALID_COLUMN_DECLARATION,
            ]
            .contains(&self.class);
        match class {
            "Doctrine\\DBAL\\Exception\\InvalidColumnType" => column_type,
            "Doctrine\\DBAL\\Exception" => dbal,
            "LogicException" => column_type || self.class == Self::INVALID_COLUMN_DECLARATION,
            "InvalidArgumentException" => self.class == Self::INVALID_ARGUMENT_EXCEPTION,
            _ => false,
        }
    }

    /// `ColumnLengthRequired::new($platform, $type)`; `platform` is
    /// `get_debug_type($platform)`.
    pub fn column_length_required(platform: &str, type_: &str) -> Self {
        Self::new(
            Self::COLUMN_LENGTH_REQUIRED,
            format!("{platform} requires the length of a {type_} column to be specified"),
        )
    }

    /// `ColumnPrecisionRequired::new()`.
    pub fn column_precision_required() -> Self {
        Self::new(
            Self::COLUMN_PRECISION_REQUIRED,
            "Column precision is not specified",
        )
    }

    /// `ColumnScaleRequired::new()`.
    pub fn column_scale_required() -> Self {
        Self::new(Self::COLUMN_SCALE_REQUIRED, "Column scale is not specified")
    }

    /// `ColumnValuesRequired::new($platform, $type)`.
    pub fn column_values_required(platform: &str, type_: &str) -> Self {
        Self::new(
            Self::COLUMN_VALUES_REQUIRED,
            format!("{platform} requires the values of a {type_} column to be specified"),
        )
    }

    /// `InvalidColumnDeclaration::fromInvalidColumnType($columnName, $e)`.
    pub fn invalid_column_declaration(column_name: &[u8], previous: Exception) -> Self {
        let mut message = b"Column \"".to_vec();
        message.extend_from_slice(column_name);
        message.extend_from_slice(b"\" has invalid type");
        Self::new(Self::INVALID_COLUMN_DECLARATION, message).with_previous(previous)
    }

    /// `InvalidColumnDeclaration::fromMissingColumnType($columnName)`.
    pub fn missing_column_type(column_name: &[u8]) -> Self {
        let mut message = b"Column \"".to_vec();
        message.extend_from_slice(column_name);
        message.extend_from_slice(
            b"\" has no type. The column definition must contain a \"typeName\" key.",
        );
        Self::new(Self::INVALID_COLUMN_DECLARATION, message)
    }

    /// `new InvalidArgumentException($message)`.
    pub fn invalid_argument(message: impl Into<PhpString>) -> Self {
        Self::new(Self::INVALID_ARGUMENT_EXCEPTION, message)
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class, self.message.to_string_lossy())
    }
}

impl std::error::Error for Exception {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

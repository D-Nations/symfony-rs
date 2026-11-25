//! Port of Doctrine\DBAL\Types\ConversionException.php and its subclasses
//! Types\Exception\InvalidFormat.php, InvalidType.php,
//! SerializationFailed.php and ValueNotConvertible.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/ConversionException.php src/Types/Exception/InvalidFormat.php
//! Upstream: doctrine/dbal src/Types/Exception/InvalidType.php src/Types/Exception/SerializationFailed.php
//! Upstream: doctrine/dbal src/Types/Exception/ValueNotConvertible.php

use std::{fmt, sync::Arc};

use php_value::{PhpString, Thrown, Value, numeric::var_export_float};

/// An exception given as `$previous`, compared by identity.
pub type Previous = Arc<dyn std::error::Error + Send + Sync>;

/// The subclass of `ConversionException` thrown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionExceptionKind {
    /// Thrown when a Database to Doctrine Type Conversion fails and we can
    /// make a statement about the expected format.
    InvalidFormat,
    /// Thrown when the PHP value passed to the converter was not of the
    /// expected type.
    InvalidType,
    SerializationFailed,
    /// Thrown when a Database to Doctrine Type Conversion fails.
    ValueNotConvertible,
}

impl ConversionExceptionKind {
    pub fn class(self) -> &'static str {
        match self {
            Self::InvalidFormat => "Doctrine\\DBAL\\Types\\Exception\\InvalidFormat",
            Self::InvalidType => "Doctrine\\DBAL\\Types\\Exception\\InvalidType",
            Self::SerializationFailed => "Doctrine\\DBAL\\Types\\Exception\\SerializationFailed",
            Self::ValueNotConvertible => "Doctrine\\DBAL\\Types\\Exception\\ValueNotConvertible",
        }
    }
}

/// Conversion Exception is thrown when the database to PHP conversion
/// fails.
#[derive(Clone)]
pub struct ConversionException {
    kind: ConversionExceptionKind,
    message: PhpString,
    previous: Option<Previous>,
}

/// `strlen($value) > 32 ? substr($value, 0, 20) . '...' : $value`.
fn shorten(value: &[u8]) -> Vec<u8> {
    if value.len() > 32 {
        let mut short = value[..20].to_vec();
        short.extend_from_slice(b"...");
        short
    } else {
        value.to_vec()
    }
}

/// `var_export($value, true)` for a scalar or null.
fn var_export_scalar(value: &Value) -> Vec<u8> {
    match value {
        Value::Null => b"NULL".to_vec(),
        Value::Bool(true) => b"true".to_vec(),
        Value::Bool(false) => b"false".to_vec(),
        Value::Int(i) => i.to_string().into_bytes(),
        Value::Float(f) => var_export_float(*f).into_bytes(),
        Value::String(s) => {
            let mut out = b"'".to_vec();
            for &byte in s.as_bytes() {
                match byte {
                    b'\'' | b'\\' => out.extend_from_slice(&[b'\\', byte]),
                    0 => out.extend_from_slice(b"' . \"\\0\" . '"),
                    _ => out.push(byte),
                }
            }
            out.push(b'\'');
            out
        }
        Value::Array(_) | Value::Object(_) => Vec::new(),
    }
}

impl ConversionException {
    fn new(kind: ConversionExceptionKind, message: Vec<u8>, previous: Option<Previous>) -> Self {
        Self {
            kind,
            message: PhpString::from(message),
            previous,
        }
    }

    /// `InvalidFormat::new($value, $toType, $expectedFormat, $previous)`.
    pub fn invalid_format(
        value: &[u8],
        to_type: &str,
        expected_format: Option<&str>,
        previous: Option<Previous>,
    ) -> Self {
        let mut message = b"Could not convert database value \"".to_vec();
        message.extend(shorten(value));
        message.extend_from_slice(
            format!(
                "\" to Doctrine Type {to_type}. Expected format \"{}\".",
                expected_format.unwrap_or("")
            )
            .as_bytes(),
        );
        Self::new(ConversionExceptionKind::InvalidFormat, message, previous)
    }

    /// `InvalidType::new($value, $toType, $possibleTypes, $previous)`.
    pub fn invalid_type(
        value: &Value,
        to_type: &str,
        possible_types: &[&str],
        previous: Option<Previous>,
    ) -> Self {
        let mut message = b"Could not convert PHP value ".to_vec();
        if matches!(value, Value::Array(_) | Value::Object(_)) {
            message.extend_from_slice(format!("of type {}", value.debug_type()).as_bytes());
        } else {
            message.extend(var_export_scalar(value));
        }
        message.extend_from_slice(
            format!(
                " to type {to_type}. Expected one of the following types: {}.",
                possible_types.join(", ")
            )
            .as_bytes(),
        );
        Self::new(ConversionExceptionKind::InvalidType, message, previous)
    }

    /// `SerializationFailed::new($value, $format, $error, $previous)`.
    pub fn serialization_failed(
        value: &Value,
        format: &str,
        error: &str,
        previous: Option<Previous>,
    ) -> Self {
        let message = format!(
            "Could not convert PHP type \"{}\" to \"{format}\". An error was triggered by the serialization: {error}",
            value.debug_type()
        );
        Self::new(
            ConversionExceptionKind::SerializationFailed,
            message.into_bytes(),
            previous,
        )
    }

    /// `ValueNotConvertible::new($value, $toType, $message, $previous)`.
    /// Without a message the value is written with `%s`: an object without
    /// `__toString()` makes PHP throw an `Error` instead (`Err`), an array
    /// is "Array" (PHP also warns).
    pub fn value_not_convertible(
        value: &Value,
        to_type: &str,
        message: Option<&[u8]>,
        previous: Option<Previous>,
    ) -> Result<Self, Thrown> {
        let text = if let Some(message) = message {
            let mut text = format!(
                "Could not convert database value to \"{to_type}\" as an error was triggered by the unserialization: "
            )
            .into_bytes();
            text.extend_from_slice(message);
            text
        } else {
            let shown = match value {
                Value::String(s) => shorten(s.as_bytes()),
                Value::Array(_) => b"Array".to_vec(),
                other => other
                    .to_php_string_or_stringable()
                    .ok_or_else(|| {
                        Thrown::new(
                            "Error",
                            format!(
                                "Object of class {} could not be converted to string",
                                other.debug_type()
                            ),
                            0,
                        )
                    })?
                    .as_bytes()
                    .to_vec(),
            };
            let mut text = b"Could not convert database value \"".to_vec();
            text.extend(shown);
            text.extend_from_slice(format!("\" to Doctrine Type \"{to_type}\".").as_bytes());
            text
        };
        Ok(Self::new(
            ConversionExceptionKind::ValueNotConvertible,
            text,
            previous,
        ))
    }

    pub fn kind(&self) -> ConversionExceptionKind {
        self.kind
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `getPrevious()`.
    pub fn previous(&self) -> Option<&Previous> {
        self.previous.as_ref()
    }
}

impl fmt::Debug for ConversionException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConversionException")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("previous", &self.previous.as_ref().map(ToString::to_string))
            .finish()
    }
}

impl fmt::Display for ConversionException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.kind.class(),
            self.message.to_string_lossy()
        )
    }
}

impl std::error::Error for ConversionException {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

//! Port of Symfony\Component\Config\Definition\Exception: Exception,
//! `InvalidConfigurationException` (path and hints) and its subclasses,
//! `UnsetKeyException` and `InvalidDefinitionException`; and
//! Symfony\Component\Config\Exception\LogicException.
//!
//! PHP's exceptions form a class hierarchy that callers test with
//! `instanceof`; here one [`Error`] carries its [`Class`], and the
//! `is_*` methods answer those tests. The built-in classes the Definition
//! code throws (`\InvalidArgumentException`, `\RuntimeException`,
//! `\TypeError`, `\Error`) and the exceptions a user closure throws are
//! classes too, so `BaseNode::finalize()` can tell them apart as upstream
//! does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Exception/Exception.php Definition/Exception/InvalidConfigurationException.php Definition/Exception/InvalidTypeException.php Definition/Exception/DuplicateKeyException.php Definition/Exception/ForbiddenOverwriteException.php Definition/Exception/UnsetKeyException.php Definition/Exception/InvalidDefinitionException.php Exception/LogicException.php

use std::fmt;

use php_value::PhpString;

/// The class of an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// `Definition\Exception\Exception` (extends `\RuntimeException`).
    Exception,
    InvalidConfiguration,
    InvalidType,
    DuplicateKey,
    ForbiddenOverwrite,
    UnsetKey,
    InvalidDefinition,
    /// `Symfony\Component\Config\Exception\LogicException`.
    Logic,
    /// `Symfony\Component\Config\Exception\LoaderLoadException` (extends
    /// `\Exception`; see [`crate::exception`]).
    LoaderLoad,
    /// `Exception\FileLoaderImportCircularReferenceException` (extends
    /// `LoaderLoadException`).
    FileLoaderImportCircularReference,
    /// `Exception\FileLocatorFileNotFoundException` (extends
    /// `\InvalidArgumentException`), with the paths tried.
    FileLocatorFileNotFound,
    /// `\InvalidArgumentException`.
    InvalidArgument,
    /// `Util\Exception\XmlParsingException` (extends
    /// `\InvalidArgumentException`; see [`crate::util::exception`]).
    XmlParsing,
    /// `Util\Exception\InvalidXmlException` (extends `XmlParsingException`).
    InvalidXml,
    /// `\RuntimeException`.
    Runtime,
    /// `\UnexpectedValueException` (extends `\RuntimeException`).
    UnexpectedValue,
    /// `\TypeError`, an `\Error`: what PHP throws for a value of the wrong
    /// type where upstream relies on a typed signature.
    TypeError,
    /// `\Error`: what PHP throws for a call to a method the class does not
    /// have, or an uninitialized typed property.
    Error,
    /// Any other class, as a closure throws it; `exception` tells an
    /// `\Exception` from an `\Error`.
    Other {
        class: String,
        exception: bool,
    },
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub fn name(&self) -> &str {
        match self {
            Self::Exception => "Symfony\\Component\\Config\\Definition\\Exception\\Exception",
            Self::InvalidConfiguration => {
                "Symfony\\Component\\Config\\Definition\\Exception\\InvalidConfigurationException"
            }
            Self::InvalidType => {
                "Symfony\\Component\\Config\\Definition\\Exception\\InvalidTypeException"
            }
            Self::DuplicateKey => {
                "Symfony\\Component\\Config\\Definition\\Exception\\DuplicateKeyException"
            }
            Self::ForbiddenOverwrite => {
                "Symfony\\Component\\Config\\Definition\\Exception\\ForbiddenOverwriteException"
            }
            Self::UnsetKey => {
                "Symfony\\Component\\Config\\Definition\\Exception\\UnsetKeyException"
            }
            Self::InvalidDefinition => {
                "Symfony\\Component\\Config\\Definition\\Exception\\InvalidDefinitionException"
            }
            Self::Logic => "Symfony\\Component\\Config\\Exception\\LogicException",
            Self::LoaderLoad => "Symfony\\Component\\Config\\Exception\\LoaderLoadException",
            Self::FileLoaderImportCircularReference => {
                "Symfony\\Component\\Config\\Exception\\FileLoaderImportCircularReferenceException"
            }
            Self::FileLocatorFileNotFound => {
                "Symfony\\Component\\Config\\Exception\\FileLocatorFileNotFoundException"
            }
            Self::InvalidArgument => "InvalidArgumentException",
            Self::XmlParsing => "Symfony\\Component\\Config\\Util\\Exception\\XmlParsingException",
            Self::InvalidXml => "Symfony\\Component\\Config\\Util\\Exception\\InvalidXmlException",
            Self::Runtime => "RuntimeException",
            Self::UnexpectedValue => "UnexpectedValueException",
            Self::TypeError => "TypeError",
            Self::Error => "Error",
            Self::Other { class, .. } => class,
        }
    }

    /// `instanceof Definition\Exception\Exception`.
    pub fn is_definition_exception(&self) -> bool {
        matches!(
            self,
            Self::Exception
                | Self::InvalidConfiguration
                | Self::InvalidType
                | Self::DuplicateKey
                | Self::ForbiddenOverwrite
                | Self::UnsetKey
                | Self::InvalidDefinition
        )
    }

    /// `instanceof InvalidConfigurationException`.
    pub fn is_invalid_configuration(&self) -> bool {
        matches!(
            self,
            Self::InvalidConfiguration
                | Self::InvalidType
                | Self::DuplicateKey
                | Self::ForbiddenOverwrite
        )
    }

    /// `instanceof \RuntimeException`.
    pub fn is_runtime(&self) -> bool {
        self.is_definition_exception() || matches!(self, Self::Runtime | Self::UnexpectedValue)
    }

    /// `instanceof \LogicException` (of the classes known here).
    pub fn is_logic(&self) -> bool {
        matches!(self, Self::Logic) || self.is_invalid_argument()
    }

    /// `instanceof \InvalidArgumentException`.
    pub fn is_invalid_argument(&self) -> bool {
        matches!(self, Self::InvalidArgument | Self::FileLocatorFileNotFound)
            || self.is_xml_parsing()
    }

    /// `instanceof LoaderLoadException`.
    pub fn is_loader_load(&self) -> bool {
        matches!(
            self,
            Self::LoaderLoad | Self::FileLoaderImportCircularReference
        )
    }

    /// `instanceof XmlParsingException`.
    pub fn is_xml_parsing(&self) -> bool {
        matches!(self, Self::XmlParsing | Self::InvalidXml)
    }

    /// `$this instanceof $class`, for the classes of this module.
    pub fn is_a(&self, class: &Class) -> bool {
        match class {
            Self::Exception => self.is_definition_exception(),
            Self::InvalidConfiguration => self.is_invalid_configuration(),
            Self::Runtime => self.is_runtime(),
            Self::InvalidArgument => self.is_invalid_argument(),
            Self::XmlParsing => self.is_xml_parsing(),
            Self::LoaderLoad => self.is_loader_load(),
            Self::Error => matches!(self, Self::Error | Self::TypeError),
            Self::Other { class, .. } => self.name() == class,
            other => self == other,
        }
    }

    /// `instanceof \Exception`, as opposed to an `\Error`.
    pub fn is_exception(&self) -> bool {
        match self {
            Self::TypeError | Self::Error => false,
            Self::Other { exception, .. } => *exception,
            _ => true,
        }
    }
}

/// A thrown exception or error.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    class: Class,
    message: Vec<u8>,
    code: i64,
    previous: Option<Box<Error>>,
    /// `InvalidConfigurationException`'s path.
    path: Option<PhpString>,
    /// `FileLocatorFileNotFoundException`'s `getPaths()`.
    paths: Vec<Vec<u8>>,
    contains_hints: bool,
}

impl Error {
    pub fn new(class: Class, message: impl Into<Vec<u8>>) -> Self {
        Self {
            class,
            message: message.into(),
            code: 0,
            previous: None,
            path: None,
            paths: Vec::new(),
            contains_hints: false,
        }
    }

    /// `new $class($message, $code, $previous)`.
    pub fn with_previous(
        class: Class,
        message: impl Into<Vec<u8>>,
        code: i64,
        previous: Error,
    ) -> Self {
        Self {
            code,
            previous: Some(Box::new(previous)),
            ..Self::new(class, message)
        }
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    pub fn get_message(&self) -> &[u8] {
        &self.message
    }

    pub fn get_code(&self) -> i64 {
        self.code
    }

    pub fn get_previous(&self) -> Option<&Error> {
        self.previous.as_deref()
    }

    pub fn set_path(&mut self, path: PhpString) {
        self.path = Some(path);
    }

    pub fn get_path(&self) -> Option<&PhpString> {
        self.path.as_ref()
    }

    /// The exception's `$code`.
    #[must_use]
    pub fn with_code(mut self, code: i64) -> Self {
        self.code = code;
        self
    }

    /// `FileLocatorFileNotFoundException`'s `$paths`.
    #[expect(
        clippy::return_self_not_must_use,
        reason = "a PHP fluent call: it changes the shared builder and returns $this"
    )]
    pub fn with_paths(mut self, paths: Vec<Vec<u8>>) -> Self {
        self.paths = paths;
        self
    }

    /// `getPaths()`: the paths a `FileLocatorFileNotFoundException` tried.
    pub fn get_paths(&self) -> &[Vec<u8>] {
        &self.paths
    }

    /// Appends "\nHint: " and the hint, or ", " and the hint after the
    /// first one.
    pub fn add_hint(&mut self, hint: &[u8]) {
        if self.contains_hints {
            self.message.extend_from_slice(b", ");
        } else {
            self.message.extend_from_slice(b"\nHint: ");
            self.contains_hints = true;
        }
        self.message.extend_from_slice(hint);
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.class.name(),
            String::from_utf8_lossy(&self.message)
        )
    }
}

impl std::error::Error for Error {}

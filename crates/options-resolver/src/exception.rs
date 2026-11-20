//! Port of Symfony\Component\OptionsResolver\Exception\*.php: the
//! component's exception classes and their marker interface.
//!
//! PHP's exceptions form a class hierarchy that callers test with
//! `instanceof`; here one [`Error`] carries its [`Class`], and
//! [`Class::is_a`] answers those tests. An exception a user closure
//! throws is [`Class::Other`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/options-resolver Exception/ExceptionInterface.php Exception/AccessException.php Exception/InvalidArgumentException.php Exception/InvalidOptionsException.php Exception/MissingOptionsException.php Exception/NoConfigurationException.php Exception/NoSuchOptionException.php Exception/OptionDefinitionException.php Exception/UndefinedOptionsException.php

use std::fmt;

use php_value::PhpString;

/// The class of an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// `AccessException` (extends `\LogicException`).
    Access,
    /// `Exception\InvalidArgumentException` (extends
    /// `\InvalidArgumentException`).
    InvalidArgument,
    /// `InvalidOptionsException` (extends `InvalidArgumentException`).
    InvalidOptions,
    /// `MissingOptionsException` (extends `InvalidArgumentException`).
    MissingOptions,
    /// `NoConfigurationException` (extends `\RuntimeException`).
    NoConfiguration,
    /// `NoSuchOptionException` (extends `\OutOfBoundsException`).
    NoSuchOption,
    /// `OptionDefinitionException` (extends `\LogicException`).
    OptionDefinition,
    /// `UndefinedOptionsException` (extends `InvalidArgumentException`).
    UndefinedOptions,
    /// Any other class, as a user closure throws it; `exception` tells an
    /// `\Exception` from an `\Error`.
    Other { class: String, exception: bool },
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub fn name(&self) -> &str {
        match self {
            Self::Access => "Symfony\\Component\\OptionsResolver\\Exception\\AccessException",
            Self::InvalidArgument => {
                "Symfony\\Component\\OptionsResolver\\Exception\\InvalidArgumentException"
            }
            Self::InvalidOptions => {
                "Symfony\\Component\\OptionsResolver\\Exception\\InvalidOptionsException"
            }
            Self::MissingOptions => {
                "Symfony\\Component\\OptionsResolver\\Exception\\MissingOptionsException"
            }
            Self::NoConfiguration => {
                "Symfony\\Component\\OptionsResolver\\Exception\\NoConfigurationException"
            }
            Self::NoSuchOption => {
                "Symfony\\Component\\OptionsResolver\\Exception\\NoSuchOptionException"
            }
            Self::OptionDefinition => {
                "Symfony\\Component\\OptionsResolver\\Exception\\OptionDefinitionException"
            }
            Self::UndefinedOptions => {
                "Symfony\\Component\\OptionsResolver\\Exception\\UndefinedOptionsException"
            }
            Self::Other { class, .. } => class,
        }
    }

    /// `instanceof Exception\ExceptionInterface`.
    pub fn is_exception_interface(&self) -> bool {
        !matches!(self, Self::Other { .. })
    }

    /// `instanceof Exception\InvalidArgumentException`.
    pub fn is_invalid_argument(&self) -> bool {
        matches!(
            self,
            Self::InvalidArgument
                | Self::InvalidOptions
                | Self::MissingOptions
                | Self::UndefinedOptions
        )
    }

    /// `instanceof \LogicException` (of the classes known here;
    /// `\InvalidArgumentException` extends it).
    pub fn is_logic(&self) -> bool {
        matches!(self, Self::Access | Self::OptionDefinition) || self.is_invalid_argument()
    }

    /// `instanceof \RuntimeException` (of the classes known here;
    /// `\OutOfBoundsException` extends it).
    pub fn is_runtime(&self) -> bool {
        matches!(self, Self::NoConfiguration | Self::NoSuchOption)
    }

    /// `instanceof \Exception`, as opposed to an `\Error`.
    pub fn is_exception(&self) -> bool {
        match self {
            Self::Other { exception, .. } => *exception,
            _ => true,
        }
    }

    /// `$this instanceof $class`, for the classes of this module.
    pub fn is_a(&self, class: &Class) -> bool {
        match class {
            Self::InvalidArgument => self.is_invalid_argument(),
            Self::Other { class, .. } => self.name() == class,
            other => self == other,
        }
    }
}

/// An exception: its class and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    class: Class,
    message: PhpString,
}

impl Error {
    pub fn new(class: Class, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }

    /// An exception of another class, as a user closure throws it.
    pub fn other(class: impl Into<String>, message: impl Into<PhpString>) -> Self {
        Self::new(
            Class::Other {
                class: class.into(),
                exception: true,
            },
            message,
        )
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `instanceof $class`.
    pub fn is_a(&self, class: &Class) -> bool {
        self.class.is_a(class)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class.name(), self.message)
    }
}

impl std::error::Error for Error {}

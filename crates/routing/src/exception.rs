//! Port of Symfony\Component\Routing\Exception\ExceptionInterface.php,
//! InvalidArgumentException.php, RuntimeException.php and
//! RouteCircularReferenceException.php, with the PHP exceptions that
//! Route, RouteCompiler and RouteCollection throw.
//!
//! One error type carries the class PHP would throw, so callers can test
//! it as `catch`/`instanceof` would ([`Error::is_a`]). Messages are
//! upstream's, built from its format strings; they are bytes, since a
//! route pattern need not be UTF-8.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Exception/ExceptionInterface.php Exception/InvalidArgumentException.php Exception/RuntimeException.php Exception/RouteCircularReferenceException.php

use php_value::PhpString;

/// The exception classes this part of the crate throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\Routing\Exception\InvalidArgumentException`.
    InvalidArgument,
    /// `Symfony\Component\Routing\Exception\RouteCircularReferenceException`.
    RouteCircularReference,
    /// PHP's `\InvalidArgumentException`.
    PhpInvalidArgument,
    /// PHP's `\LogicException`.
    PhpLogic,
    /// PHP's `\DomainException`.
    PhpDomain,
    /// PHP's `\BadMethodCallException`.
    PhpBadMethodCall,
    /// PHP's `\Exception` (`serialize()` of a `Closure`).
    PhpException,
    /// PHP's `\TypeError`.
    PhpTypeError,
    /// PHP's `\Error`.
    PhpError,
    /// Input this port does not reproduce; not a PHP class. It is refused
    /// rather than approximated.
    NotPorted,
}

const INTERFACE: &str = "Symfony\\Component\\Routing\\Exception\\ExceptionInterface";

impl Class {
    /// The class name, as `$e::class` gives it.
    pub const fn name(self) -> &'static str {
        self.ancestry()[0]
    }

    /// The classes and interfaces it is an instance of, itself first.
    const fn ancestry(self) -> &'static [&'static str] {
        match self {
            Self::InvalidArgument => &[
                "Symfony\\Component\\Routing\\Exception\\InvalidArgumentException",
                INTERFACE,
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::RouteCircularReference => &[
                "Symfony\\Component\\Routing\\Exception\\RouteCircularReferenceException",
                "Symfony\\Component\\Routing\\Exception\\RuntimeException",
                INTERFACE,
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::PhpInvalidArgument => &[
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpLogic => &["LogicException", "Exception", "Throwable"],
            Self::PhpDomain => &[
                "DomainException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpBadMethodCall => &[
                "BadMethodCallException",
                "BadFunctionCallException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpException => &["Exception", "Throwable"],
            Self::PhpTypeError => &["TypeError", "Error", "Throwable"],
            Self::PhpError => &["Error", "Throwable"],
            Self::NotPorted => &["NotPorted"],
        }
    }
}

/// An exception thrown by a route, its compiler or a collection.
#[derive(Debug, Clone, PartialEq)]
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

    pub fn class(&self) -> Class {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `$e instanceof $class`.
    pub fn is_a(&self, class: &str) -> bool {
        self.class.ancestry().contains(&class)
    }

    /// `new RouteCircularReferenceException($routeId, $path)`.
    pub(crate) fn circular_reference(route_id: &[u8], path: &[&[u8]]) -> Self {
        let mut message = b"Circular reference detected for route \"".to_vec();
        message.extend_from_slice(route_id);
        message.extend_from_slice(b"\", path: \"");
        message.extend_from_slice(&path.join(&b" -> "[..]));
        message.extend_from_slice(b"\".");
        Self::new(Class::RouteCircularReference, message)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class.name(), self.message)
    }
}

impl std::error::Error for Error {}

/// Builds a message from byte pieces (upstream's `sprintf()` arguments
/// need not be UTF-8).
pub(crate) fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

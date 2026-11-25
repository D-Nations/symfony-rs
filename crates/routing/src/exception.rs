//! Port of Symfony\Component\Routing\Exception\*.php: the routing
//! exceptions, with the PHP exceptions that Route, `RouteCompiler`,
//! `RouteCollection` and `RequestContext` throw.
//!
//! One error type carries the class PHP would throw, so callers can test
//! it as `catch`/`instanceof` would ([`Error::is_a`]). Messages are
//! upstream's, built from its format strings; they are bytes, since a
//! route pattern need not be UTF-8. `MethodNotAllowedException`'s allowed
//! methods and `MissingMandatoryParametersException`'s route name and
//! parameters are [`Detail`]s.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing Exception/ExceptionInterface.php Exception/InvalidArgumentException.php Exception/InvalidParameterException.php Exception/LogicException.php Exception/MethodNotAllowedException.php Exception/MissingMandatoryParametersException.php Exception/NoConfigurationException.php Exception/ResourceNotFoundException.php Exception/RouteCircularReferenceException.php Exception/RouteNotFoundException.php Exception/RuntimeException.php

use php_value::PhpString;

/// The exception classes this crate throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\Routing\Exception\InvalidArgumentException`.
    InvalidArgument,
    /// `Symfony\Component\Routing\Exception\InvalidParameterException`: a
    /// parameter does not match its requirement when generating a URL.
    InvalidParameter,
    /// `Symfony\Component\Routing\Exception\LogicException` (not an
    /// `ExceptionInterface`).
    Logic,
    /// `Symfony\Component\Routing\Exception\MethodNotAllowedException`
    /// (HTTP 405), with [`Detail::AllowedMethods`].
    MethodNotAllowed,
    /// `Symfony\Component\Routing\Exception\MissingMandatoryParametersException`,
    /// with [`Detail::MissingParameters`].
    MissingMandatoryParameters,
    /// `Symfony\Component\Routing\Exception\NoConfigurationException`: no
    /// routes are configured.
    NoConfiguration,
    /// `Symfony\Component\Routing\Exception\ResourceNotFoundException`
    /// (HTTP 404).
    ResourceNotFound,
    /// `Symfony\Component\Routing\Exception\RouteCircularReferenceException`.
    RouteCircularReference,
    /// `Symfony\Component\Routing\Exception\RouteNotFoundException`: no
    /// route of that name.
    RouteNotFound,
    /// `Symfony\Component\Routing\Exception\RuntimeException`.
    Runtime,
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
            Self::InvalidParameter => &[
                "Symfony\\Component\\Routing\\Exception\\InvalidParameterException",
                INTERFACE,
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::Logic => &[
                "Symfony\\Component\\Routing\\Exception\\LogicException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::MethodNotAllowed => &[
                "Symfony\\Component\\Routing\\Exception\\MethodNotAllowedException",
                INTERFACE,
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::MissingMandatoryParameters => &[
                "Symfony\\Component\\Routing\\Exception\\MissingMandatoryParametersException",
                INTERFACE,
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::NoConfiguration => &[
                "Symfony\\Component\\Routing\\Exception\\NoConfigurationException",
                "Symfony\\Component\\Routing\\Exception\\ResourceNotFoundException",
                INTERFACE,
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::ResourceNotFound => &[
                "Symfony\\Component\\Routing\\Exception\\ResourceNotFoundException",
                INTERFACE,
                "RuntimeException",
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
            Self::RouteNotFound => &[
                "Symfony\\Component\\Routing\\Exception\\RouteNotFoundException",
                INTERFACE,
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::Runtime => &[
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

/// What an exception carries besides its message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Detail {
    #[default]
    None,
    /// `MethodNotAllowedException`'s `getAllowedMethods()`: uppercased.
    AllowedMethods(Vec<PhpString>),
    /// `MissingMandatoryParametersException`'s `getRouteName()` and
    /// `getMissingParameters()`.
    MissingParameters {
        route_name: PhpString,
        parameters: Vec<PhpString>,
    },
}

/// An exception thrown by routing.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    class: Class,
    message: PhpString,
    detail: Detail,
}

impl Error {
    pub fn new(class: Class, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
            detail: Detail::None,
        }
    }

    /// `new MethodNotAllowedException($allowedMethods, $message)`: the
    /// methods are uppercased.
    pub fn method_not_allowed<S: AsRef<[u8]>>(
        allowed_methods: impl IntoIterator<Item = S>,
        message: impl Into<PhpString>,
    ) -> Self {
        Self {
            class: Class::MethodNotAllowed,
            message: message.into(),
            detail: Detail::AllowedMethods(
                allowed_methods
                    .into_iter()
                    .map(|m| PhpString::from(m.as_ref().to_ascii_uppercase()))
                    .collect(),
            ),
        }
    }

    /// `new MissingMandatoryParametersException($routeName,
    /// $missingParameters)`, with upstream's message.
    pub fn missing_mandatory_parameters<S: AsRef<[u8]>>(
        route_name: &[u8],
        missing_parameters: impl IntoIterator<Item = S>,
    ) -> Self {
        let parameters: Vec<PhpString> = missing_parameters
            .into_iter()
            .map(|p| PhpString::from(p.as_ref()))
            .collect();
        let joined: Vec<&[u8]> = parameters
            .iter()
            .map(php_value::PhpString::as_bytes)
            .collect();
        let message = concat(&[
            b"Some mandatory parameters are missing (\"",
            &joined.join(&b"\", \""[..]),
            b"\") to generate a URL for route \"",
            route_name,
            b"\".",
        ]);
        Self {
            class: Class::MissingMandatoryParameters,
            message: message.into(),
            detail: Detail::MissingParameters {
                route_name: PhpString::from(route_name),
                parameters,
            },
        }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// The allowed methods, route name or missing parameters.
    pub fn detail(&self) -> &Detail {
        &self.detail
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

//! Port of Symfony\Component\HttpFoundation\Exception\BadRequestException.php,
//! UnexpectedValueException.php, RequestExceptionInterface.php,
//! SuspiciousOperationException.php, ConflictingHeadersException.php,
//! JsonException.php and SessionNotFoundException.php, with
//! the PHP exceptions the bags throw (`\UnexpectedValueException`,
//! `\InvalidArgumentException`) and PHP's `\Error` for an enum class that
//! has no `from()`; and the File exceptions (`File/Exception/*.php`)
//! with symfony/mime's, which `File` passes on.
//!
//! One error type carries the class PHP would throw, so callers can test
//! it as `catch`/`instanceof` would ([`Error::is_a`]). Messages are
//! upstream's, built from its format strings; they are bytes, since an
//! enum `\ValueError` message quotes the value as given.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Exception/BadRequestException.php Exception/UnexpectedValueException.php Exception/RequestExceptionInterface.php Exception/SuspiciousOperationException.php Exception/ConflictingHeadersException.php Exception/JsonException.php Exception/SessionNotFoundException.php File/Exception/FileException.php File/Exception/FileNotFoundException.php File/Exception/AccessDeniedException.php File/Exception/CannotWriteFileException.php File/Exception/ExtensionFileException.php File/Exception/FormSizeFileException.php File/Exception/IniSizeFileException.php File/Exception/NoFileException.php File/Exception/NoTmpDirFileException.php File/Exception/PartialFileException.php File/Exception/UnexpectedTypeException.php File/Exception/UploadException.php

use php_value::PhpString;

/// The exception classes this crate throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\HttpFoundation\Exception\BadRequestException`,
    /// a `RequestExceptionInterface` (`HttpKernel` answers it with 400).
    BadRequest,
    /// `Symfony\Component\HttpFoundation\Exception\UnexpectedValueException`.
    UnexpectedValue,
    /// `Symfony\Component\HttpFoundation\Exception\SuspiciousOperationException`,
    /// a `RequestExceptionInterface` (`HttpKernel` answers it with 400).
    SuspiciousOperation,
    /// `Symfony\Component\HttpFoundation\Exception\ConflictingHeadersException`,
    /// a `RequestExceptionInterface`.
    ConflictingHeaders,
    /// `Symfony\Component\HttpFoundation\Exception\JsonException`, a
    /// `RequestExceptionInterface`.
    Json,
    /// `Symfony\Component\HttpFoundation\Exception\SessionNotFoundException`,
    /// a `RequestExceptionInterface` (`RequestStack::getSession()`).
    SessionNotFound,
    /// PHP's `\JsonException` (`json_decode()` with `JSON_THROW_ON_ERROR`).
    PhpJson,
    /// PHP's `\LogicException`.
    PhpLogic,
    /// PHP's `\DomainException`, a `\LogicException` (a marshaller's
    /// unreadable data).
    PhpDomain,
    /// PHP's `\UnexpectedValueException`.
    PhpUnexpectedValue,
    /// PHP's `\InvalidArgumentException`.
    PhpInvalidArgument,
    /// PHP's `\RuntimeException` (`HeaderBag::getDate()`).
    PhpRuntime,
    /// PHP's `\TypeError`.
    PhpTypeError,
    /// PHP's `\ValueError` (`IpUtils::isPrivateIp()`, `inet_pton()`).
    PhpValueError,
    /// PHP's `\Error`.
    PhpError,
    /// `Symfony\Component\HttpFoundation\File\Exception\FileException`.
    File,
    /// `Symfony\Component\HttpFoundation\File\Exception\FileNotFoundException`, a `FileException`.
    FileNotFound,
    /// `Symfony\Component\HttpFoundation\File\Exception\AccessDeniedException`, a `FileException`.
    AccessDenied,
    /// `Symfony\Component\HttpFoundation\File\Exception\CannotWriteFileException`, a `FileException`.
    CannotWriteFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\ExtensionFileException`, a `FileException`.
    ExtensionFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\FormSizeFileException`, a `FileException`.
    FormSizeFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\IniSizeFileException`, a `FileException`.
    IniSizeFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\NoFileException`, a `FileException`.
    NoFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\NoTmpDirFileException`, a `FileException`.
    NoTmpDirFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\PartialFileException`, a `FileException`.
    PartialFile,
    /// `Symfony\Component\HttpFoundation\File\Exception\UnexpectedTypeException`, a `FileException`.
    UnexpectedType,
    /// `Symfony\Component\HttpFoundation\File\Exception\UploadException`, a `FileException`.
    Upload,
    /// `Symfony\Component\Mime\Exception\LogicException`.
    MimeLogic,
    /// `Symfony\Component\Mime\Exception\InvalidArgumentException`.
    MimeInvalidArgument,
    /// `Symfony\Component\Mime\Exception\RuntimeException`.
    MimeRuntime,
    /// Input this port does not reproduce; not a PHP class. It is refused
    /// rather than approximated.
    NotPorted,
    /// An exception of another type a callback threw (a save handler's),
    /// passed on as it is ([`Error::thrown`]).
    Thrown,
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::BadRequest => {
                "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException"
            }
            Self::UnexpectedValue => {
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException"
            }
            Self::SuspiciousOperation => {
                "Symfony\\Component\\HttpFoundation\\Exception\\SuspiciousOperationException"
            }
            Self::ConflictingHeaders => {
                "Symfony\\Component\\HttpFoundation\\Exception\\ConflictingHeadersException"
            }
            Self::Json => "Symfony\\Component\\HttpFoundation\\Exception\\JsonException",
            Self::SessionNotFound => {
                "Symfony\\Component\\HttpFoundation\\Exception\\SessionNotFoundException"
            }
            Self::PhpJson => "JsonException",
            Self::PhpLogic => "LogicException",
            Self::PhpDomain => "DomainException",
            Self::PhpUnexpectedValue => "UnexpectedValueException",
            Self::PhpInvalidArgument => "InvalidArgumentException",
            Self::PhpRuntime => "RuntimeException",
            Self::PhpTypeError => "TypeError",
            Self::PhpValueError => "ValueError",
            Self::PhpError => "Error",
            Self::File => "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
            Self::FileNotFound => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileNotFoundException"
            }
            Self::AccessDenied => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\AccessDeniedException"
            }
            Self::CannotWriteFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\CannotWriteFileException"
            }
            Self::ExtensionFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\ExtensionFileException"
            }
            Self::FormSizeFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FormSizeFileException"
            }
            Self::IniSizeFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\IniSizeFileException"
            }
            Self::NoFile => "Symfony\\Component\\HttpFoundation\\File\\Exception\\NoFileException",
            Self::NoTmpDirFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\NoTmpDirFileException"
            }
            Self::PartialFile => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\PartialFileException"
            }
            Self::UnexpectedType => {
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\UnexpectedTypeException"
            }
            Self::Upload => "Symfony\\Component\\HttpFoundation\\File\\Exception\\UploadException",
            Self::MimeLogic => "Symfony\\Component\\Mime\\Exception\\LogicException",
            Self::MimeInvalidArgument => {
                "Symfony\\Component\\Mime\\Exception\\InvalidArgumentException"
            }
            Self::MimeRuntime => "Symfony\\Component\\Mime\\Exception\\RuntimeException",
            Self::NotPorted => "NotPorted",
            Self::Thrown => "Throwable",
        }
    }

    /// The classes and interfaces it is an instance of, itself first.
    #[expect(clippy::too_many_lines, reason = "one arm for each upstream class")]
    pub const fn ancestry(self) -> &'static [&'static str] {
        match self {
            Self::BadRequest => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::UnexpectedValue => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::SuspiciousOperation => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\SuspiciousOperationException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::ConflictingHeaders => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\ConflictingHeadersException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::Json => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\JsonException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::SessionNotFound => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\SessionNotFoundException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpJson => &["JsonException", "Exception", "Throwable"],
            Self::PhpLogic => &["LogicException", "Exception", "Throwable"],
            Self::PhpDomain => &[
                "DomainException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpUnexpectedValue => &[
                "UnexpectedValueException",
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
            Self::PhpRuntime => &["RuntimeException", "Exception", "Throwable"],
            Self::PhpTypeError => &["TypeError", "Error", "Throwable"],
            Self::PhpValueError => &["ValueError", "Error", "Throwable"],
            Self::PhpError => &["Error", "Throwable"],
            Self::File => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::FileNotFound => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileNotFoundException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::AccessDenied => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\AccessDeniedException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::CannotWriteFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\CannotWriteFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::ExtensionFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\ExtensionFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::FormSizeFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FormSizeFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::IniSizeFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\IniSizeFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::NoFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\NoFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::NoTmpDirFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\NoTmpDirFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::PartialFile => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\PartialFileException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::UnexpectedType => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\UnexpectedTypeException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::Upload => &[
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\UploadException",
                "Symfony\\Component\\HttpFoundation\\File\\Exception\\FileException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::MimeLogic => &[
                "Symfony\\Component\\Mime\\Exception\\LogicException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::MimeInvalidArgument => &[
                "Symfony\\Component\\Mime\\Exception\\InvalidArgumentException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::MimeRuntime => &[
                "Symfony\\Component\\Mime\\Exception\\RuntimeException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::NotPorted => &["NotPorted"],
            Self::Thrown => &["Throwable"],
        }
    }
}

/// An exception thrown by a bag.
#[derive(Debug, Clone)]
pub struct Error {
    class: Class,
    message: PhpString,
    code: i64,
    previous: Option<Box<Error>>,
    /// The exception of another type this one carries (`Class::Thrown`).
    thrown: Option<crate::streamed_response::Throwable>,
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class
            && self.message == other.message
            && self.code == other.code
            && self.previous == other.previous
            && match (&self.thrown, &other.thrown) {
                (None, None) => true,
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl Error {
    pub fn new(class: Class, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
            code: 0,
            previous: None,
            thrown: None,
        }
    }

    /// An exception a callback threw: this type's as it is, any other
    /// carried in a `Class::Thrown` ([`Error::thrown`]).
    pub fn from_throwable(error: crate::streamed_response::Throwable) -> Self {
        if let Some(own) = error.downcast_ref::<Self>() {
            return own.clone();
        }
        let message = error.to_string();
        Self {
            thrown: Some(error),
            ..Self::new(Class::Thrown, message)
        }
    }

    /// The exception of another type this one carries.
    pub fn thrown(&self) -> Option<&crate::streamed_response::Throwable> {
        self.thrown.as_ref()
    }

    /// `new $class($message, $code, $previous)`.
    pub fn with_previous(
        class: Class,
        message: impl Into<PhpString>,
        code: i64,
        previous: Error,
    ) -> Self {
        Self {
            code,
            previous: Some(Box::new(previous)),
            ..Self::new(class, message)
        }
    }

    /// The same error with a code, as `new $class($message, $code)`.
    #[must_use]
    pub fn with_code(self, code: i64) -> Self {
        Self { code, ..self }
    }

    /// `getCode()`.
    pub fn code(&self) -> i64 {
        self.code
    }

    /// `getPrevious()`.
    pub fn previous(&self) -> Option<&Error> {
        self.previous.as_deref()
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

    /// The same message as another class, as `InputBag::getEnum()`
    /// rethrows: `new BadRequestException($e->getMessage(), ...)`.
    pub(crate) fn rethrown_as(self, class: Class) -> Self {
        Self { class, ..self }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            self.class.name(),
            self.message.to_string_lossy()
        )
    }
}

impl std::error::Error for Error {}

impl From<php_value::filter::Unsupported> for Error {
    fn from(error: php_value::filter::Unsupported) -> Self {
        Self::new(Class::NotPorted, error.to_string())
    }
}

impl From<symfony_mime::Error> for Error {
    fn from(error: symfony_mime::Error) -> Self {
        let class = match error.class() {
            symfony_mime::Class::Logic => Class::MimeLogic,
            symfony_mime::Class::InvalidArgument => Class::MimeInvalidArgument,
            symfony_mime::Class::Runtime => Class::MimeRuntime,
            symfony_mime::Class::NotPorted => Class::NotPorted,
        };
        Self::new(class, error.message())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

//! Port of Symfony\Component\HttpKernel\Exception\HttpException.php,
//! HttpExceptionInterface.php, its subclasses (`AccessDenied`, `BadRequest`,
//! `Conflict`, `Gone`, `LengthRequired`, `Locked`, `MethodNotAllowed`,
//! `NotAcceptable`, `NotFound`, `PreconditionFailed`, `PreconditionRequired`,
//! `ServiceUnavailable`, `TooManyRequests`, `Unauthorized`,
//! `UnprocessableEntity`, `UnsupportedMediaType`) and
//! ControllerDoesNotReturnResponseException.php; with the PHP throwables
//! the kernel tells apart (`\Error` and its subclasses, `\Exception` and
//! its subclasses).
//!
//! A [`Throwable`] is shared (`Arc`), as a PHP exception object is: the
//! kernel can hand it to a listener, keep a reference (`$this->
//! terminateException`) and rethrow the same object. [`instance_of`] is
//! `instanceof` for the classes this crate and `HttpFoundation` throw: each
//! reports its ancestry, a class name first. Any other Rust error is an
//! `\Exception` (never an `\Error`).
//!
//! `ControllerDoesNotReturnResponseException`'s file, line and trace (the
//! controller's definition, read by reflection) are not reproduced.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Exception/HttpException.php Exception/HttpExceptionInterface.php Exception/AccessDeniedHttpException.php Exception/BadRequestHttpException.php Exception/ConflictHttpException.php Exception/GoneHttpException.php Exception/LengthRequiredHttpException.php Exception/LockedHttpException.php Exception/MethodNotAllowedHttpException.php Exception/NotAcceptableHttpException.php Exception/NotFoundHttpException.php Exception/PreconditionFailedHttpException.php Exception/PreconditionRequiredHttpException.php Exception/ServiceUnavailableHttpException.php Exception/TooManyRequestsHttpException.php Exception/UnauthorizedHttpException.php Exception/UnprocessableEntityHttpException.php Exception/UnsupportedMediaTypeHttpException.php Exception/ControllerDoesNotReturnResponseException.php

use std::{fmt, sync::Arc};

use php_value::{Key, PhpArray, PhpString, Value};

/// A PHP `\Throwable`: shared, so `===` is [`same`].
pub type Throwable = Arc<dyn std::error::Error + Send + Sync + 'static>;

/// `$a === $b` for two throwables.
pub fn same(a: &Throwable, b: &Throwable) -> bool {
    Arc::ptr_eq(a, b)
}

/// A throwable from a listener (the `EventDispatcher`'s boxed error).
pub fn from_box(error: symfony_event_dispatcher::Throwable) -> Throwable {
    Arc::from(error)
}

/// The classes a throwable is an instance of, its class first; an error
/// type this crate does not know is an `\Exception`.
pub fn ancestry(e: &(dyn std::error::Error + 'static)) -> Vec<&'static str> {
    if let Some(e) = e.downcast_ref::<HttpException>() {
        return e.class.ancestry();
    }
    if let Some(e) = e.downcast_ref::<PhpException>() {
        return e.class.ancestry().to_vec();
    }
    if e.downcast_ref::<ControllerDoesNotReturnResponseException>()
        .is_some()
    {
        return vec![
            ControllerDoesNotReturnResponseException::CLASS,
            "LogicException",
            "Exception",
            "Throwable",
        ];
    }
    if let Some(e) = e.downcast_ref::<symfony_http_foundation::exception::Error>() {
        return e.class().ancestry().to_vec();
    }
    vec!["Exception", "Throwable"]
}

/// `$e instanceof $class`.
pub fn instance_of(e: &(dyn std::error::Error + 'static), class: &str) -> bool {
    ancestry(e).contains(&class)
}

/// `$e::class`.
pub fn class_of(e: &(dyn std::error::Error + 'static)) -> &'static str {
    ancestry(e).first().copied().unwrap_or("Exception")
}

/// `Symfony\Component\HttpKernel\Exception\HttpExceptionInterface`.
pub const HTTP_EXCEPTION_INTERFACE: &str =
    "Symfony\\Component\\HttpKernel\\Exception\\HttpExceptionInterface";

/// `Symfony\Component\HttpFoundation\Exception\RequestExceptionInterface`.
pub const REQUEST_EXCEPTION_INTERFACE: &str =
    "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface";

/// The PHP throwables an application or a test throws that are not
/// Symfony classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhpClass {
    Exception,
    RuntimeException,
    LogicException,
    InvalidArgumentException,
    UnexpectedValueException,
    Error,
    TypeError,
    ValueError,
}

impl PhpClass {
    /// The class and what it extends.
    pub const fn ancestry(self) -> &'static [&'static str] {
        match self {
            Self::Exception => &["Exception", "Throwable"],
            Self::RuntimeException => &["RuntimeException", "Exception", "Throwable"],
            Self::LogicException => &["LogicException", "Exception", "Throwable"],
            Self::InvalidArgumentException => &[
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::UnexpectedValueException => &[
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::Error => &["Error", "Throwable"],
            Self::TypeError => &["TypeError", "Error", "Throwable"],
            Self::ValueError => &["ValueError", "Error", "Throwable"],
        }
    }
}

/// A PHP built-in throwable (`new \RuntimeException($message)`).
#[derive(Debug)]
pub struct PhpException {
    class: PhpClass,
    message: PhpString,
    code: i64,
    previous: Option<Throwable>,
}

impl PhpException {
    pub fn new(class: PhpClass, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
            code: 0,
            previous: None,
        }
    }

    /// With `$previous`.
    #[must_use]
    pub fn with_previous(mut self, previous: Throwable) -> Self {
        self.previous = Some(previous);
        self
    }

    /// With `$code`.
    #[must_use]
    pub fn with_code(mut self, code: i64) -> Self {
        self.code = code;
        self
    }

    /// As a shared throwable.
    pub fn throwable(self) -> Throwable {
        Arc::new(self)
    }

    pub fn class(&self) -> PhpClass {
        self.class
    }

    /// `getMessage()`.
    pub fn get_message(&self) -> &PhpString {
        &self.message
    }

    /// `getCode()`.
    pub fn get_code(&self) -> i64 {
        self.code
    }

    /// `getPrevious()`.
    pub fn get_previous(&self) -> Option<&Throwable> {
        self.previous.as_ref()
    }
}

impl fmt::Display for PhpException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl std::error::Error for PhpException {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// `HttpException` and its subclasses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpClass {
    Http,
    AccessDenied,
    BadRequest,
    Conflict,
    Gone,
    LengthRequired,
    Locked,
    MethodNotAllowed,
    NotAcceptable,
    NotFound,
    PreconditionFailed,
    PreconditionRequired,
    ServiceUnavailable,
    TooManyRequests,
    Unauthorized,
    UnprocessableEntity,
    UnsupportedMediaType,
}

const HTTP_EXCEPTION: &str = "Symfony\\Component\\HttpKernel\\Exception\\HttpException";

impl HttpClass {
    /// `$e::class`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Http => HTTP_EXCEPTION,
            Self::AccessDenied => {
                "Symfony\\Component\\HttpKernel\\Exception\\AccessDeniedHttpException"
            }
            Self::BadRequest => {
                "Symfony\\Component\\HttpKernel\\Exception\\BadRequestHttpException"
            }
            Self::Conflict => "Symfony\\Component\\HttpKernel\\Exception\\ConflictHttpException",
            Self::Gone => "Symfony\\Component\\HttpKernel\\Exception\\GoneHttpException",
            Self::LengthRequired => {
                "Symfony\\Component\\HttpKernel\\Exception\\LengthRequiredHttpException"
            }
            Self::Locked => "Symfony\\Component\\HttpKernel\\Exception\\LockedHttpException",
            Self::MethodNotAllowed => {
                "Symfony\\Component\\HttpKernel\\Exception\\MethodNotAllowedHttpException"
            }
            Self::NotAcceptable => {
                "Symfony\\Component\\HttpKernel\\Exception\\NotAcceptableHttpException"
            }
            Self::NotFound => "Symfony\\Component\\HttpKernel\\Exception\\NotFoundHttpException",
            Self::PreconditionFailed => {
                "Symfony\\Component\\HttpKernel\\Exception\\PreconditionFailedHttpException"
            }
            Self::PreconditionRequired => {
                "Symfony\\Component\\HttpKernel\\Exception\\PreconditionRequiredHttpException"
            }
            Self::ServiceUnavailable => {
                "Symfony\\Component\\HttpKernel\\Exception\\ServiceUnavailableHttpException"
            }
            Self::TooManyRequests => {
                "Symfony\\Component\\HttpKernel\\Exception\\TooManyRequestsHttpException"
            }
            Self::Unauthorized => {
                "Symfony\\Component\\HttpKernel\\Exception\\UnauthorizedHttpException"
            }
            Self::UnprocessableEntity => {
                "Symfony\\Component\\HttpKernel\\Exception\\UnprocessableEntityHttpException"
            }
            Self::UnsupportedMediaType => {
                "Symfony\\Component\\HttpKernel\\Exception\\UnsupportedMediaTypeHttpException"
            }
        }
    }

    fn ancestry(self) -> Vec<&'static str> {
        let mut classes = Vec::with_capacity(6);
        if self != Self::Http {
            classes.push(self.name());
        }
        classes.extend([
            HTTP_EXCEPTION,
            HTTP_EXCEPTION_INTERFACE,
            "RuntimeException",
            "Exception",
            "Throwable",
        ]);
        classes
    }
}

/// The arguments the subclasses share after their own:
/// `$message = '', ?\Throwable $previous = null, int $code = 0, array
/// $headers = []`.
#[derive(Debug, Clone, Default)]
pub struct HttpArgs {
    pub message: PhpString,
    pub previous: Option<Throwable>,
    pub code: i64,
    pub headers: PhpArray,
}

impl HttpArgs {
    /// Only a message.
    pub fn message(message: impl Into<PhpString>) -> Self {
        Self {
            message: message.into(),
            ..Self::default()
        }
    }
}

/// `Symfony\Component\HttpKernel\Exception\HttpException` (an
/// `HttpExceptionInterface`, a `\RuntimeException`) or one of its
/// subclasses.
#[derive(Debug, Clone)]
pub struct HttpException {
    class: HttpClass,
    status_code: i64,
    message: PhpString,
    previous: Option<Throwable>,
    headers: PhpArray,
    code: i64,
}

impl HttpException {
    /// `new HttpException($statusCode, $message, $previous, $headers,
    /// $code)`.
    pub fn new(status_code: i64, args: HttpArgs) -> Self {
        Self::of(HttpClass::Http, status_code, args)
    }

    fn of(class: HttpClass, status_code: i64, args: HttpArgs) -> Self {
        Self {
            class,
            status_code,
            message: args.message,
            previous: args.previous,
            headers: args.headers,
            code: args.code,
        }
    }

    /// `HttpException::fromStatusCode($statusCode, ...)`: the subclass for
    /// the status, else an `HttpException`.
    pub fn from_status_code(status_code: i64, args: HttpArgs) -> Self {
        match status_code {
            400 => Self::bad_request(args),
            403 => Self::access_denied(args),
            404 => Self::not_found(args),
            406 => Self::not_acceptable(args),
            409 => Self::conflict(args),
            410 => Self::gone(args),
            411 => Self::length_required(args),
            412 => Self::precondition_failed(args),
            423 => Self::locked(args),
            415 => Self::unsupported_media_type(args),
            422 => Self::unprocessable_entity(args),
            428 => Self::precondition_required(args),
            429 => Self::too_many_requests(Value::Null, args),
            503 => Self::service_unavailable(Value::Null, args),
            _ => Self::new(status_code, args),
        }
    }

    /// `new AccessDeniedHttpException(...)` (403).
    pub fn access_denied(args: HttpArgs) -> Self {
        Self::of(HttpClass::AccessDenied, 403, args)
    }

    /// `new BadRequestHttpException(...)` (400).
    pub fn bad_request(args: HttpArgs) -> Self {
        Self::of(HttpClass::BadRequest, 400, args)
    }

    /// `new ConflictHttpException(...)` (409).
    pub fn conflict(args: HttpArgs) -> Self {
        Self::of(HttpClass::Conflict, 409, args)
    }

    /// `new GoneHttpException(...)` (410).
    pub fn gone(args: HttpArgs) -> Self {
        Self::of(HttpClass::Gone, 410, args)
    }

    /// `new LengthRequiredHttpException(...)` (411).
    pub fn length_required(args: HttpArgs) -> Self {
        Self::of(HttpClass::LengthRequired, 411, args)
    }

    /// `new LockedHttpException(...)` (423).
    pub fn locked(args: HttpArgs) -> Self {
        Self::of(HttpClass::Locked, 423, args)
    }

    /// `new MethodNotAllowedHttpException($allow, ...)` (405): `Allow` is
    /// the methods joined by `, ` and upper-cased.
    pub fn method_not_allowed<S: AsRef<[u8]>>(
        allow: impl IntoIterator<Item = S>,
        mut args: HttpArgs,
    ) -> Self {
        let joined: Vec<Vec<u8>> = allow.into_iter().map(|m| m.as_ref().to_vec()).collect();
        let allow = joined.join(&b", "[..]).to_ascii_uppercase();
        args.headers.insert(
            Key::from_bytes(b"Allow"),
            Value::String(PhpString::from(allow)),
        );
        Self::of(HttpClass::MethodNotAllowed, 405, args)
    }

    /// `new NotAcceptableHttpException(...)` (406).
    pub fn not_acceptable(args: HttpArgs) -> Self {
        Self::of(HttpClass::NotAcceptable, 406, args)
    }

    /// `new NotFoundHttpException(...)` (404).
    pub fn not_found(args: HttpArgs) -> Self {
        Self::of(HttpClass::NotFound, 404, args)
    }

    /// `new PreconditionFailedHttpException(...)` (412).
    pub fn precondition_failed(args: HttpArgs) -> Self {
        Self::of(HttpClass::PreconditionFailed, 412, args)
    }

    /// `new PreconditionRequiredHttpException(...)` (428).
    pub fn precondition_required(args: HttpArgs) -> Self {
        Self::of(HttpClass::PreconditionRequired, 428, args)
    }

    /// `new ServiceUnavailableHttpException($retryAfter, ...)` (503): a
    /// truthy `retry_after` (`int|string|null`) is the `Retry-After`
    /// header.
    pub fn service_unavailable(retry_after: Value, mut args: HttpArgs) -> Self {
        if retry_after.is_truthy() {
            args.headers
                .insert(Key::from_bytes(b"Retry-After"), retry_after);
        }
        Self::of(HttpClass::ServiceUnavailable, 503, args)
    }

    /// `new TooManyRequestsHttpException($retryAfter, ...)` (429): a
    /// truthy `retry_after` (`int|string|null`) is the `Retry-After`
    /// header.
    pub fn too_many_requests(retry_after: Value, mut args: HttpArgs) -> Self {
        if retry_after.is_truthy() {
            args.headers
                .insert(Key::from_bytes(b"Retry-After"), retry_after);
        }
        Self::of(HttpClass::TooManyRequests, 429, args)
    }

    /// `new UnauthorizedHttpException($challenge, ...)` (401): the
    /// challenge is the `WWW-Authenticate` header.
    pub fn unauthorized(challenge: impl Into<PhpString>, mut args: HttpArgs) -> Self {
        args.headers.insert(
            Key::from_bytes(b"WWW-Authenticate"),
            Value::String(challenge.into()),
        );
        Self::of(HttpClass::Unauthorized, 401, args)
    }

    /// `new UnprocessableEntityHttpException(...)` (422).
    pub fn unprocessable_entity(args: HttpArgs) -> Self {
        Self::of(HttpClass::UnprocessableEntity, 422, args)
    }

    /// `new UnsupportedMediaTypeHttpException(...)` (415).
    pub fn unsupported_media_type(args: HttpArgs) -> Self {
        Self::of(HttpClass::UnsupportedMediaType, 415, args)
    }

    /// As a shared throwable.
    pub fn throwable(self) -> Throwable {
        Arc::new(self)
    }

    pub fn class(&self) -> HttpClass {
        self.class
    }

    /// `$e instanceof $class`.
    pub fn is_a(&self, class: &str) -> bool {
        self.class.ancestry().contains(&class)
    }

    /// `getStatusCode()`.
    pub fn get_status_code(&self) -> i64 {
        self.status_code
    }

    /// `getHeaders()`.
    pub fn get_headers(&self) -> &PhpArray {
        &self.headers
    }

    /// `setHeaders($headers)`.
    pub fn set_headers(&mut self, headers: PhpArray) {
        self.headers = headers;
    }

    /// `getMessage()`.
    pub fn get_message(&self) -> &PhpString {
        &self.message
    }

    /// `getCode()`.
    pub fn get_code(&self) -> i64 {
        self.code
    }

    /// `getPrevious()`.
    pub fn get_previous(&self) -> Option<&Throwable> {
        self.previous.as_ref()
    }
}

impl fmt::Display for HttpException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl std::error::Error for HttpException {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// `Symfony\Component\HttpKernel\Exception\ControllerDoesNotReturnResponseException`,
/// a `\LogicException`.
#[derive(Debug, Clone)]
pub struct ControllerDoesNotReturnResponseException {
    message: PhpString,
}

impl ControllerDoesNotReturnResponseException {
    pub const CLASS: &'static str =
        "Symfony\\Component\\HttpKernel\\Exception\\ControllerDoesNotReturnResponseException";

    pub fn new(message: impl Into<PhpString>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// `getMessage()`.
    pub fn get_message(&self) -> &PhpString {
        &self.message
    }
}

impl fmt::Display for ControllerDoesNotReturnResponseException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.to_string_lossy())
    }
}

impl std::error::Error for ControllerDoesNotReturnResponseException {}

/// `$e->getMessage()` for a throwable this crate knows, else its
/// `Display` text.
pub fn message_of(e: &(dyn std::error::Error + 'static)) -> PhpString {
    if let Some(e) = e.downcast_ref::<HttpException>() {
        return e.message.clone();
    }
    if let Some(e) = e.downcast_ref::<PhpException>() {
        return e.message.clone();
    }
    if let Some(e) = e.downcast_ref::<ControllerDoesNotReturnResponseException>() {
        return e.message.clone();
    }
    if let Some(e) = e.downcast_ref::<symfony_http_foundation::exception::Error>() {
        return e.message().clone();
    }
    PhpString::from(e.to_string())
}

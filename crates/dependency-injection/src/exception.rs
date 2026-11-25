//! Port of Symfony\Component\DependencyInjection\Exception\*.php.
//!
//! One [`Error`] type with an [`ErrorKind`] for each exception class. The
//! kind gives the class name and the `instanceof` answers of the PHP
//! hierarchy ([`Error::is`], [`Error::instance_of`]). The messages are
//! upstream's, built from the same fields, and are UTF-8 strings: service
//! ids, parameter names and class names are `String`s in this crate.
//!
//! `AutowiringFailedException`'s lazy message (a closure, rendered when
//! the message is first read) is a [`LazyMessage`]. `InvalidParameterTypeException`
//! takes its parameter's description as strings, because the port has no
//! `\ReflectionParameter`.
//!
//! Upstream: symfony/dependency-injection Exception/AutoconfigureFailedException.php Exception/AutowiringFailedException.php Exception/BadMethodCallException.php Exception/EmptyParameterValueException.php Exception/EnvNotFoundException.php Exception/EnvParameterException.php Exception/ExceptionInterface.php Exception/InvalidArgumentException.php Exception/InvalidParameterTypeException.php Exception/LogicException.php Exception/OutOfBoundsException.php Exception/ParameterCircularReferenceException.php Exception/ParameterNotFoundException.php Exception/RuntimeException.php Exception/ServiceCircularReferenceException.php Exception/ServiceNotFoundException.php

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

/// A result with this crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

const NS: &str = "Symfony\\Component\\DependencyInjection\\Exception\\";

/// The class of an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// `InvalidArgumentException` (extends `\InvalidArgumentException`).
    InvalidArgument,
    /// `OutOfBoundsException` (extends `\OutOfBoundsException`).
    OutOfBounds,
    /// `LogicException` (extends `\LogicException`).
    Logic,
    /// `BadMethodCallException` (extends `\BadMethodCallException`).
    BadMethodCall,
    /// `RuntimeException` (extends `\RuntimeException`).
    Runtime,
    /// `ServiceNotFoundException` (extends `InvalidArgumentException`,
    /// implements PSR-11's `NotFoundExceptionInterface`).
    ServiceNotFound,
    /// `ParameterNotFoundException` (extends `InvalidArgumentException`,
    /// implements `NotFoundExceptionInterface`).
    ParameterNotFound,
    /// `EmptyParameterValueException` (extends `InvalidArgumentException`,
    /// implements `NotFoundExceptionInterface`).
    EmptyParameterValue,
    /// `EnvNotFoundException` (extends `InvalidArgumentException`).
    EnvNotFound,
    /// `EnvParameterException` (extends `InvalidArgumentException`).
    EnvParameter,
    /// `InvalidParameterTypeException` (extends `InvalidArgumentException`).
    InvalidParameterType,
    /// `ServiceCircularReferenceException` (extends `RuntimeException`).
    ServiceCircularReference,
    /// `ParameterCircularReferenceException` (extends `RuntimeException`).
    ParameterCircularReference,
    /// `AutowiringFailedException` (extends `RuntimeException`).
    AutowiringFailed,
    /// `AutoconfigureFailedException` (extends `AutowiringFailedException`).
    AutoconfigureFailed,
    /// A class outside this component (a PHP built-in such as
    /// `\ValueError`, or another component's exception), by name.
    Other(String),
}

impl ErrorKind {
    /// The class name, as `$e::class` gives it.
    pub fn class(&self) -> String {
        let short = match self {
            Self::InvalidArgument => "InvalidArgumentException",
            Self::OutOfBounds => "OutOfBoundsException",
            Self::Logic => "LogicException",
            Self::BadMethodCall => "BadMethodCallException",
            Self::Runtime => "RuntimeException",
            Self::ServiceNotFound => "ServiceNotFoundException",
            Self::ParameterNotFound => "ParameterNotFoundException",
            Self::EmptyParameterValue => "EmptyParameterValueException",
            Self::EnvNotFound => "EnvNotFoundException",
            Self::EnvParameter => "EnvParameterException",
            Self::InvalidParameterType => "InvalidParameterTypeException",
            Self::ServiceCircularReference => "ServiceCircularReferenceException",
            Self::ParameterCircularReference => "ParameterCircularReferenceException",
            Self::AutowiringFailed => "AutowiringFailedException",
            Self::AutoconfigureFailed => "AutoconfigureFailedException",
            Self::Other(class) => return class.clone(),
        };
        format!("{NS}{short}")
    }

    /// The class this one extends inside the component, if any.
    fn parent(&self) -> Option<Self> {
        match self {
            Self::ServiceNotFound
            | Self::ParameterNotFound
            | Self::EmptyParameterValue
            | Self::EnvNotFound
            | Self::EnvParameter
            | Self::InvalidParameterType => Some(Self::InvalidArgument),
            Self::ServiceCircularReference
            | Self::ParameterCircularReference
            | Self::AutowiringFailed => Some(Self::Runtime),
            Self::AutoconfigureFailed => Some(Self::AutowiringFailed),
            _ => None,
        }
    }

    /// The SPL class at the root of the component's class, if any.
    fn spl(&self) -> Option<&'static str> {
        let mut kind = self.clone();
        while let Some(parent) = kind.parent() {
            kind = parent;
        }
        match kind {
            Self::InvalidArgument => Some("InvalidArgumentException"),
            Self::OutOfBounds => Some("OutOfBoundsException"),
            Self::Logic => Some("LogicException"),
            Self::BadMethodCall => Some("BadMethodCallException"),
            Self::Runtime => Some("RuntimeException"),
            _ => None,
        }
    }
}

/// A message rendered on first read: `AutowiringFailedException`'s
/// closure message.
#[derive(Clone)]
pub struct LazyMessage(Arc<Mutex<LazyState>>);

enum LazyState {
    Pending(Box<dyn FnOnce() -> String + Send>),
    Done(String),
}

impl LazyMessage {
    pub fn new(callback: impl FnOnce() -> String + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(LazyState::Pending(Box::new(callback)))))
    }

    /// The message, calling the callback the first time.
    pub fn get(&self) -> String {
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let LazyState::Done(text) = &*state {
            return text.clone();
        }
        let LazyState::Pending(callback) =
            std::mem::replace(&mut *state, LazyState::Done(String::new()))
        else {
            return String::new();
        };
        let text = callback();
        *state = LazyState::Done(text.clone());
        text
    }

    /// Whether the callback is still waiting (`getMessageCallback()` is
    /// not null).
    pub fn is_pending(&self) -> bool {
        matches!(
            &*self.0.lock().unwrap_or_else(PoisonError::into_inner),
            LazyState::Pending(_)
        )
    }
}

impl fmt::Debug for LazyMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LazyMessage")
    }
}

#[derive(Debug, Clone)]
enum Message {
    Text(String),
    Lazy(LazyMessage),
}

/// `ParameterNotFoundException`'s fields; the message is rebuilt from them
/// (`updateRepr()`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParameterNotFound {
    pub key: String,
    pub source_id: Option<String>,
    pub source_key: Option<String>,
    pub alternatives: Vec<String>,
    pub non_nested_alternative: Option<String>,
    pub source_extension_name: Option<String>,
    pub extra_message: Option<String>,
}

impl ParameterNotFound {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ..Self::default()
        }
    }

    /// `updateRepr()`.
    fn message(&self) -> String {
        let mut message = if let Some(source_id) = &self.source_id {
            format!(
                "The service \"{source_id}\" has a dependency on a non-existent parameter \"{}\".",
                self.key
            )
        } else if let Some(source_key) = &self.source_key {
            format!(
                "The parameter \"{source_key}\" has a dependency on a non-existent parameter \"{}\".",
                self.key
            )
        } else if let Some(extension) = &self.source_extension_name {
            format!(
                "You have requested a non-existent parameter \"{}\" while loading extension \"{extension}\".",
                self.key
            )
        } else if self.key.starts_with('.') {
            format!(
                "Parameter \"{}\" not found. It was probably deleted during the compilation of the container.",
                self.key
            )
        } else {
            format!(
                "You have requested a non-existent parameter \"{}\".",
                self.key
            )
        };
        if !self.alternatives.is_empty() {
            if self.alternatives.len() == 1 {
                message.push_str(" Did you mean this: \"");
            } else {
                message.push_str(" Did you mean one of these: \"");
            }
            message.push_str(&self.alternatives.join("\", \""));
            message.push_str("\"?");
        } else if let Some(alternative) = &self.non_nested_alternative {
            message.push_str(" You cannot access nested array items, do you want to inject \"");
            message.push_str(alternative);
            message.push_str("\" instead?");
        }
        // `if ($this->extraMessage)`: "" and "0" are falsy.
        if let Some(extra) = &self.extra_message
            && !extra.is_empty()
            && extra != "0"
        {
            message.push(' ');
            message.push_str(extra);
        }
        message
    }
}

#[derive(Debug, Clone)]
enum Data {
    None,
    ServiceNotFound {
        id: String,
        source_id: Option<String>,
        alternatives: Vec<String>,
    },
    ParameterNotFound(Box<ParameterNotFound>),
    ServiceCircularReference {
        service_id: String,
        path: Vec<String>,
    },
    ParameterCircularReference {
        parameters: Vec<String>,
    },
    AutowiringFailed {
        service_id: String,
    },
}

/// A thrown exception of the component.
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: Message,
    code: i64,
    data: Box<Data>,
    previous: Option<Box<Error>>,
    /// A `ParameterNotFoundException`'s message set by [`Error::set_message`],
    /// until a setter rebuilds it.
    message_override: Option<String>,
}

impl Error {
    /// `new $class($message)`.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: Message::Text(message.into()),
            code: 0,
            data: Box::new(Data::None),
            previous: None,
            message_override: None,
        }
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidArgument, message)
    }

    pub fn out_of_bounds(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::OutOfBounds, message)
    }

    pub fn logic(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Logic, message)
    }

    pub fn bad_method_call(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::BadMethodCall, message)
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Runtime, message)
    }

    /// `ServiceNotFoundException($id, $sourceId, $previous, $alternatives,
    /// $msg)`.
    pub fn service_not_found(
        id: impl Into<String>,
        source_id: Option<String>,
        alternatives: Vec<String>,
        msg: Option<String>,
    ) -> Self {
        let id = id.into();
        let mut message = match (msg, &source_id) {
            (Some(msg), _) => msg,
            (None, None) => format!("You have requested a non-existent service \"{id}\"."),
            (None, Some(source)) => format!(
                "The service \"{source}\" has a dependency on a non-existent service \"{id}\"."
            ),
        };
        if !alternatives.is_empty() {
            if alternatives.len() == 1 {
                message.push_str(" Did you mean this: \"");
            } else {
                message.push_str(" Did you mean one of these: \"");
            }
            message.push_str(&alternatives.join("\", \""));
            message.push_str("\"?");
        }
        Self {
            data: Box::new(Data::ServiceNotFound {
                id,
                source_id,
                alternatives,
            }),
            ..Self::new(ErrorKind::ServiceNotFound, message)
        }
    }

    /// `ParameterNotFoundException`.
    pub fn parameter_not_found(fields: ParameterNotFound) -> Self {
        Self {
            data: Box::new(Data::ParameterNotFound(Box::new(fields))),
            ..Self::new(ErrorKind::ParameterNotFound, String::new())
        }
    }

    /// `EmptyParameterValueException($message)`.
    pub fn empty_parameter_value(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::EmptyParameterValue, message)
    }

    /// `EnvNotFoundException($message)`.
    pub fn env_not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::EnvNotFound, message)
    }

    /// `EnvParameterException($envs, $previous, $message)`; `message` is
    /// the `%s` template, upstream's default when `None`.
    pub fn env_parameter(envs: &[String], message: Option<&str>) -> Self {
        let template = message.unwrap_or(
            "Incompatible use of dynamic environment variables \"%s\" found in parameters.",
        );
        // sprintf() with one `%s` and no other directives in every caller.
        let message = template.replacen("%s", &envs.join("\", \""), 1);
        Self::new(ErrorKind::EnvParameter, message)
    }

    /// `InvalidParameterTypeException($serviceId, $type, $parameter)`,
    /// with the parameter described by its declaring function
    /// (`Class::method` or a function name), 0-based position and accepted
    /// type. The code is the passed type, as upstream sets `$this->code`.
    pub fn invalid_parameter_type(
        service_id: &str,
        passed_type: &str,
        function: &str,
        position: usize,
        accepted_type: &str,
    ) -> Self {
        Self::new(
            ErrorKind::InvalidParameterType,
            format!(
                "Invalid definition for service \"{service_id}\": argument {} of \"{function}()\" accepts \"{accepted_type}\", \"{passed_type}\" passed.",
                position + 1
            ),
        )
    }

    /// `ServiceCircularReferenceException($serviceId, $path)`.
    pub fn service_circular_reference(service_id: impl Into<String>, path: Vec<String>) -> Self {
        let service_id = service_id.into();
        let message = format!(
            "Circular reference detected for service \"{service_id}\", path: \"{}\".",
            path.join(" -> ")
        );
        Self {
            data: Box::new(Data::ServiceCircularReference { service_id, path }),
            ..Self::new(ErrorKind::ServiceCircularReference, message)
        }
    }

    /// `ParameterCircularReferenceException($parameters)`.
    pub fn parameter_circular_reference(parameters: Vec<String>) -> Self {
        let first = parameters.first().cloned().unwrap_or_default();
        let message = format!(
            "Circular reference detected for parameter \"{first}\" (\"{}\" > \"{first}\").",
            parameters.join("\" > \"")
        );
        Self {
            data: Box::new(Data::ParameterCircularReference { parameters }),
            ..Self::new(ErrorKind::ParameterCircularReference, message)
        }
    }

    /// `AutowiringFailedException($serviceId, $message)`.
    pub fn autowiring_failed(service_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            data: Box::new(Data::AutowiringFailed {
                service_id: service_id.into(),
            }),
            ..Self::new(ErrorKind::AutowiringFailed, message)
        }
    }

    /// `AutowiringFailedException($serviceId, $closure)`: the message is
    /// rendered when first read.
    pub fn autowiring_failed_lazy(service_id: impl Into<String>, message: LazyMessage) -> Self {
        Self {
            kind: ErrorKind::AutowiringFailed,
            message: Message::Lazy(message),
            code: 0,
            data: Box::new(Data::AutowiringFailed {
                service_id: service_id.into(),
            }),
            previous: None,
            message_override: None,
        }
    }

    /// The same error as an `AutoconfigureFailedException`.
    #[must_use]
    pub fn into_autoconfigure_failed(mut self) -> Self {
        self.kind = ErrorKind::AutoconfigureFailed;
        self
    }

    /// `$previous`.
    #[must_use]
    pub fn with_previous(mut self, previous: Error) -> Self {
        self.previous = Some(Box::new(previous));
        self
    }

    #[must_use]
    pub fn with_code(mut self, code: i64) -> Self {
        self.code = code;
        self
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// `$e::class`.
    pub fn class(&self) -> String {
        self.kind.class()
    }

    /// `getMessage()`.
    pub fn message(&self) -> String {
        if let Data::ParameterNotFound(fields) = &*self.data {
            return self
                .message_override
                .clone()
                .unwrap_or_else(|| fields.message());
        }
        match &self.message {
            Message::Text(text) => text.clone(),
            Message::Lazy(lazy) => lazy.get(),
        }
    }

    pub fn code(&self) -> i64 {
        self.code
    }

    pub fn previous(&self) -> Option<&Error> {
        self.previous.as_deref()
    }

    /// The previous error, to change it in place.
    pub fn previous_mut(&mut self) -> Option<&mut Error> {
        self.previous.as_deref_mut()
    }

    /// `$this->message = $message` set through reflection, as
    /// `Compiler::compile()` does. A `ParameterNotFoundException` keeps
    /// its fields; its next setter rebuilds the message from them, as
    /// upstream's `updateRepr()` does.
    pub fn set_message(&mut self, message: impl Into<String>) {
        let message = message.into();
        if matches!(&*self.data, Data::ParameterNotFound(_)) {
            self.message_override = Some(message);
        } else {
            self.message = Message::Text(message);
        }
    }

    /// Whether the error is of this kind or one that extends it.
    pub fn is(&self, kind: &ErrorKind) -> bool {
        let mut current = Some(self.kind.clone());
        while let Some(k) = current {
            if &k == kind {
                return true;
            }
            current = k.parent();
        }
        false
    }

    /// `$e instanceof $class`, for the component's classes, their SPL
    /// roots, `ExceptionInterface`, PSR-11's interfaces and `\Exception`.
    pub fn instance_of(&self, class: &str) -> bool {
        let class = class.trim_start_matches('\\');
        if let ErrorKind::Other(name) = &self.kind {
            return name == class;
        }
        let mut current = Some(self.kind.clone());
        while let Some(k) = current {
            if k.class() == class {
                return true;
            }
            current = k.parent();
        }
        // The SPL root and its own SPL ancestors.
        let spl: &[&str] = match self.kind.spl() {
            Some("InvalidArgumentException") => &["InvalidArgumentException", "LogicException"],
            Some("BadMethodCallException") => &[
                "BadMethodCallException",
                "BadFunctionCallException",
                "LogicException",
            ],
            Some("LogicException") => &["LogicException"],
            Some("OutOfBoundsException") => &["OutOfBoundsException", "RuntimeException"],
            Some("RuntimeException") => &["RuntimeException"],
            _ => &[],
        };
        if spl.contains(&class) {
            return true;
        }
        match class {
            "Exception"
            | "Throwable"
            | "Symfony\\Component\\DependencyInjection\\Exception\\ExceptionInterface"
            | "Psr\\Container\\ContainerExceptionInterface" => true,
            "Psr\\Container\\NotFoundExceptionInterface" => matches!(
                self.kind,
                ErrorKind::ServiceNotFound
                    | ErrorKind::ParameterNotFound
                    | ErrorKind::EmptyParameterValue
            ),
            _ => false,
        }
    }

    /// `ServiceNotFoundException::getId()`.
    pub fn id(&self) -> Option<&str> {
        match &*self.data {
            Data::ServiceNotFound { id, .. } => Some(id),
            _ => None,
        }
    }

    /// `getSourceId()` of `ServiceNotFoundException` and
    /// `ParameterNotFoundException`.
    pub fn source_id(&self) -> Option<&str> {
        match &*self.data {
            Data::ServiceNotFound { source_id, .. } => source_id.as_deref(),
            Data::ParameterNotFound(fields) => fields.source_id.as_deref(),
            _ => None,
        }
    }

    /// `ServiceNotFoundException::getAlternatives()`.
    pub fn alternatives(&self) -> &[String] {
        match &*self.data {
            Data::ServiceNotFound { alternatives, .. } => alternatives,
            Data::ParameterNotFound(fields) => &fields.alternatives,
            _ => &[],
        }
    }

    /// `ParameterNotFoundException`'s fields.
    pub fn parameter_not_found_fields(&self) -> Option<&ParameterNotFound> {
        match &*self.data {
            Data::ParameterNotFound(fields) => Some(fields),
            _ => None,
        }
    }

    /// `ParameterNotFoundException`'s setters (`setSourceId()`,
    /// `setSourceKey()`, `setSourceExtensionName()`, `setExtraMessage()`),
    /// after which the message is rebuilt. Does nothing for another kind.
    pub fn update_parameter_not_found(&mut self, update: impl FnOnce(&mut ParameterNotFound)) {
        if let Data::ParameterNotFound(fields) = &mut *self.data {
            update(fields);
            self.message_override = None;
        }
    }

    /// `getServiceId()` of `ServiceCircularReferenceException` and
    /// `AutowiringFailedException`.
    pub fn service_id(&self) -> Option<&str> {
        match &*self.data {
            Data::ServiceCircularReference { service_id, .. }
            | Data::AutowiringFailed { service_id } => Some(service_id),
            _ => None,
        }
    }

    /// `ServiceCircularReferenceException::getPath()`.
    pub fn path(&self) -> &[String] {
        match &*self.data {
            Data::ServiceCircularReference { path, .. } => path,
            _ => &[],
        }
    }

    /// `ParameterCircularReferenceException::getParameters()`.
    pub fn parameters(&self) -> &[String] {
        match &*self.data {
            Data::ParameterCircularReference { parameters } => parameters,
            _ => &[],
        }
    }

    /// `AutowiringFailedException::getMessageCallback()` is not null.
    pub fn has_pending_message_callback(&self) -> bool {
        matches!(&self.message, Message::Lazy(lazy) if lazy.is_pending())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class(), self.message())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.previous
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_not_found_messages() {
        let e = Error::service_not_found("foo", None, vec![], None);
        assert_eq!(
            e.message(),
            "You have requested a non-existent service \"foo\"."
        );
        let e = Error::service_not_found(
            "foo",
            Some("bar".into()),
            vec!["fo".into(), "fooo".into()],
            None,
        );
        assert_eq!(
            e.message(),
            "The service \"bar\" has a dependency on a non-existent service \"foo\". Did you mean one of these: \"fo\", \"fooo\"?"
        );
        assert!(e.instance_of("InvalidArgumentException"));
        assert!(e.instance_of("Psr\\Container\\NotFoundExceptionInterface"));
        assert!(!e.instance_of("RuntimeException"));
    }

    #[test]
    fn parameter_not_found_rebuilds_its_message() {
        let mut e = Error::parameter_not_found(ParameterNotFound::new("bar"));
        assert_eq!(
            e.message(),
            "You have requested a non-existent parameter \"bar\"."
        );
        e.update_parameter_not_found(|f| f.source_key = Some("foo".into()));
        assert_eq!(
            e.message(),
            "The parameter \"foo\" has a dependency on a non-existent parameter \"bar\"."
        );
    }

    #[test]
    fn lazy_message_is_rendered_once() {
        let e = Error::autowiring_failed_lazy("s", LazyMessage::new(|| "late".to_string()));
        assert!(e.has_pending_message_callback());
        assert_eq!(e.message(), "late");
        assert!(!e.has_pending_message_callback());
        assert!(
            e.into_autoconfigure_failed()
                .is(&ErrorKind::AutowiringFailed)
        );
    }
}

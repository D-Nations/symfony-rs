//! The file loaders of Symfony\Component\DependencyInjection\Loader:
//! `FileLoader`, `YamlFileLoader` (with `ContentLoaderTrait`),
//! `IniFileLoader`, `DirectoryLoader`, `GlobFileLoader`, `ClosureLoader`
//! and `UndefinedExtensionHandler`.
//!
//! The loaders are symfony-config loaders (`Arc<dyn LoaderInterface>`),
//! so a `LoaderResolver` can hold them beside other components' loaders.
//! Several loaders fill one container: it is a [`SharedContainer`], which
//! no loader keeps locked while it imports another resource.
//!
//! Errors cross symfony-config's `LoaderInterface` as `symfony_config::Error`s.
//! A component exception becomes one of class
//! [`symfony_config::definition::exception::Class::Other`] with its class name,
//! message, code and previous chain ([`to_config_error`]);
//! [`from_config_error`] turns it back into an [`Error`] of the same
//! kind. Data that only a component exception holds (a
//! `ServiceNotFoundException`'s alternatives, say) does not survive the
//! trip; the loaders throw none of those.
//!
//! Not ported: `PhpFileLoader` and `Loader\Configurator\*`, which run PHP
//! code.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use symfony_config::{Class, Error as ConfigError};

use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, ErrorKind};

pub mod closure_loader;
pub mod undefined_extension_handler;

pub use closure_loader::ClosureLoader;
pub use undefined_extension_handler::UndefinedExtensionHandler;

/// The container the loaders of one build fill (PHP passes the same
/// `ContainerBuilder` object to each).
pub type SharedContainer = Arc<Mutex<ContainerBuilder>>;

/// A new [`SharedContainer`].
pub fn shared(container: ContainerBuilder) -> SharedContainer {
    Arc::new(Mutex::new(container))
}

/// Locks a shared container.
pub(crate) fn lock(container: &SharedContainer) -> MutexGuard<'_, ContainerBuilder> {
    container.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The config classes, by name, that a component error may stand for
/// after a round trip.
fn config_class(name: &str) -> Option<Class> {
    [
        Class::Logic,
        Class::LoaderLoad,
        Class::FileLoaderImportCircularReference,
        Class::FileLocatorFileNotFound,
        Class::InvalidArgument,
        Class::XmlParsing,
        Class::InvalidXml,
        Class::Runtime,
        Class::UnexpectedValue,
        Class::TypeError,
        Class::Error,
    ]
    .into_iter()
    .find(|c| c.name() == name)
}

/// A component exception as a `symfony_config::Error`: the same class
/// name, message, code and previous chain.
pub fn to_config_error(error: &Error) -> ConfigError {
    let class_name = error.class();
    let class = match config_class(&class_name) {
        Some(class) => class,
        None => Class::Other {
            exception: !matches!(class_name.as_str(), "TypeError" | "Error" | "ValueError"),
            class: class_name,
        },
    };
    let message = error.message().into_bytes();
    match error.previous() {
        Some(previous) => {
            ConfigError::with_previous(class, message, error.code(), to_config_error(previous))
        }
        None => ConfigError::new(class, message).with_code(error.code()),
    }
}

/// The component's exception kind of a class name.
fn error_kind(class: &str) -> ErrorKind {
    [
        ErrorKind::InvalidArgument,
        ErrorKind::OutOfBounds,
        ErrorKind::Logic,
        ErrorKind::BadMethodCall,
        ErrorKind::Runtime,
        ErrorKind::ServiceNotFound,
        ErrorKind::ParameterNotFound,
        ErrorKind::EmptyParameterValue,
        ErrorKind::EnvNotFound,
        ErrorKind::EnvParameter,
        ErrorKind::InvalidParameterType,
        ErrorKind::ServiceCircularReference,
        ErrorKind::ParameterCircularReference,
        ErrorKind::AutowiringFailed,
        ErrorKind::AutoconfigureFailed,
    ]
    .into_iter()
    .find(|k| k.class() == class)
    .unwrap_or_else(|| ErrorKind::Other(class.to_string()))
}

/// A `symfony_config::Error` as a component [`Error`]: a component class
/// gets its kind back, any other class is [`ErrorKind::Other`].
pub fn from_config_error(error: &ConfigError) -> Error {
    let mut converted = Error::new(
        error_kind(error.class().name()),
        String::from_utf8_lossy(error.get_message()).into_owned(),
    )
    .with_code(error.get_code());
    if let Some(previous) = error.get_previous() {
        converted = converted.with_previous(from_config_error(previous));
    }
    converted
}

/// `.map_err()` from a component error.
#[expect(clippy::needless_pass_by_value, reason = "used as `.map_err(cfg)`")]
pub(crate) fn cfg(error: Error) -> ConfigError {
    to_config_error(&error)
}

/// `.map_err()` from a config error.
#[expect(clippy::needless_pass_by_value, reason = "used as `.map_err(di)`")]
pub(crate) fn di(error: ConfigError) -> Error {
    from_config_error(&error)
}

//! Port of Symfony\Component\Config\Exception\LoaderLoadException.php,
//! with FileLoaderImportCircularReferenceException.php and
//! FileLocatorFileNotFoundException.php: constructors for the
//! [`Error`]s of those classes, with upstream's messages.
//!
//! The classes are [`Class::LoaderLoad`],
//! [`Class::FileLoaderImportCircularReference`] (a LoaderLoadException)
//! and [`Class::FileLocatorFileNotFound`] (an `\InvalidArgumentException`
//! whose `getPaths()` is [`Error::get_paths`]). `DIRECTORY_SEPARATOR` is
//! `/` (Linux, the oracle's system).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Exception/LoaderLoadException.php Exception/FileLoaderImportCircularReferenceException.php Exception/FileLocatorFileNotFoundException.php

use php_value::Value;

use crate::{
    Class, Error,
    php::{self, implode},
};

/// `new LoaderLoadException($resource, $sourceResource, $code, $previous,
/// $type)`.
pub fn loader_load_exception(
    resource: &Value,
    source_resource: Option<&[u8]>,
    code: i64,
    previous: Option<Error>,
    r#type: Option<&[u8]>,
) -> Error {
    let resource: Vec<u8> = match resource {
        Value::String(s) => s.as_bytes().to_vec(),
        // json_encode($resource, \JSON_THROW_ON_ERROR)
        other => php::json_encode(other).unwrap_or_else(|| {
            format!("resource of type \"{}\"", other.debug_type()).into_bytes()
        }),
    };

    let mut message = Vec::new();
    if let Some(previous) = &previous {
        // Include the previous exception, to help the user see what might
        // be the underlying cause; one trailing period is removed.
        let text = previous.get_message();
        message.extend_from_slice(text.strip_suffix(b".").unwrap_or(text));
        message.extend_from_slice(b" in ");
        message.extend_from_slice(&resource);
        message.push(b' ');

        // show tweaked trace to complete the human readable sentence
        match source_resource {
            None => {
                message.extend_from_slice(b"(which is loaded in resource \"");
                message.extend_from_slice(&resource);
            }
            Some(source) => {
                message.extend_from_slice(b"(which is being imported from \"");
                message.extend_from_slice(source);
            }
        }
        message.extend_from_slice(b"\").");
    } else if let Some(source) = source_resource {
        message.extend_from_slice(b"Cannot import resource \"");
        message.extend_from_slice(&resource);
        message.extend_from_slice(b"\" from \"");
        message.extend_from_slice(source);
        message.extend_from_slice(b"\".");
    } else {
        message.extend_from_slice(b"Cannot load resource \"");
        message.extend_from_slice(&resource);
        message.extend_from_slice(b"\".");
    }

    // Is the resource located inside a bundle?
    if resource.first() == Some(&b'@') {
        let first = resource.split(|&c| c == b'/').next().unwrap_or_default();
        message.extend_from_slice(b" Make sure the \"");
        message.extend_from_slice(&first[1..]);
        message.extend_from_slice(
            b"\" bundle is correctly registered and loaded in the application kernel class.",
        );
        message.extend_from_slice(b" If the bundle is registered, make sure the bundle path \"");
        message.extend_from_slice(&resource);
        message.extend_from_slice(b"\" is not empty.");
    } else if let Some(r#type) = r#type {
        message.extend_from_slice(b" Make sure there is a loader supporting the \"");
        message.extend_from_slice(r#type);
        message.extend_from_slice(b"\" type.");
    }

    match previous {
        Some(previous) => Error::with_previous(Class::LoaderLoad, message, code, previous),
        None => Error::new(Class::LoaderLoad, message).with_code(code),
    }
}

/// `new FileLoaderImportCircularReferenceException($resources)`: the keys
/// of FileLoader's loading set, oldest first (never empty there).
pub fn file_loader_import_circular_reference_exception(resources: &[Vec<u8>]) -> Error {
    let first = resources.first().cloned().unwrap_or_default();
    let mut message = b"Circular reference detected in \"".to_vec();
    message.extend_from_slice(&first);
    message.extend_from_slice(b"\" (\"");
    message.extend_from_slice(&implode(b"\" > \"", resources.iter().cloned()));
    message.extend_from_slice(b"\" > \"");
    message.extend_from_slice(&first);
    message.extend_from_slice(b"\").");
    Error::new(Class::FileLoaderImportCircularReference, message)
}

/// `new FileLocatorFileNotFoundException($message, 0, null, $paths)`.
pub fn file_locator_file_not_found_exception(
    message: impl Into<Vec<u8>>,
    paths: Vec<Vec<u8>>,
) -> Error {
    Error::new(Class::FileLocatorFileNotFound, message).with_paths(paths)
}

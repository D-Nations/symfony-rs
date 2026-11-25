//! Port of Symfony\Component\Config\Resource\ResourceInterface.php and
//! SelfCheckingResourceInterface.php, and the resources that watch the
//! filesystem: `FileResource`, `FileExistenceResource`, `DirectoryResource` and
//! `GlobResource`.
//!
//! A resource is a [`ResourceInterface`] trait object. `__toString()` is
//! [`ResourceInterface::to_string`]; `__serialize()` is
//! [`ResourceInterface::serialize`], the array PHP's `serialize()` stores
//! for the object (`ConfigCache`'s meta file writes it).
//! Each resource's `from_serialized()` is what `unserialize()` restores.
//!
//! Paths are bytes and the filesystem is symfony-finder's
//! (`symfony_finder::fs`), `/` being `DIRECTORY_SEPARATOR` as on Linux.
//! A stat that fails is PHP's `false` (its warnings are suppressed with
//! `@` upstream, or not observable).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/ResourceInterface.php Resource/SelfCheckingResourceInterface.php

use std::fmt;

use php_value::{Key, PhpArray, Value};

use crate::{Class, Error};

pub mod directory_resource;
pub mod file_existence_resource;
pub mod file_resource;
pub mod glob_resource;
pub mod self_checking_resource_checker;
pub mod skipping_resource_checker;

pub use directory_resource::DirectoryResource;
pub use file_existence_resource::FileExistenceResource;
pub use file_resource::FileResource;
pub use glob_resource::GlobResource;
pub use self_checking_resource_checker::SelfCheckingResourceChecker;
pub use skipping_resource_checker::SkippingResourceChecker;

/// `ResourceInterface`: a resource whose freshness a cache checks.
pub trait ResourceInterface: Send + Sync + fmt::Debug {
    /// The class name, as `$resource::class` gives it.
    fn class(&self) -> &str;

    /// `__toString()`: a string that uniquely identifies the resource.
    fn to_string(&self) -> Vec<u8>;

    /// `__serialize()`: the properties `serialize()` stores.
    fn serialize(&self) -> Result<PhpArray, Error>;

    /// `instanceof SelfCheckingResourceInterface`.
    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        None
    }
}

/// `SelfCheckingResourceInterface`: a resource that knows whether it is
/// fresh.
pub trait SelfCheckingResourceInterface: ResourceInterface {
    /// `isFresh($timestamp)`: whether the resource has not been updated
    /// since the given timestamp.
    fn is_fresh(&self, timestamp: i64) -> Result<bool, Error>;
}

/// `realpath($resource) ?: (file_exists($resource) ? $resource : false)`.
/// PHP's `realpath('')` is the working directory, and `realpath()` knows
/// no stream wrappers, `file://` included: such a path is kept as given.
pub(crate) fn resolve(path: &[u8]) -> Option<Vec<u8>> {
    use symfony_finder::fs;
    let real = if path.is_empty() {
        fs::realpath(b".")
    } else if fs::scheme(path).is_some() {
        None
    } else {
        fs::realpath(path)
    };
    real.or_else(|| fs::file_exists(path).then(|| path.to_vec()))
}

/// The property `name` of a serialized resource, which must be a string.
pub(crate) fn string_property(data: &PhpArray, class: &str, name: &str) -> Result<Vec<u8>, Error> {
    match data.get(&Key::from(name)) {
        Some(Value::String(s)) => Ok(s.as_bytes().to_vec()),
        Some(other) => Err(Error::new(
            Class::TypeError,
            format!(
                "Cannot assign {} to property {class}::${name} of type string",
                other.debug_type()
            ),
        )),
        None => Err(Error::new(
            Class::Error,
            format!("Typed property {class}::${name} must not be accessed before initialization"),
        )),
    }
}

/// `hash('xxh128', $data)`.
pub(crate) fn xxh128(data: &[u8]) -> Vec<u8> {
    format!("{:032x}", twox_hash::XxHash3_128::oneshot(data)).into_bytes()
}

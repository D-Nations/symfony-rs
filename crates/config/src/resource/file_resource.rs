//! Port of Symfony\Component\Config\Resource\FileResource.php: a file,
//! fresh while its modification time is not after the timestamp.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/FileResource.php

use php_value::{PhpArray, Value, php_array};
use symfony_finder::fs;

use super::{ResourceInterface, SelfCheckingResourceInterface, resolve, string_property};
use crate::{Class, Error};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\FileResource";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResource {
    resource: Vec<u8>,
}

impl FileResource {
    /// `new FileResource($resource)`: the real path, or the path as given
    /// when only a stream wrapper finds it.
    pub fn new(resource: &[u8]) -> Result<Self, Error> {
        let Some(resolved) = resolve(resource) else {
            let mut message = b"The file \"".to_vec();
            message.extend_from_slice(resource);
            message.extend_from_slice(b"\" does not exist.");
            return Err(Error::new(Class::InvalidArgument, message));
        };
        Ok(Self { resource: resolved })
    }

    /// The resource `unserialize()` restores from [`ResourceInterface::serialize`].
    pub fn from_serialized(data: &PhpArray) -> Result<Self, Error> {
        Ok(Self {
            resource: string_property(data, CLASS, "resource")?,
        })
    }

    /// `getResource()`: the canonicalized, absolute path.
    pub fn get_resource(&self) -> &[u8] {
        &self.resource
    }
}

impl ResourceInterface for FileResource {
    fn class(&self) -> &str {
        CLASS
    }

    fn to_string(&self) -> Vec<u8> {
        self.resource.clone()
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        Ok(php_array!["resource" => Value::string(self.resource.clone())])
    }

    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl SelfCheckingResourceInterface for FileResource {
    fn is_fresh(&self, timestamp: i64) -> Result<bool, Error> {
        Ok(fs::stat(&self.resource).is_some_and(|s| s.mtime <= timestamp))
    }
}

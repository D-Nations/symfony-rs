//! Port of Symfony\Component\Config\Resource\FileExistenceResource.php:
//! fresh while a path's existence is what it was when the resource was
//! created.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/FileExistenceResource.php

use php_value::{Key, PhpArray, Value, php_array};
use symfony_finder::fs;

use super::{ResourceInterface, SelfCheckingResourceInterface, string_property};
use crate::{Class, Error};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\FileExistenceResource";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileExistenceResource {
    resource: Vec<u8>,
    exists: bool,
}

impl FileExistenceResource {
    /// `new FileExistenceResource($resource)`.
    pub fn new(resource: &[u8]) -> Self {
        Self {
            resource: resource.to_vec(),
            exists: fs::file_exists(resource),
        }
    }

    /// The resource `unserialize()` restores from [`ResourceInterface::serialize`].
    pub fn from_serialized(data: &PhpArray) -> Result<Self, Error> {
        let exists = match data.get(&Key::from("exists")) {
            Some(Value::Bool(b)) => *b,
            other => {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "Cannot assign {} to property {CLASS}::$exists of type bool",
                        other.map_or_else(|| "null".to_owned(), Value::debug_type)
                    ),
                ));
            }
        };
        Ok(Self {
            resource: string_property(data, CLASS, "resource")?,
            exists,
        })
    }

    /// `getResource()`: the file path to the resource.
    pub fn get_resource(&self) -> &[u8] {
        &self.resource
    }
}

impl ResourceInterface for FileExistenceResource {
    fn class(&self) -> &str {
        CLASS
    }

    fn to_string(&self) -> Vec<u8> {
        let mut s = b"existence.".to_vec();
        s.extend_from_slice(&self.resource);
        s
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        Ok(php_array![
            "resource" => Value::string(self.resource.clone()),
            "exists" => self.exists,
        ])
    }

    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl SelfCheckingResourceInterface for FileExistenceResource {
    fn is_fresh(&self, _timestamp: i64) -> Result<bool, Error> {
        Ok(fs::file_exists(&self.resource) == self.exists)
    }
}

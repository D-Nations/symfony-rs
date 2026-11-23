//! Port of Symfony\Component\Config\Resource\SkippingResourceChecker.php:
//! a checker that claims the resources of the given classes (all of
//! them when none is given) and calls them fresh, so that no later
//! checker looks at them.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/SkippingResourceChecker.php

use php_value::{Key, PhpArray, Value};

use super::ResourceInterface;
use crate::{Error, config_cache::ResourceCheckerInterface};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\SkippingResourceChecker";

/// `new SkippingResourceChecker($skippedResourceTypes)`.
#[derive(Debug, Clone, Default)]
pub struct SkippingResourceChecker {
    /// `array_flip($skippedResourceTypes)`: class name => position.
    skipped_resource_types: PhpArray,
}

impl SkippingResourceChecker {
    pub fn new<S: AsRef<str>>(skipped_resource_types: &[S]) -> Self {
        let mut flipped = PhpArray::new();
        for (i, class) in skipped_resource_types.iter().enumerate() {
            let position = i64::try_from(i).unwrap_or(i64::MAX);
            flipped.insert(
                Key::from_bytes(class.as_ref().as_bytes()),
                Value::Int(position),
            );
        }
        Self {
            skipped_resource_types: flipped,
        }
    }

    /// `__serialize()`.
    pub fn serialize(&self) -> PhpArray {
        let mut data = PhpArray::new();
        data.insert(
            "skippedResourceTypes",
            Value::Array(self.skipped_resource_types.clone()),
        );
        data
    }
}

impl ResourceCheckerInterface for SkippingResourceChecker {
    fn class(&self) -> &str {
        CLASS
    }

    /// `!$this->skippedResourceTypes || isset($this->skippedResourceTypes[$metadata::class])`.
    fn supports(&self, metadata: &dyn ResourceInterface) -> bool {
        self.skipped_resource_types.is_empty()
            || self
                .skipped_resource_types
                .get(&Key::from_bytes(metadata.class().as_bytes()))
                .is_some()
    }

    fn is_fresh(&self, _resource: &dyn ResourceInterface, _timestamp: i64) -> Result<bool, Error> {
        Ok(true)
    }
}

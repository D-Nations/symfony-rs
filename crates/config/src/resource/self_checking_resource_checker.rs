//! Port of Symfony\Component\Config\Resource\SelfCheckingResourceChecker.php:
//! the checker for resources that know whether they are fresh, with the
//! answers shared by every instance (upstream's `private static $cache`,
//! keyed by `"$resource:$timestamp"`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/SelfCheckingResourceChecker.php

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use super::ResourceInterface;
use crate::{Error, config_cache::ResourceCheckerInterface};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\SelfCheckingResourceChecker";

static CACHE: LazyLock<Mutex<HashMap<Vec<u8>, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// `new SelfCheckingResourceChecker()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SelfCheckingResourceChecker;

impl SelfCheckingResourceChecker {
    pub fn new() -> Self {
        Self
    }

    /// Empties the shared answers, as upstream's tests do through
    /// reflection (`$cache` set to `[]`).
    pub fn clear_cache() {
        CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl ResourceCheckerInterface for SelfCheckingResourceChecker {
    fn class(&self) -> &str {
        CLASS
    }

    fn supports(&self, metadata: &dyn ResourceInterface) -> bool {
        metadata.as_self_checking().is_some()
    }

    fn is_fresh(&self, resource: &dyn ResourceInterface, timestamp: i64) -> Result<bool, Error> {
        let mut key = resource.to_string();
        key.push(b':');
        key.extend_from_slice(timestamp.to_string().as_bytes());
        if let Some(&fresh) = CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
        {
            return Ok(fresh);
        }
        // a resource that is not self-checking: PHP calls a method it
        // does not have
        let Some(checking) = resource.as_self_checking() else {
            return Err(Error::new(
                crate::Class::Error,
                format!("Call to undefined method {}::isFresh()", resource.class()),
            ));
        };
        let fresh = checking.is_fresh(timestamp)?;
        CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, fresh);
        Ok(fresh)
    }
}

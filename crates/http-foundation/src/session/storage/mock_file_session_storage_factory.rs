//! Port of Symfony\Component\HttpFoundation\Session\Storage\MockFileSessionStorageFactory.php:
//! a new `MockFileSessionStorage` for each request, sharing the factory's
//! metadata bag.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/MockFileSessionStorageFactory.php

use std::sync::Arc;

use super::{
    metadata_bag::MetadataBag, mock_file_session_storage::MockFileSessionStorage,
    session_storage_factory_interface::SessionStorageFactoryInterface,
    session_storage_interface::SessionStorageInterface,
};
use crate::{exception::Result, request_stack::RequestHandle};

/// `new MockFileSessionStorageFactory($savePath, $name, $metaBag)`.
#[derive(Debug, Clone)]
pub struct MockFileSessionStorageFactory {
    save_path: Option<Vec<u8>>,
    name: String,
    meta_bag: Option<Arc<MetadataBag>>,
}

impl Default for MockFileSessionStorageFactory {
    fn default() -> Self {
        Self::new(None, "MOCKSESSID", None)
    }
}

impl MockFileSessionStorageFactory {
    pub fn new(
        save_path: Option<Vec<u8>>,
        name: impl Into<String>,
        meta_bag: Option<Arc<MetadataBag>>,
    ) -> Self {
        Self {
            save_path,
            name: name.into(),
            meta_bag,
        }
    }
}

impl SessionStorageFactoryInterface for MockFileSessionStorageFactory {
    fn create_storage(
        &self,
        _request: Option<&RequestHandle>,
    ) -> Result<Arc<dyn SessionStorageInterface>> {
        Ok(Arc::new(MockFileSessionStorage::new(
            self.save_path.clone(),
            self.name.clone(),
            self.meta_bag.clone(),
        )?))
    }
}

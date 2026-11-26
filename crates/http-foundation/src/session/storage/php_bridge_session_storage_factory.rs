//! Port of Symfony\Component\HttpFoundation\Session\Storage\PhpBridgeSessionStorageFactory.php:
//! a `PhpBridgeSessionStorage` per request, with `cookie_secure` on for a
//! secure request when the factory is `secure`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/PhpBridgeSessionStorageFactory.php

use std::sync::Arc;

use super::{
    metadata_bag::MetadataBag,
    native_session_storage::SaveHandler,
    native_session_storage_factory::{cookie_secure, secure_request},
    php_bridge_session_storage,
    session_storage_factory_interface::SessionStorageFactoryInterface,
    session_storage_interface::SessionStorageInterface,
};
use crate::{exception::Result, request_stack::RequestHandle};

/// `new PhpBridgeSessionStorageFactory($handler, $metaBag, $secure)`.
#[derive(Debug, Default)]
pub struct PhpBridgeSessionStorageFactory {
    handler: Option<SaveHandler>,
    meta_bag: Option<Arc<MetadataBag>>,
    secure: bool,
}

impl PhpBridgeSessionStorageFactory {
    pub fn new(
        handler: Option<SaveHandler>,
        meta_bag: Option<Arc<MetadataBag>>,
        secure: bool,
    ) -> Self {
        Self {
            handler,
            meta_bag,
            secure,
        }
    }
}

impl SessionStorageFactoryInterface for PhpBridgeSessionStorageFactory {
    fn create_storage(
        &self,
        request: Option<&RequestHandle>,
    ) -> Result<Arc<dyn SessionStorageInterface>> {
        let storage = php_bridge_session_storage::new(self.handler.clone(), self.meta_bag.clone())?;
        if secure_request(self.secure, request)? {
            storage.set_options(&cookie_secure());
        }
        Ok(Arc::new(storage))
    }
}

//! Port of Symfony\Component\HttpFoundation\Session\Storage\NativeSessionStorageFactory.php:
//! a `NativeSessionStorage` per request, with `cookie_secure` on for a
//! secure request when the factory is `secure`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/NativeSessionStorageFactory.php

use std::sync::Arc;

use php_value::{PhpArray, Value};

use super::{
    metadata_bag::MetadataBag,
    native_session_storage::{NativeSessionStorage, SaveHandler},
    session_storage_factory_interface::SessionStorageFactoryInterface,
    session_storage_interface::SessionStorageInterface,
};
use crate::{exception::Result, request_stack::RequestHandle};

/// `new NativeSessionStorageFactory($options, $handler, $metaBag, $secure)`.
#[derive(Debug, Default)]
pub struct NativeSessionStorageFactory {
    options: PhpArray,
    handler: Option<SaveHandler>,
    meta_bag: Option<Arc<MetadataBag>>,
    secure: bool,
}

impl NativeSessionStorageFactory {
    pub fn new(
        options: PhpArray,
        handler: Option<SaveHandler>,
        meta_bag: Option<Arc<MetadataBag>>,
        secure: bool,
    ) -> Self {
        Self {
            options,
            handler,
            meta_bag,
            secure,
        }
    }
}

/// `$this->secure && $request?->isSecure()`.
pub(crate) fn secure_request(secure: bool, request: Option<&RequestHandle>) -> Result<bool> {
    if !secure {
        return Ok(false);
    }
    match request {
        Some(request) => request.with_mut(crate::request::Request::is_secure),
        None => Ok(false),
    }
}

/// `['cookie_secure' => true]`.
pub(crate) fn cookie_secure() -> PhpArray {
    let mut options = PhpArray::new();
    options.insert("cookie_secure", Value::Bool(true));
    options
}

impl SessionStorageFactoryInterface for NativeSessionStorageFactory {
    fn create_storage(
        &self,
        request: Option<&RequestHandle>,
    ) -> Result<Arc<dyn SessionStorageInterface>> {
        let storage =
            NativeSessionStorage::new(&self.options, self.handler.clone(), self.meta_bag.clone())?;
        if secure_request(self.secure, request)? {
            storage.set_options(&cookie_secure());
        }
        Ok(Arc::new(storage))
    }
}

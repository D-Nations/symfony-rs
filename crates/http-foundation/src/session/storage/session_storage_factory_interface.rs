//! Port of Symfony\Component\HttpFoundation\Session\Storage\SessionStorageFactoryInterface.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/SessionStorageFactoryInterface.php

use std::{fmt, sync::Arc};

use super::session_storage_interface::SessionStorageInterface;
use crate::{exception::Result, request_stack::RequestHandle};

/// `SessionStorageFactoryInterface`.
pub trait SessionStorageFactoryInterface: fmt::Debug + Send + Sync {
    /// `createStorage($request)`.
    fn create_storage(
        &self,
        request: Option<&RequestHandle>,
    ) -> Result<Arc<dyn SessionStorageInterface>>;
}

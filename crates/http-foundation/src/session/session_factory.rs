//! Port of Symfony\Component\HttpFoundation\Session\SessionFactory.php: a
//! `Session` on a storage made for the main request.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionFactory.php

use std::{fmt, sync::Arc};

use super::{
    Session, UsageReporter, session_factory_interface::SessionFactoryInterface,
    session_interface::SessionInterface,
    storage::session_storage_factory_interface::SessionStorageFactoryInterface,
};
use crate::{exception::Result, request_stack::RequestStack};

/// `new SessionFactory($requestStack, $storageFactory, $usageReporter)`.
pub struct SessionFactory {
    request_stack: RequestStack,
    storage_factory: Arc<dyn SessionStorageFactoryInterface>,
    usage_reporter: Option<UsageReporter>,
}

impl fmt::Debug for SessionFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionFactory")
            .field("storage_factory", &self.storage_factory)
            .finish_non_exhaustive()
    }
}

impl SessionFactory {
    pub fn new(
        request_stack: RequestStack,
        storage_factory: Arc<dyn SessionStorageFactoryInterface>,
        usage_reporter: Option<UsageReporter>,
    ) -> Self {
        Self {
            request_stack,
            storage_factory,
            usage_reporter,
        }
    }
}

impl SessionFactoryInterface for SessionFactory {
    fn create_session(&self) -> Result<Arc<dyn SessionInterface>> {
        let storage = self
            .storage_factory
            .create_storage(self.request_stack.get_main_request().as_ref())?;
        Ok(Arc::new(Session::new(
            storage,
            None,
            None,
            self.usage_reporter.clone(),
        )?))
    }
}

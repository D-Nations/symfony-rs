//! Port of Symfony\Component\HttpFoundation\Session\Storage\Proxy\SessionHandlerProxy.php:
//! a save handler given to PHP through the proxy, which passes each call
//! on.
//!
//! The save handler's name is `session.save_handler`'s (when the proxy is
//! made) for PHP's own `\SessionHandler`, alone or in a
//! `StrictSessionHandler`; `user` otherwise. `validateId()` is `true` for
//! a handler without it, and `updateTimestamp()` a `write()`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Proxy/SessionHandlerProxy.php

use std::sync::Arc;

use php_value::PhpString;

use super::abstract_proxy::AbstractProxy;
use crate::{
    exception::Result,
    php_session::ini_get,
    session::storage::handler::{
        HandlerResult, SessionHandlerInterface, SessionIdInterface,
        SessionUpdateTimestampHandlerInterface,
    },
};

/// `SessionHandlerProxy::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Proxy\\SessionHandlerProxy";

/// `new SessionHandlerProxy($handler)`.
#[derive(Debug)]
pub struct SessionHandlerProxy {
    proxy: AbstractProxy,
    handler: Arc<dyn SessionHandlerInterface>,
}

impl SessionHandlerProxy {
    pub fn new(handler: Arc<dyn SessionHandlerInterface>) -> Self {
        let wrapper = handler.is_native();
        let name = if wrapper || handler.is_strict_wrapper() {
            ini_get("save_handler").map(|n| n.to_string_lossy().into_owned())
        } else {
            Some("user".to_owned())
        };
        Self {
            proxy: AbstractProxy::new(CLASS, wrapper, name),
            handler,
        }
    }

    /// `getHandler()`.
    pub fn get_handler(&self) -> Arc<dyn SessionHandlerInterface> {
        Arc::clone(&self.handler)
    }

    /// The `AbstractProxy` methods.
    pub fn proxy(&self) -> &AbstractProxy {
        &self.proxy
    }

    /// `getSaveHandlerName()`.
    pub fn get_save_handler_name(&self) -> Option<String> {
        self.proxy.get_save_handler_name()
    }

    /// `isSessionHandlerInterface()`.
    pub fn is_session_handler_interface(&self) -> bool {
        true
    }

    /// `isWrapper()`.
    pub fn is_wrapper(&self) -> bool {
        self.proxy.is_wrapper()
    }

    /// `isActive()`.
    pub fn is_active(&self) -> bool {
        self.proxy.is_active()
    }

    /// `getId()`.
    pub fn get_id(&self) -> String {
        self.proxy.get_id()
    }

    /// `setId($id)`.
    pub fn set_id(&self, id: &str) -> Result<()> {
        self.proxy.set_id(id)
    }

    /// `getName()`.
    pub fn get_name(&self) -> String {
        self.proxy.get_name()
    }

    /// `setName($name)`.
    pub fn set_name(&self, name: &str) -> Result<()> {
        self.proxy.set_name(name)
    }
}

impl SessionHandlerInterface for SessionHandlerProxy {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        self.handler.open(save_path, session_name)
    }

    fn close(&self) -> HandlerResult<bool> {
        self.handler.close()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.handler.read(session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.handler.write(session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.handler.destroy(session_id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.handler.gc(max_lifetime)
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for SessionHandlerProxy {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        match self.handler.as_update_timestamp() {
            Some(handler) => handler.validate_id(session_id),
            None => Ok(true),
        }
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        match self.handler.as_update_timestamp() {
            Some(handler) => handler.update_timestamp(session_id, data),
            None => self.write(session_id, data),
        }
    }
}

impl SessionIdInterface for SessionHandlerProxy {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        crate::session::storage::handler::create_sid()
    }
}

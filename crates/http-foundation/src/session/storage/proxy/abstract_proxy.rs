//! Port of Symfony\Component\HttpFoundation\Session\Storage\Proxy\AbstractProxy.php:
//! the save handler as `NativeSessionStorage` sees it, with the session's
//! status, id and name from PHP's session functions.
//!
//! PHP's subclasses set the protected `$wrapper` and `$saveHandlerName`;
//! here they are given to [`AbstractProxy::new`], with the subclass's
//! class name.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Proxy/AbstractProxy.php

use crate::{
    exception::{Class, Error, Result},
    php_session::{
        Status, session_id, session_name, session_status, set_session_id, set_session_name,
    },
};

/// `AbstractProxy::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Proxy\\AbstractProxy";

/// A subclass of `AbstractProxy` that is no `\SessionHandlerInterface`.
#[derive(Debug, Clone)]
pub struct AbstractProxy {
    class: String,
    wrapper: bool,
    save_handler_name: Option<String>,
}

impl Default for AbstractProxy {
    fn default() -> Self {
        Self::new(CLASS, false, None)
    }
}

impl AbstractProxy {
    /// A subclass `class` with its `$wrapper` and `$saveHandlerName`.
    pub fn new(class: impl Into<String>, wrapper: bool, save_handler_name: Option<String>) -> Self {
        Self {
            class: class.into(),
            wrapper,
            save_handler_name,
        }
    }

    /// `static::class`.
    pub fn class(&self) -> &str {
        &self.class
    }

    /// `getSaveHandlerName()`.
    pub fn get_save_handler_name(&self) -> Option<String> {
        self.save_handler_name.clone()
    }

    /// `isSessionHandlerInterface()`: never, for this proxy.
    pub fn is_session_handler_interface(&self) -> bool {
        false
    }

    /// `isWrapper()`.
    pub fn is_wrapper(&self) -> bool {
        self.wrapper
    }

    /// `isActive()`.
    pub fn is_active(&self) -> bool {
        session_status() == Status::Active
    }

    /// `getId()`.
    pub fn get_id(&self) -> String {
        session_id().to_string_lossy().into_owned()
    }

    /// `setId($id)`.
    pub fn set_id(&self, id: &str) -> Result<()> {
        if self.is_active() {
            return Err(Error::new(
                Class::PhpLogic,
                "Cannot change the ID of an active session.",
            ));
        }
        set_session_id(id);
        Ok(())
    }

    /// `getName()`.
    pub fn get_name(&self) -> String {
        session_name().to_string_lossy().into_owned()
    }

    /// `setName($name)`.
    pub fn set_name(&self, name: &str) -> Result<()> {
        if self.is_active() {
            return Err(Error::new(
                Class::PhpLogic,
                "Cannot change the name of an active session.",
            ));
        }
        set_session_name(name);
        Ok(())
    }
}

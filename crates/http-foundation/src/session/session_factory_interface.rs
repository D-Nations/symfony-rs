//! Port of Symfony\Component\HttpFoundation\Session\SessionFactoryInterface.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionFactoryInterface.php

use std::{fmt, sync::Arc};

use super::session_interface::SessionInterface;
use crate::exception::Result;

/// `SessionFactoryInterface`.
pub trait SessionFactoryInterface: fmt::Debug + Send + Sync {
    /// `createSession()`.
    fn create_session(&self) -> Result<Arc<dyn SessionInterface>>;
}

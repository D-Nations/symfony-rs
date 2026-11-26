//! Port of Symfony\Component\HttpFoundation\Session\FlashBagAwareSessionInterface.php:
//! a session with a flash bag.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/FlashBagAwareSessionInterface.php

use std::sync::Arc;

use super::{flash::flash_bag_interface::FlashBagInterface, session_interface::SessionInterface};
use crate::exception::Result;

/// `FlashBagAwareSessionInterface`.
pub trait FlashBagAwareSessionInterface: SessionInterface {
    /// `getFlashBag()`.
    fn get_flash_bag(&self) -> Result<Arc<dyn FlashBagInterface>>;
}

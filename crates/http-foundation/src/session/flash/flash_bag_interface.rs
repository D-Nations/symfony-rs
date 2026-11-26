//! Port of Symfony\Component\HttpFoundation\Session\Flash\FlashBagInterface.php:
//! messages kept for the next request.
//!
//! Methods declared to return `array` fail with PHP's `\TypeError` when the
//! stored messages are something else (`setAll()` takes any array).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Flash/FlashBagInterface.php

use php_value::{PhpArray, Value};

use crate::{exception::Result, session::session_bag_interface::SessionBagInterface};

/// `FlashBagInterface`.
pub trait FlashBagInterface: SessionBagInterface {
    /// `add($type, $message)`.
    fn add(&self, kind: &str, message: Value) -> Result<()>;

    /// `set($type, $messages)`: a string is a list of one.
    fn set(&self, kind: &str, messages: Value) -> Result<()>;

    /// `peek($type, $default)`.
    fn peek(&self, kind: &str, default: PhpArray) -> Result<PhpArray>;

    /// `peekAll()`.
    fn peek_all(&self) -> Result<PhpArray>;

    /// `get($type, $default)`: the messages, removed.
    fn get(&self, kind: &str, default: PhpArray) -> Result<PhpArray>;

    /// `all()`: every message, removed.
    fn all(&self) -> Result<PhpArray>;

    /// `setAll($messages)`.
    fn set_all(&self, messages: PhpArray) -> Result<()>;

    /// `has($type)`.
    fn has(&self, kind: &str) -> Result<bool>;

    /// `keys()`.
    fn keys(&self) -> Result<Vec<Value>>;
}

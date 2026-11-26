//! Port of Symfony\Component\HttpFoundation\Session\Storage\PhpBridgeSessionStorage.php:
//! the bags on a session PHP started itself (`session_start()`), sharing
//! its save handler. `start()` only loads the bags, and `clear()` empties
//! them and nothing else in `$_SESSION`. No options are set and no
//! shutdown is registered.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/PhpBridgeSessionStorage.php

use std::sync::Arc;

use super::{
    metadata_bag::MetadataBag,
    native_session_storage::{NativeSessionStorage, SaveHandler},
};
use crate::exception::Result;

/// `PhpBridgeSessionStorage::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\PhpBridgeSessionStorage";

/// `new PhpBridgeSessionStorage($handler, $metaBag)`: a
/// `NativeSessionStorage` with this class's `start()` and `clear()`.
pub fn new(
    handler: Option<SaveHandler>,
    meta_bag: Option<Arc<MetadataBag>>,
) -> Result<NativeSessionStorage> {
    NativeSessionStorage::bridge(handler, meta_bag)
}

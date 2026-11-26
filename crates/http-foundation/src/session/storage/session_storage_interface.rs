//! Port of Symfony\Component\HttpFoundation\Session\Storage\SessionStorageInterface.php:
//! where a session's data lives between requests.
//!
//! Storages are shared (`Arc`) and change through `&self`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/SessionStorageInterface.php

use std::{fmt, sync::Arc};

use super::metadata_bag::MetadataBag;
use crate::{exception::Result, session::session_bag_interface::SessionBagInterface};

/// `SessionStorageInterface`.
pub trait SessionStorageInterface: fmt::Debug + Send + Sync {
    /// `start()`.
    fn start(&self) -> Result<bool>;

    /// `isStarted()`.
    fn is_started(&self) -> bool;

    /// `getId()`.
    fn get_id(&self) -> String;

    /// `setId($id)`.
    fn set_id(&self, id: &str) -> Result<()>;

    /// `getName()`.
    fn get_name(&self) -> String;

    /// `setName($name)`.
    fn set_name(&self, name: &str) -> Result<()>;

    /// `regenerate($destroy, $lifetime)`: a new id for the same data.
    fn regenerate(&self, destroy: bool, lifetime: Option<i64>) -> Result<bool>;

    /// `save()`.
    fn save(&self) -> Result<()>;

    /// `clear()`: every bag emptied.
    fn clear(&self) -> Result<()>;

    /// `getBag($name)`.
    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>>;

    /// `registerBag($bag)`.
    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>) -> Result<()>;

    /// `getMetadataBag()`.
    fn get_metadata_bag(&self) -> Arc<MetadataBag>;
}

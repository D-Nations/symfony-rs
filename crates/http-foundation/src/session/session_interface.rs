//! Port of Symfony\Component\HttpFoundation\Session\SessionInterface.php:
//! what a session offers its users.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionInterface.php

use std::{fmt, sync::Arc};

use php_value::{PhpArray, Value};

use super::{session_bag_interface::SessionBagInterface, storage::metadata_bag::MetadataBag};
use crate::exception::Result;

/// `SessionInterface`.
pub trait SessionInterface: fmt::Debug + Send + Sync {
    /// `start()`.
    fn start(&self) -> Result<bool>;
    /// `getId()`.
    fn get_id(&self) -> String;
    /// `setId($id)`.
    fn set_id(&self, id: &str) -> Result<()>;
    /// `getName()`.
    fn get_name(&self) -> String;
    /// `setName($name)`.
    fn set_name(&self, name: &str);
    /// `invalidate($lifetime)`: the data cleared, under a new id.
    fn invalidate(&self, lifetime: Option<i64>) -> Result<bool>;
    /// `migrate($destroy, $lifetime)`: the data kept, under a new id.
    fn migrate(&self, destroy: bool, lifetime: Option<i64>) -> Result<bool>;
    /// `save()`.
    fn save(&self) -> Result<()>;
    /// `has($name)`.
    fn has(&self, name: &str) -> Result<bool>;
    /// `get($name, $default)`.
    fn get(&self, name: &str, default: Value) -> Result<Value>;
    /// `set($name, $value)`.
    fn set(&self, name: &str, value: Value) -> Result<()>;
    /// `all()`.
    fn all(&self) -> Result<PhpArray>;
    /// `replace($attributes)`.
    fn replace(&self, attributes: &PhpArray) -> Result<()>;
    /// `remove($name)`.
    fn remove(&self, name: &str) -> Result<Value>;
    /// `clear()`.
    fn clear(&self) -> Result<()>;
    /// `isStarted()`.
    fn is_started(&self) -> bool;
    /// `registerBag($bag)`.
    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>);
    /// `getBag($name)`.
    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>>;
    /// `getMetadataBag()`.
    fn get_metadata_bag(&self) -> Arc<MetadataBag>;
}

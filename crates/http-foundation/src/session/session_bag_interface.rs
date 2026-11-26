//! Port of Symfony\Component\HttpFoundation\Session\SessionBagInterface.php:
//! a part of the session data that a storage loads into a bag.
//!
//! `initialize(array &$array)` is given a [`Reference`]: the bag keeps it,
//! so what the bag changes is the storage's data, as PHP's reference
//! makes it. Bags are shared (`Arc`) and change through `&self`.
//!
//! PHP's checks on a bag's type become methods: [`get_bag`] is
//! `method_exists($bag, 'getBag') ? $bag->getBag()`, and the `into_*`
//! methods are the interfaces `Session` requires of its bags.
//!
//! [`get_bag`]: SessionBagInterface::get_bag
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionBagInterface.php

use std::{any::Any, fmt, sync::Arc};

use php_value::Value;

use crate::{
    exception::Result,
    php::Reference,
    session::{
        attribute::attribute_bag_interface::AttributeBagInterface,
        flash::flash_bag_interface::FlashBagInterface,
    },
};

/// `SessionBagInterface`.
pub trait SessionBagInterface: Any + fmt::Debug + Send + Sync {
    /// `getName()`.
    fn get_name(&self) -> String;

    /// `initialize(array &$array)`: the bag's data is now `array`'s.
    /// Fails with `\TypeError` when it holds something else than an array.
    fn initialize(&self, array: &Reference) -> Result<()>;

    /// `getStorageKey()`.
    fn get_storage_key(&self) -> String;

    /// `clear()`: empties the bag, returning what it held.
    fn clear(&self) -> Result<Value>;

    /// `$bag->getBag()` for a bag that has the method (a
    /// `SessionBagProxy`); `None` for any other.
    fn get_bag(&self) -> Option<Arc<dyn SessionBagInterface>> {
        None
    }

    /// The bag as an `AttributeBagInterface`, if it is one.
    fn into_attribute_bag(self: Arc<Self>) -> Option<Arc<dyn AttributeBagInterface>> {
        None
    }

    /// The bag as a `FlashBagInterface`, if it is one.
    fn into_flash_bag(self: Arc<Self>) -> Option<Arc<dyn FlashBagInterface>> {
        None
    }

    /// The bag, to downcast to its class.
    fn as_any(&self) -> &dyn Any;
}

/// Whether `a` and `b` are the same bag object (`===`).
pub fn same_bag(a: &Arc<dyn SessionBagInterface>, b: &Arc<dyn SessionBagInterface>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

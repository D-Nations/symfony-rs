//! Port of Symfony\Component\HttpFoundation\Session\SessionBagProxy.php:
//! the wrapper `Session` registers each bag in, which counts each use of
//! the session (`usageIndex`, shared with the session) and reports it,
//! and keeps the bag's data in the session's own list.
//!
//! The usage index is `?int`, `null` when the caller passes an unset
//! variable; incrementing null gives 1, as incrementing 0 does, so it is
//! an integer starting at 0 here.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/SessionBagProxy.php

use std::{
    any::Any,
    fmt,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicI64, Ordering},
    },
};

use php_value::Value;

use super::{Data, UsageReporter, session_bag_interface::SessionBagInterface};
use crate::{exception::Result, php::Reference};

/// `new SessionBagProxy($bag, $data, $usageIndex, $usageReporter)`.
pub struct SessionBagProxy {
    bag: Arc<dyn SessionBagInterface>,
    data: Arc<Mutex<Data>>,
    usage_index: Arc<AtomicI64>,
    usage_reporter: Option<UsageReporter>,
}

impl fmt::Debug for SessionBagProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionBagProxy")
            .field("bag", &self.bag)
            .field("usage_index", &self.usage_index.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl SessionBagProxy {
    pub fn new(
        bag: Arc<dyn SessionBagInterface>,
        data: Arc<Mutex<Data>>,
        usage_index: Arc<AtomicI64>,
        usage_reporter: Option<UsageReporter>,
    ) -> Self {
        Self {
            bag,
            data,
            usage_index,
            usage_reporter,
        }
    }

    fn data(&self) -> std::sync::MutexGuard<'_, Data> {
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `++$this->usageIndex`, then the reporter when it is not negative.
    fn used(&self) {
        let index = self
            .usage_index
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        if let Some(reporter) = &self.usage_reporter
            && index >= 0
        {
            reporter();
        }
    }

    /// `isEmpty()`: whether the bag's data is missing or empty; reading it
    /// counts as a use.
    pub fn is_empty(&self) -> bool {
        let key = self.bag.get_storage_key();
        let value = self.data().get(&key);
        if value.as_ref().is_none_or(Value::is_null) {
            return true;
        }
        self.used();
        !value.is_some_and(|v| v.is_truthy())
    }
}

impl SessionBagInterface for SessionBagProxy {
    fn get_name(&self) -> String {
        self.bag.get_name()
    }

    fn initialize(&self, array: &Reference) -> Result<()> {
        self.used();
        let key = self.bag.get_storage_key();
        self.data().bind(&key, array.clone());
        self.bag.initialize(array)
    }

    fn get_storage_key(&self) -> String {
        self.bag.get_storage_key()
    }

    fn clear(&self) -> Result<Value> {
        self.bag.clear()
    }

    /// `getBag()`: the bag; counts as a use.
    fn get_bag(&self) -> Option<Arc<dyn SessionBagInterface>> {
        self.used();
        Some(Arc::clone(&self.bag))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

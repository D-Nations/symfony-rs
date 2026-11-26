//! Port of Symfony\Component\HttpFoundation\Session\Session.php: the
//! session, its attributes and flash messages kept in bags that a storage
//! loads and saves.
//!
//! PHP shares the session's data by reference: the storage's data, each
//! bag's array and the session's own list (`$this->data`, which the bag
//! proxies fill) are the same values. Here they hold the same
//! [`Reference`]s, in a [`Data`]: keys and references, which copies keep
//! sharing, as copying a PHP array keeps its reference elements.
//!
//! `new Session()` without a storage creates a `NativeSessionStorage`,
//! which is not ported yet: a storage must be given. The usage index is
//! shared with the bag proxies, as PHP's `&getUsageIndex()` gives it out;
//! it saturates where PHP would turn it into a float.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Session.php

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicI64, Ordering},
};

use php_value::{Key, PhpArray, Value};

use crate::{
    exception::{Class, Error, Result},
    php::Reference,
};

pub mod attribute;
pub mod flash;
pub mod flash_bag_aware_session_interface;
pub mod session_bag_interface;
pub mod session_bag_proxy;
pub mod session_factory;
pub mod session_factory_interface;
pub mod session_interface;
pub mod session_utils;
pub mod storage;

use attribute::{attribute_bag::AttributeBag, attribute_bag_interface::AttributeBagInterface};
use flash::{flash_bag::FlashBag, flash_bag_interface::FlashBagInterface};
use flash_bag_aware_session_interface::FlashBagAwareSessionInterface;
use session_bag_interface::SessionBagInterface;
use session_bag_proxy::SessionBagProxy;
use session_interface::SessionInterface;
use storage::{metadata_bag::MetadataBag, session_storage_interface::SessionStorageInterface};

/// `Session::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Session\\Session";

/// The callable told each time the session is used.
pub type UsageReporter = Arc<dyn Fn() + Send + Sync>;

/// Session data: keys and the references that hold their values.
#[derive(Debug, Clone, Default)]
pub struct Data {
    entries: Vec<(Key, Reference)>,
}

impl Data {
    /// An array's values, each in a reference of its own.
    pub fn from_array(array: &PhpArray) -> Self {
        Self {
            entries: array
                .iter()
                .map(|(k, v)| (k.clone(), Reference::new(v.clone())))
                .collect(),
        }
    }

    fn position(&self, key: &Key) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    /// `$data[$key]`.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.position(&Key::from_bytes(key.as_bytes()))
            .map(|i| self.entries[i].1.get())
    }

    /// `unset($data[$key])`.
    pub fn remove(&mut self, key: &str) {
        if let Some(i) = self.position(&Key::from_bytes(key.as_bytes())) {
            self.entries.remove(i);
        }
    }

    /// `array_keys($data)`.
    pub fn keys(&self) -> Vec<Key> {
        self.entries.iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The values now, as an array.
    pub fn to_array(&self) -> PhpArray {
        let mut array = PhpArray::new();
        for (key, reference) in &self.entries {
            array.insert(key.clone(), reference.get());
        }
        array
    }

    /// The values now, in order.
    pub fn values(&self) -> Vec<Value> {
        self.entries.iter().map(|(_, r)| r.get()).collect()
    }

    /// `$data[$key] ??= []`, and the reference that holds it.
    pub fn entry_or_empty_array(&mut self, key: &str) -> Reference {
        let key = Key::from_bytes(key.as_bytes());
        let reference = if let Some(i) = self.position(&key) {
            self.entries[i].1.clone()
        } else {
            let reference = Reference::new(Value::Null);
            self.entries.push((key, reference.clone()));
            reference
        };
        reference.with(|value| {
            if value.is_null() {
                *value = Value::Array(PhpArray::new());
            }
        });
        reference
    }

    /// `$data[$key] = &$reference`.
    pub fn bind(&mut self, key: &str, reference: Reference) {
        let key = Key::from_bytes(key.as_bytes());
        match self.position(&key) {
            Some(i) => self.entries[i].1 = reference,
            None => self.entries.push((key, reference)),
        }
    }
}

/// A bag's array: the referenced value, which `initialize()` checked is
/// an array (a value that is not becomes an empty array).
pub(crate) fn bag_array(value: &mut Value) -> &mut PhpArray {
    if !matches!(value, Value::Array(_)) {
        *value = Value::Array(PhpArray::new());
    }
    if let Value::Array(array) = value {
        return array;
    }
    // Not reached: the value was made an array above.
    *value = Value::Array(PhpArray::new());
    bag_array(value)
}

/// `initialize(array &$name)`'s type check.
pub(crate) fn require_array(method: &str, parameter: &str, reference: &Reference) -> Result<()> {
    let value = reference.get();
    if matches!(value, Value::Array(_)) {
        return Ok(());
    }
    Err(Error::new(
        Class::PhpTypeError,
        format!(
            "{method}(): Argument #1 (${parameter}) must be of type array, {} given",
            value.value_name()
        ),
    ))
}

/// `new Session($storage, $attributes, $flashes, $usageReporter)`.
pub struct Session {
    storage: Arc<dyn SessionStorageInterface>,
    flash_name: String,
    attribute_name: String,
    data: Arc<Mutex<Data>>,
    usage_index: Arc<AtomicI64>,
    usage_reporter: Option<UsageReporter>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("storage", &self.storage)
            .field("usage_index", &self.usage_index.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Session {
    pub fn new(
        storage: Arc<dyn SessionStorageInterface>,
        attributes: Option<Arc<dyn AttributeBagInterface>>,
        flashes: Option<Arc<dyn FlashBagInterface>>,
        usage_reporter: Option<UsageReporter>,
    ) -> Self {
        let attributes = attributes.unwrap_or_else(|| Arc::new(AttributeBag::default()));
        let flashes = flashes.unwrap_or_else(|| Arc::new(FlashBag::default()));
        let session = Self {
            storage,
            flash_name: flashes.get_name(),
            attribute_name: attributes.get_name(),
            data: Arc::default(),
            usage_index: Arc::new(AtomicI64::new(0)),
            usage_reporter,
        };
        session.register_bag(attributes);
        session.register_bag(flashes);
        session
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

    /// `getIterator()`: the attributes.
    pub fn get_iterator(&self) -> Result<PhpArray> {
        Ok(self.get_attribute_bag()?.all())
    }

    /// `count()`.
    pub fn count(&self) -> Result<usize> {
        Ok(self.get_attribute_bag()?.all().len())
    }

    /// `&getUsageIndex()`: the counter itself.
    pub fn get_usage_index(&self) -> Arc<AtomicI64> {
        Arc::clone(&self.usage_index)
    }

    /// `isEmpty()`: whether every bag's data is empty; counts as a use
    /// once the session is started.
    pub fn is_empty(&self) -> bool {
        if self.is_started() {
            self.used();
        }
        let values = self
            .data
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values();
        !values.iter().any(Value::is_truthy)
    }

    fn get_attribute_bag(&self) -> Result<Arc<dyn AttributeBagInterface>> {
        self.get_bag(&self.attribute_name)?
            .into_attribute_bag()
            .ok_or_else(|| {
                Error::new(
                    Class::PhpTypeError,
                    format!("{CLASS}::getAttributeBag(): Return value must be of type Symfony\\Component\\HttpFoundation\\Session\\Attribute\\AttributeBagInterface, a bag of another class returned"),
                )
            })
    }
}

impl SessionInterface for Session {
    fn start(&self) -> Result<bool> {
        self.storage.start()
    }

    fn get_id(&self) -> String {
        self.storage.get_id()
    }

    fn set_id(&self, id: &str) -> Result<()> {
        if self.storage.get_id() != id {
            self.storage.set_id(id)?;
        }
        Ok(())
    }

    fn get_name(&self) -> String {
        self.storage.get_name()
    }

    fn set_name(&self, name: &str) {
        self.storage.set_name(name);
    }

    fn invalidate(&self, lifetime: Option<i64>) -> Result<bool> {
        self.storage.clear()?;
        self.migrate(true, lifetime)
    }

    fn migrate(&self, destroy: bool, lifetime: Option<i64>) -> Result<bool> {
        self.storage.regenerate(destroy, lifetime)
    }

    fn save(&self) -> Result<()> {
        self.storage.save()
    }

    fn has(&self, name: &str) -> Result<bool> {
        Ok(self.get_attribute_bag()?.has(name))
    }

    fn get(&self, name: &str, default: Value) -> Result<Value> {
        Ok(self.get_attribute_bag()?.get(name, default))
    }

    fn set(&self, name: &str, value: Value) -> Result<()> {
        self.get_attribute_bag()?.set(name, value);
        Ok(())
    }

    fn all(&self) -> Result<PhpArray> {
        Ok(self.get_attribute_bag()?.all())
    }

    fn replace(&self, attributes: &PhpArray) -> Result<()> {
        self.get_attribute_bag()?.replace(attributes);
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<Value> {
        Ok(self.get_attribute_bag()?.remove(name))
    }

    fn clear(&self) -> Result<()> {
        self.get_attribute_bag()?.clear().map(|_| ())
    }

    fn is_started(&self) -> bool {
        self.storage.is_started()
    }

    /// `registerBag($bag)`: the bag, in a proxy that shares this
    /// session's data and usage index.
    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>) {
        self.storage.register_bag(Arc::new(SessionBagProxy::new(
            bag,
            Arc::clone(&self.data),
            Arc::clone(&self.usage_index),
            self.usage_reporter.clone(),
        )));
    }

    /// `getBag($name)`: the storage's bag, or the bag a proxy holds.
    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>> {
        let bag = self.storage.get_bag(name)?;
        Ok(bag.get_bag().unwrap_or(bag))
    }

    fn get_metadata_bag(&self) -> Arc<MetadataBag> {
        self.used();
        self.storage.get_metadata_bag()
    }
}

impl FlashBagAwareSessionInterface for Session {
    fn get_flash_bag(&self) -> Result<Arc<dyn FlashBagInterface>> {
        self.get_bag(&self.flash_name)?
            .into_flash_bag()
            .ok_or_else(|| {
                Error::new(
                    Class::PhpTypeError,
                    format!("{CLASS}::getFlashBag(): Return value must be of type Symfony\\Component\\HttpFoundation\\Session\\Flash\\FlashBagInterface, a bag of another class returned"),
                )
            })
    }
}

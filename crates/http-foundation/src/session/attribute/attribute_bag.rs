//! Port of Symfony\Component\HttpFoundation\Session\Attribute\AttributeBag.php:
//! the session's attributes, stored under `_sf2_attributes`.
//!
//! Before `initialize()` the bag has an array of its own; after it, the
//! reference it was given. Names are PHP array keys: `'12'` is the key
//! `12`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Attribute/AttributeBag.php

use std::{
    any::Any,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{Key, PhpArray, Value};

use super::attribute_bag_interface::AttributeBagInterface;
use crate::{
    exception::Result,
    php::Reference,
    session::{bag_array, session_bag_interface::SessionBagInterface},
};

/// `AttributeBag::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Session\\Attribute\\AttributeBag";

#[derive(Debug)]
struct State {
    name: String,
    attributes: Reference,
}

/// `new AttributeBag($storageKey)`.
#[derive(Debug)]
pub struct AttributeBag {
    storage_key: String,
    state: Mutex<State>,
}

impl Default for AttributeBag {
    fn default() -> Self {
        Self::new("_sf2_attributes")
    }
}

impl AttributeBag {
    pub fn new(storage_key: impl Into<String>) -> Self {
        Self {
            storage_key: storage_key.into(),
            state: Mutex::new(State {
                name: "attributes".to_owned(),
                attributes: Reference::new(Value::Array(PhpArray::new())),
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with<R>(&self, f: impl FnOnce(&mut PhpArray) -> R) -> R {
        let attributes = self.state().attributes.clone();
        attributes.with(|value| f(bag_array(value)))
    }

    /// `setName($name)`.
    pub fn set_name(&self, name: impl Into<String>) {
        self.state().name = name.into();
    }

    /// `getIterator()`: the attributes now.
    pub fn get_iterator(&self) -> PhpArray {
        self.all()
    }

    /// `count()`.
    pub fn count(&self) -> usize {
        self.with(|a| a.len())
    }
}

impl SessionBagInterface for AttributeBag {
    fn get_name(&self) -> String {
        self.state().name.clone()
    }

    fn initialize(&self, array: &Reference) -> Result<()> {
        crate::session::require_array(&format!("{CLASS}::initialize"), "attributes", array)?;
        self.state().attributes = array.clone();
        Ok(())
    }

    fn get_storage_key(&self) -> String {
        self.storage_key.clone()
    }

    fn clear(&self) -> Result<Value> {
        Ok(Value::Array(self.with(std::mem::take)))
    }

    fn into_attribute_bag(self: Arc<Self>) -> Option<Arc<dyn AttributeBagInterface>> {
        Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl AttributeBagInterface for AttributeBag {
    fn has(&self, name: &str) -> bool {
        self.with(|a| a.get(&Key::from_bytes(name.as_bytes())).is_some())
    }

    fn get(&self, name: &str, default: Value) -> Value {
        self.with(|a| a.get(&Key::from_bytes(name.as_bytes())).cloned())
            .unwrap_or(default)
    }

    fn set(&self, name: &str, value: Value) {
        self.with(|a| a.insert(Key::from_bytes(name.as_bytes()), value));
    }

    fn all(&self) -> PhpArray {
        self.with(|a| a.clone())
    }

    fn replace(&self, attributes: &PhpArray) {
        self.with(|a| {
            *a = PhpArray::new();
            for (key, value) in attributes.iter() {
                // set(string $name): an integer key is its string
                a.insert(key.clone(), value.clone());
            }
        });
    }

    fn remove(&self, name: &str) -> Value {
        self.with(|a| a.remove(&Key::from_bytes(name.as_bytes())))
            .unwrap_or(Value::Null)
    }
}

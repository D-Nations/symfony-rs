//! Port of Symfony\Component\HttpFoundation\Session\Flash\FlashBag.php:
//! messages by type, kept until read, stored under `_symfony_flashes`.
//!
//! A type "has" messages when its entry is truthy. `peek()` and `get()`
//! return the entry as it is, so an entry `setAll()` made a string fails
//! their `array` return type, as in PHP.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Flash/FlashBag.php

use std::{
    any::Any,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{Key, PhpArray, Value};

use super::{flash_bag_interface::FlashBagInterface, messages};
use crate::{
    exception::Result,
    php::{Reference, array_return, push_onto},
    session::{bag_array, require_array, session_bag_interface::SessionBagInterface},
};

/// `FlashBag::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Session\\Flash\\FlashBag";

#[derive(Debug)]
struct State {
    name: String,
    flashes: Reference,
}

/// `new FlashBag($storageKey)`.
#[derive(Debug)]
pub struct FlashBag {
    storage_key: String,
    state: Mutex<State>,
}

impl Default for FlashBag {
    fn default() -> Self {
        Self::new("_symfony_flashes")
    }
}

impl FlashBag {
    pub fn new(storage_key: impl Into<String>) -> Self {
        Self {
            storage_key: storage_key.into(),
            state: Mutex::new(State {
                name: "flashes".to_owned(),
                flashes: Reference::new(Value::Array(PhpArray::new())),
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with<R>(&self, f: impl FnOnce(&mut PhpArray) -> R) -> R {
        let flashes = self.state().flashes.clone();
        flashes.with(|value| f(bag_array(value)))
    }

    /// `setName($name)`.
    pub fn set_name(&self, name: impl Into<String>) {
        self.state().name = name.into();
    }

    fn has_in(flashes: &PhpArray, key: &Key) -> bool {
        flashes.get(key).is_some_and(Value::is_truthy)
    }
}

impl SessionBagInterface for FlashBag {
    fn get_name(&self) -> String {
        self.state().name.clone()
    }

    fn initialize(&self, array: &Reference) -> Result<()> {
        require_array(&format!("{CLASS}::initialize"), "flashes", array)?;
        self.state().flashes = array.clone();
        Ok(())
    }

    fn get_storage_key(&self) -> String {
        self.storage_key.clone()
    }

    fn clear(&self) -> Result<Value> {
        self.all().map(Value::Array)
    }

    fn into_flash_bag(self: Arc<Self>) -> Option<Arc<dyn FlashBagInterface>> {
        Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl FlashBagInterface for FlashBag {
    fn add(&self, kind: &str, message: Value) -> Result<()> {
        self.with(|f| push_onto(f, &Key::from_bytes(kind.as_bytes()), message))
    }

    fn set(&self, kind: &str, messages: Value) -> Result<()> {
        let list = messages::cast(&format!("{CLASS}::set"), messages)?;
        self.with(|f| f.insert(Key::from_bytes(kind.as_bytes()), Value::Array(list)));
        Ok(())
    }

    fn peek(&self, kind: &str, default: PhpArray) -> Result<PhpArray> {
        let key = Key::from_bytes(kind.as_bytes());
        let found = self.with(|f| {
            Self::has_in(f, &key)
                .then(|| f.get(&key).cloned())
                .flatten()
        });
        match found {
            Some(value) => array_return(&format!("{CLASS}::peek"), value),
            None => Ok(default),
        }
    }

    fn peek_all(&self) -> Result<PhpArray> {
        Ok(self.with(|f| f.clone()))
    }

    fn get(&self, kind: &str, default: PhpArray) -> Result<PhpArray> {
        let key = Key::from_bytes(kind.as_bytes());
        let found = self.with(|f| {
            if Self::has_in(f, &key) {
                f.remove(&key)
            } else {
                None
            }
        });
        match found {
            Some(value) => array_return(&format!("{CLASS}::get"), value),
            None => Ok(default),
        }
    }

    fn all(&self) -> Result<PhpArray> {
        Ok(self.with(std::mem::take))
    }

    fn set_all(&self, messages: PhpArray) -> Result<()> {
        self.with(|f| *f = messages);
        Ok(())
    }

    fn has(&self, kind: &str) -> Result<bool> {
        Ok(self.with(|f| Self::has_in(f, &Key::from_bytes(kind.as_bytes()))))
    }

    fn keys(&self) -> Result<Vec<Value>> {
        Ok(self.with(|f| f.keys().map(Key::to_value).collect()))
    }
}

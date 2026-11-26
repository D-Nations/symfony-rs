//! Port of Symfony\Component\HttpFoundation\Session\Flash\AutoExpireFlashBag.php:
//! flash messages that are shown on the next request whether or not they
//! are read.
//!
//! The data is `['display' => ..., 'new' => ...]`. `initialize()` moves
//! what was added last time (`new`) to `display`; reading looks at
//! `display`, adding writes to `new`. An entry that is not an array where
//! PHP needs one fails as PHP's built-ins do (`array_key_exists()`,
//! `array_keys()`, the `array` return types).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Flash/AutoExpireFlashBag.php

use std::{
    any::Any,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{Key, PhpArray, Value};

use super::{flash_bag_interface::FlashBagInterface, messages};
use crate::{
    exception::{Class, Error, Result},
    php::{Reference, array_return, push_onto},
    session::{bag_array, require_array, session_bag_interface::SessionBagInterface},
};

/// `AutoExpireFlashBag::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Session\\Flash\\AutoExpireFlashBag";

fn display() -> Key {
    Key::from_bytes(b"display")
}

fn new_key() -> Key {
    Key::from_bytes(b"new")
}

#[derive(Debug)]
struct State {
    name: String,
    flashes: Reference,
}

/// `new AutoExpireFlashBag($storageKey)`.
#[derive(Debug)]
pub struct AutoExpireFlashBag {
    storage_key: String,
    state: Mutex<State>,
}

impl Default for AutoExpireFlashBag {
    fn default() -> Self {
        Self::new("_symfony_flashes")
    }
}

/// `$flashes[$key]` as an array a built-in is given: PHP's `\TypeError`
/// for anything else (null when missing).
fn argument<'a>(
    function: &str,
    position: &str,
    flashes: &'a PhpArray,
    key: &Key,
) -> Result<&'a PhpArray> {
    match flashes.get(key) {
        Some(Value::Array(array)) => Ok(array),
        other => Err(Error::new(
            Class::PhpTypeError,
            format!(
                "{function}(): Argument {position} must be of type array, {} given",
                other.map_or_else(|| "null".to_owned(), Value::value_name)
            ),
        )),
    }
}

/// `$flashes['new']` to write into: created when missing, null or
/// `false`, as PHP's nested assignment makes it.
fn new_entry(flashes: &mut PhpArray) -> Result<&mut PhpArray> {
    let key = new_key();
    if !matches!(flashes.get(&key), Some(Value::Array(_))) {
        match flashes.get(&key) {
            None | Some(Value::Null | Value::Bool(false)) => {
                flashes.insert(key.clone(), Value::Array(PhpArray::new()));
            }
            Some(Value::String(_)) => {
                return Err(Error::new(
                    Class::PhpError,
                    "Cannot use string offset as an array",
                ));
            }
            Some(_) => {
                return Err(Error::new(
                    Class::PhpError,
                    "Cannot use a scalar value as an array",
                ));
            }
        }
    }
    match flashes.get_mut(&key) {
        Some(Value::Array(array)) => Ok(array),
        _ => Err(Error::new(
            Class::PhpError,
            "Cannot use a scalar value as an array",
        )),
    }
}

impl AutoExpireFlashBag {
    pub fn new(storage_key: impl Into<String>) -> Self {
        let mut flashes = PhpArray::new();
        flashes.insert(display(), Value::Array(PhpArray::new()));
        flashes.insert(new_key(), Value::Array(PhpArray::new()));
        Self {
            storage_key: storage_key.into(),
            state: Mutex::new(State {
                name: "flashes".to_owned(),
                flashes: Reference::new(Value::Array(flashes)),
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

    fn has_in(flashes: &PhpArray, kind: &Key) -> Result<bool> {
        let display = argument("array_key_exists", "#2 ($array)", flashes, &display())?;
        Ok(display.get(kind).is_some_and(Value::is_truthy))
    }
}

impl SessionBagInterface for AutoExpireFlashBag {
    fn get_name(&self) -> String {
        self.state().name.clone()
    }

    fn initialize(&self, array: &Reference) -> Result<()> {
        require_array(&format!("{CLASS}::initialize"), "flashes", array)?;
        self.state().flashes = array.clone();
        // The logic: messages from the last request will be stored in new, so we move them to previous
        // This request we will show what is in 'display'.  What is placed into 'new' this time round will
        // be moved to display next time round.
        self.with(|f| {
            let shown = f
                .get(&new_key())
                .cloned()
                .unwrap_or(Value::Array(PhpArray::new()));
            f.insert(display(), shown);
            f.insert(new_key(), Value::Array(PhpArray::new()));
        });
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

impl FlashBagInterface for AutoExpireFlashBag {
    fn add(&self, kind: &str, message: Value) -> Result<()> {
        self.with(|f| push_onto(new_entry(f)?, &Key::from_bytes(kind.as_bytes()), message))
    }

    fn set(&self, kind: &str, messages: Value) -> Result<()> {
        let list = messages::cast(&format!("{CLASS}::set"), messages)?;
        self.with(|f| {
            new_entry(f)?.insert(Key::from_bytes(kind.as_bytes()), Value::Array(list));
            Ok(())
        })
    }

    fn peek(&self, kind: &str, default: PhpArray) -> Result<PhpArray> {
        let key = Key::from_bytes(kind.as_bytes());
        let found = self.with(|f| -> Result<Option<Value>> {
            if !Self::has_in(f, &key)? {
                return Ok(None);
            }
            Ok(argument("array_key_exists", "#2 ($array)", f, &display())?
                .get(&key)
                .cloned())
        })?;
        match found {
            Some(value) => array_return(&format!("{CLASS}::peek"), value),
            None => Ok(default),
        }
    }

    fn peek_all(&self) -> Result<PhpArray> {
        let shown = self.with(|f| f.get(&display()).cloned());
        match shown {
            Some(value) => array_return(&format!("{CLASS}::peekAll"), value),
            None => Ok(PhpArray::new()),
        }
    }

    fn get(&self, kind: &str, default: PhpArray) -> Result<PhpArray> {
        let key = Key::from_bytes(kind.as_bytes());
        let found = self.with(|f| -> Result<Option<Value>> {
            if !Self::has_in(f, &key)? {
                return Ok(None);
            }
            // isset($this->flashes['display'][$type]), then unset()
            Ok(match f.get_mut(&display()) {
                Some(Value::Array(shown)) if shown.get(&key).is_some_and(|v| !v.is_null()) => {
                    shown.remove(&key)
                }
                _ => Some(Value::Array(default.clone())),
            })
        })?;
        match found {
            Some(value) => array_return(&format!("{CLASS}::get"), value),
            None => Ok(default),
        }
    }

    fn all(&self) -> Result<PhpArray> {
        let shown = self.with(|f| {
            let shown = f.get(&display()).cloned().unwrap_or(Value::Null);
            f.insert(display(), Value::Array(PhpArray::new()));
            shown
        });
        array_return(&format!("{CLASS}::all"), shown)
    }

    fn set_all(&self, messages: PhpArray) -> Result<()> {
        self.with(|f| f.insert(new_key(), Value::Array(messages)));
        Ok(())
    }

    fn has(&self, kind: &str) -> Result<bool> {
        self.with(|f| Self::has_in(f, &Key::from_bytes(kind.as_bytes())))
    }

    fn keys(&self) -> Result<Vec<Value>> {
        self.with(|f| {
            Ok(argument("array_keys", "#1 ($array)", f, &display())?
                .keys()
                .map(Key::to_value)
                .collect())
        })
    }
}

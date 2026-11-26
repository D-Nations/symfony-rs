//! Port of Symfony\Component\HttpFoundation\Session\Storage\MetadataBag.php:
//! when the session was created and last used, and its cookie's lifetime
//! (`c`, `u` and `l` under `_sf2_meta`).
//!
//! The clock is `php_value::clock::time()`, which tests mock.
//! The lifetime defaults to `(int) ini_get('session.cookie_lifetime')`
//! ([`crate::php_session`]). The getters' `int`
//! return types and the `int $lastUsed` property convert as PHP does in
//! coercive mode, or fail with its `\TypeError` and `\Error`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/MetadataBag.php

use std::{
    any::Any,
    sync::{Mutex, PoisonError},
};

use php_value::{Key, PhpArray, Value, clock::time};

use crate::{
    exception::{Class, Error, Result},
    php::{Reference, int_return},
    session::{bag_array, require_array, session_bag_interface::SessionBagInterface},
};

/// `MetadataBag::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Session\\Storage\\MetadataBag";

/// `MetadataBag::CREATED`.
pub const CREATED: &str = "c";
/// `MetadataBag::UPDATED`.
pub const UPDATED: &str = "u";
/// `MetadataBag::LIFETIME`.
pub const LIFETIME: &str = "l";

fn key(name: &str) -> Key {
    Key::from_bytes(name.as_bytes())
}

#[derive(Debug)]
struct State {
    name: String,
    meta: Reference,
    last_used: Option<i64>,
}

/// `new MetadataBag($storageKey, $updateThreshold, $cookieLifetime)`.
#[derive(Debug)]
pub struct MetadataBag {
    storage_key: String,
    update_threshold: i64,
    cookie_lifetime: Option<i64>,
    state: Mutex<State>,
}

impl Default for MetadataBag {
    fn default() -> Self {
        Self::new("_sf2_meta", 0, None)
    }
}

/// `int - $value` for the stored timestamp.
fn minus(now: i64, value: &Value) -> Result<i64> {
    match value {
        Value::Int(i) => Ok(now.saturating_sub(*i)),
        Value::Null => Ok(now),
        Value::Bool(b) => Ok(now - i64::from(*b)),
        Value::String(s) => php_value::numeric::int_argument(s.as_bytes())
            .map(|i| now.saturating_sub(i))
            .ok_or_else(|| {
                Error::new(
                    Class::PhpTypeError,
                    "Unsupported operand types: int - string",
                )
            }),
        other => Err(Error::new(
            Class::PhpTypeError,
            format!("Unsupported operand types: int - {}", other.debug_type()),
        )),
    }
}

impl MetadataBag {
    pub fn new(
        storage_key: impl Into<String>,
        update_threshold: i64,
        cookie_lifetime: Option<i64>,
    ) -> Self {
        let mut meta = PhpArray::new();
        for name in [CREATED, UPDATED, LIFETIME] {
            meta.insert(key(name), Value::Int(0));
        }
        Self {
            storage_key: storage_key.into(),
            update_threshold,
            cookie_lifetime,
            state: Mutex::new(State {
                name: "__metadata".to_owned(),
                meta: Reference::new(Value::Array(meta)),
                last_used: None,
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn read(&self, name: &str) -> Value {
        let meta = self.state().meta.clone();
        meta.with(|value| {
            bag_array(value)
                .get(&key(name))
                .cloned()
                .unwrap_or(Value::Null)
        })
    }

    /// `getLifetime()`.
    pub fn get_lifetime(&self) -> Result<i64> {
        int_return(&format!("{CLASS}::getLifetime"), &self.read(LIFETIME))
    }

    /// `stampNew($lifetime)`: created and used now, with this lifetime.
    pub fn stamp_new(&self, lifetime: Option<i64>) {
        self.stamp_created(lifetime);
    }

    /// `getCreated()`.
    pub fn get_created(&self) -> Result<i64> {
        int_return(&format!("{CLASS}::getCreated"), &self.read(CREATED))
    }

    /// `getLastUsed()`.
    pub fn get_last_used(&self) -> Result<i64> {
        self.state().last_used.ok_or_else(|| {
            Error::new(
                Class::PhpError,
                format!(
                    "Typed property {CLASS}::$lastUsed must not be accessed before initialization"
                ),
            )
        })
    }

    /// `setName($name)`.
    pub fn set_name(&self, name: impl Into<String>) {
        self.state().name = name.into();
    }

    fn stamp_created(&self, lifetime: Option<i64>) {
        let now = time();
        let lifetime = lifetime
            .or(self.cookie_lifetime)
            .unwrap_or_else(|| crate::php_session::ini_int("cookie_lifetime"));
        let meta = {
            let mut state = self.state();
            state.last_used = Some(now);
            state.meta.clone()
        };
        meta.with(|value| {
            let meta = bag_array(value);
            // `$meta[CREATED] = $meta[UPDATED] = ...` assigns from the
            // right: a new array gets `u` before `c`.
            meta.insert(key(UPDATED), Value::Int(now));
            meta.insert(key(CREATED), Value::Int(now));
            meta.insert(key(LIFETIME), Value::Int(lifetime));
        });
    }
}

impl SessionBagInterface for MetadataBag {
    fn get_name(&self) -> String {
        self.state().name.clone()
    }

    fn initialize(&self, array: &Reference) -> Result<()> {
        require_array(&format!("{CLASS}::initialize"), "array", array)?;
        self.state().meta = array.clone();
        let created = self.read(CREATED);
        if created.is_null() {
            self.stamp_created(None);
            return Ok(());
        }
        let updated = self.read(UPDATED);
        // $this->lastUsed = $this->meta[self::UPDATED] (an int property)
        let last_used = match &updated {
            Value::Null => {
                return Err(Error::new(
                    Class::PhpTypeError,
                    format!("Cannot assign null to property {CLASS}::$lastUsed of type int"),
                ));
            }
            other => int_return("", other).map_err(|_| {
                Error::new(
                    Class::PhpTypeError,
                    format!(
                        "Cannot assign {} to property {CLASS}::$lastUsed of type int",
                        other.debug_type()
                    ),
                )
            })?,
        };
        self.state().last_used = Some(last_used);
        let now = time();
        if minus(now, &updated)? >= self.update_threshold {
            array.with(|value| bag_array(value).insert(key(UPDATED), Value::Int(now)));
        }
        Ok(())
    }

    fn get_storage_key(&self) -> String {
        self.storage_key.clone()
    }

    fn clear(&self) -> Result<Value> {
        // nothing to do
        Ok(Value::Null)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

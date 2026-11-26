//! Port of Symfony\Component\HttpFoundation\Session\Storage\NativeSessionStorage.php:
//! the session in PHP's session extension ([`crate::php_session`]):
//! `$_SESSION`, a save handler given to PHP through a
//! `SessionHandlerProxy`, and the `session.*` settings from its options.
//!
//! The bags are bound to their parts of `$_SESSION` by reference. Saving
//! leaves out empty bags (and a lone metadata bag), writes and closes the
//! session, gives the warnings of `session_write_close()` the save
//! handler's class, and puts the bags' data back. `PhpBridgeSessionStorage`
//! (a session PHP already started) overrides `start()` and `clear()`;
//! here it is this storage with `bridge` set.
//!
//! The file and line of `headers_sent($file, $line)` are not known here:
//! the message names `""` at line 0.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/NativeSessionStorage.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{
    Key, PhpArray, Value,
    compare::loose_equals,
    filter::{FILTER_VALIDATE_BOOLEAN, filter_var},
};

use super::{
    handler::{SessionHandlerInterface, strict_session_handler::StrictSessionHandler},
    metadata_bag::MetadataBag,
    proxy::{abstract_proxy::AbstractProxy, session_handler_proxy::SessionHandlerProxy},
    session_storage_interface::SessionStorageInterface,
};
use crate::{
    exception::{Class, Error, Result},
    php::with_globals,
    php_session::{
        self, SessionHandler, SessionVar, Status, ini_get, ini_set_value, session_create_id,
        session_name, session_status, set_session_id,
    },
    session::{Data, session_bag_interface::SessionBagInterface},
};

/// `NativeSessionStorage::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\NativeSessionStorage";

/// `AbstractProxy|\SessionHandlerInterface`: what the storage is given.
#[derive(Debug, Clone)]
pub enum SaveHandler {
    /// An `AbstractProxy` that is no `\SessionHandlerInterface`.
    Proxy(Arc<AbstractProxy>),
    /// A `SessionHandlerProxy`.
    HandlerProxy(Arc<SessionHandlerProxy>),
    /// A save handler, which the storage puts in a `SessionHandlerProxy`.
    Handler(Arc<dyn SessionHandlerInterface>),
}

/// The save handler as the storage keeps it: always a proxy
/// (`getSaveHandler()`).
#[derive(Debug, Clone)]
pub enum SaveHandlerProxy {
    Proxy(Arc<AbstractProxy>),
    HandlerProxy(Arc<SessionHandlerProxy>),
}

impl SaveHandlerProxy {
    /// The `AbstractProxy` methods.
    pub fn proxy(&self) -> &AbstractProxy {
        match self {
            Self::Proxy(proxy) => proxy,
            Self::HandlerProxy(proxy) => proxy.proxy(),
        }
    }

    /// `getSaveHandlerName()`.
    pub fn get_save_handler_name(&self) -> Option<String> {
        self.proxy().get_save_handler_name()
    }

    /// `isActive()`.
    pub fn is_active(&self) -> bool {
        self.proxy().is_active()
    }

    /// `isWrapper()`.
    pub fn is_wrapper(&self) -> bool {
        self.proxy().is_wrapper()
    }

    /// `isSessionHandlerInterface()`.
    pub fn is_session_handler_interface(&self) -> bool {
        matches!(self, Self::HandlerProxy(_))
    }
}

#[derive(Debug)]
struct State {
    /// `$this->bags`, by name, in order.
    bags: Vec<(String, Arc<dyn SessionBagInterface>)>,
    started: bool,
    closed: bool,
    save_handler: SaveHandlerProxy,
    metadata_bag: Arc<MetadataBag>,
}

/// `new NativeSessionStorage($options, $handler, $metaBag)`.
#[derive(Debug)]
pub struct NativeSessionStorage {
    state: Mutex<State>,
    /// `PhpBridgeSessionStorage`'s `start()` and `clear()`.
    bridge: bool,
}

/// The options `setOptions()` passes on to `ini_set()`.
const VALID_OPTIONS: [&str; 16] = [
    "cache_expire",
    "cache_limiter",
    "cookie_domain",
    "cookie_httponly",
    "cookie_lifetime",
    "cookie_path",
    "cookie_secure",
    "cookie_samesite",
    "gc_divisor",
    "gc_maxlifetime",
    "gc_probability",
    "lazy_write",
    "name",
    "serialize_handler",
    "use_strict_mode",
    "use_cookies",
];

impl NativeSessionStorage {
    /// `new NativeSessionStorage($options, $handler, $metaBag)`: the
    /// options are added to `cache_limiter` `''`, `cache_expire` 0,
    /// `use_cookies` 1, `lazy_write` 1 and `use_strict_mode` 1.
    pub fn new(
        options: &PhpArray,
        handler: Option<SaveHandler>,
        meta_bag: Option<Arc<MetadataBag>>,
    ) -> Result<Self> {
        let mut options = options.clone();
        for (key, value) in [
            ("cache_limiter", Value::string("")),
            ("cache_expire", Value::Int(0)),
            ("use_cookies", Value::Int(1)),
            ("lazy_write", Value::Int(1)),
            ("use_strict_mode", Value::Int(1)),
        ] {
            if options.get(&key.into()).is_none() {
                options.insert(key, value);
            }
        }
        php_session::session_register_shutdown();
        let storage = Self::unconfigured(meta_bag, false);
        storage.set_options(&options);
        storage.set_save_handler(handler)?;
        Ok(storage)
    }

    /// `new PhpBridgeSessionStorage($handler, $metaBag)`.
    pub(crate) fn bridge(
        handler: Option<SaveHandler>,
        meta_bag: Option<Arc<MetadataBag>>,
    ) -> Result<Self> {
        let storage = Self::unconfigured(meta_bag, true);
        storage.set_save_handler(handler)?;
        Ok(storage)
    }

    fn unconfigured(meta_bag: Option<Arc<MetadataBag>>, bridge: bool) -> Self {
        Self {
            state: Mutex::new(State {
                bags: Vec::new(),
                started: false,
                closed: false,
                save_handler: SaveHandlerProxy::Proxy(Arc::new(AbstractProxy::default())),
                metadata_bag: meta_bag.unwrap_or_default(),
            }),
            bridge,
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `getSaveHandler()`: a proxy.
    pub fn get_save_handler(&self) -> SaveHandlerProxy {
        self.state().save_handler.clone()
    }

    /// `setMetadataBag($metaBag)`.
    pub fn set_metadata_bag(&self, meta_bag: Option<Arc<MetadataBag>>) {
        self.state().metadata_bag = meta_bag.unwrap_or_default();
    }

    /// `setOptions($options)`: the valid ones given to `ini_set()`, unless
    /// headers are sent or a session is active; `cookie_secure` `'auto'`
    /// is left out.
    pub fn set_options(&self, options: &PhpArray) {
        if php_session::headers_sent() || session_status() == Status::Active {
            return;
        }
        for (key, value) in options.iter() {
            let Key::Str(name) = key else {
                continue;
            };
            let Some(name) = VALID_OPTIONS
                .iter()
                .find(|valid| valid.as_bytes() == name.as_bytes())
            else {
                continue;
            };
            if *name == "cookie_secure" && value.identical(&Value::string("auto")) {
                continue;
            }
            ini_set_value(name, value);
        }
    }

    /// `setSaveHandler($saveHandler)`: a handler in a
    /// `SessionHandlerProxy` (PHP's files handler in a
    /// `StrictSessionHandler` when there is none), given to PHP unless
    /// headers are sent or a session is active.
    pub fn set_save_handler(&self, save_handler: Option<SaveHandler>) -> Result<()> {
        let save_handler = match save_handler {
            Some(SaveHandler::Handler(handler)) => {
                SaveHandlerProxy::HandlerProxy(Arc::new(SessionHandlerProxy::new(handler)))
            }
            Some(SaveHandler::HandlerProxy(proxy)) => SaveHandlerProxy::HandlerProxy(proxy),
            Some(SaveHandler::Proxy(proxy)) => SaveHandlerProxy::Proxy(proxy),
            None => SaveHandlerProxy::HandlerProxy(Arc::new(SessionHandlerProxy::new(Arc::new(
                StrictSessionHandler::new(Arc::new(SessionHandler))?,
            )))),
        };
        self.state().save_handler = save_handler.clone();
        if php_session::headers_sent() || session_status() == Status::Active {
            return Ok(());
        }
        if let SaveHandlerProxy::HandlerProxy(proxy) = save_handler {
            php_session::session_set_save_handler(proxy, false);
        }
        Ok(())
    }

    /// `loadSession()`: each bag, then the metadata bag, given its part of
    /// `$_SESSION` (an empty array for a part that is not an array).
    fn load_session(&self) -> Result<()> {
        let bags: Vec<Arc<dyn SessionBagInterface>> = {
            let state = self.state();
            let mut bags: Vec<_> = state.bags.iter().map(|(_, b)| Arc::clone(b)).collect();
            bags.push(Arc::clone(&state.metadata_bag) as Arc<dyn SessionBagInterface>);
            bags
        };
        for bag in bags {
            let key = Key::from_bytes(bag.get_storage_key().as_bytes());
            let reference = php_session::with_session_var(|session| {
                match session {
                    SessionVar::Unset | SessionVar::Other(Value::Null) => {
                        *session = SessionVar::Array(Data::default());
                    }
                    SessionVar::Array(_) => {}
                    SessionVar::Other(other) => return Err(not_an_array(other, false)),
                }
                match session {
                    SessionVar::Array(data) => Ok(data.reference(&key)),
                    SessionVar::Other(other) => Err(not_an_array(other, false)),
                    SessionVar::Unset => Err(not_an_array(&Value::Null, false)),
                }
            })?;
            reference.with(|value| {
                if value.as_array().is_none() {
                    *value = Value::Array(PhpArray::new());
                }
            });
            bag.initialize(&reference)?;
        }
        let mut state = self.state();
        state.started = true;
        state.closed = false;
        Ok(())
    }

    fn native_start(&self) -> Result<bool> {
        if self.state().started {
            return Ok(true);
        }
        if session_status() == Status::Active {
            return Err(Error::new(
                Class::PhpRuntime,
                "Failed to start the session: already started by PHP.",
            ));
        }
        let use_cookies = filter_var(
            &Value::String(ini_get("use_cookies").unwrap_or_default()),
            FILTER_VALIDATE_BOOLEAN,
            &Value::Null,
        )
        .is_ok_and(|v| v.is_truthy());
        if use_cookies && php_session::headers_sent() {
            return Err(Error::new(
                Class::PhpRuntime,
                "Failed to start the session because headers have already been sent by \"\" at line 0.",
            ));
        }
        let name = session_name();
        let cookie = with_globals(|g| g.cookie.get(&Key::from_bytes(name.as_bytes())).cloned());
        if let Some(cookie) = cookie.filter(Value::is_truthy) {
            let files =
                self.state().save_handler.get_save_handler_name().as_deref() == Some("files");
            if files {
                let Value::String(id) = &cookie else {
                    return Err(Error::new(
                        Class::PhpTypeError,
                        format!(
                            "preg_match(): Argument #2 ($subject) must be of type string, {} given",
                            cookie.value_name()
                        ),
                    ));
                };
                if !valid_session_id(id.as_bytes()) {
                    // the session ID in the header is invalid, create a new one
                    let new = session_create_id(b"").map_err(Error::from_throwable)?;
                    set_session_id(new.unwrap_or_default());
                }
            }
        }
        if !php_session::session_start().map_err(Error::from_throwable)? {
            return Err(Error::new(
                Class::PhpRuntime,
                "Failed to start the session.",
            ));
        }
        self.load_session()?;
        Ok(true)
    }

    /// `empty($_SESSION[$key])`, then `unset($_SESSION[$key])`.
    fn unset_if_empty(key: &Key) -> Result<()> {
        php_session::with_session_var(|session| match session {
            SessionVar::Array(data) => {
                if !data.value(key).is_some_and(|v| v.is_truthy()) {
                    data.unset(key);
                }
                Ok(())
            }
            SessionVar::Unset | SessionVar::Other(Value::Null) => Ok(()),
            SessionVar::Other(other) => Err(not_an_array(other, true)),
        })
    }
}

/// PHP's `\Error` for a `$_SESSION` that is not an array, used as one
/// (`unset` for `unset($_SESSION[$key])`): for `true` and numbers; other
/// values (strings, `false`, objects) are not ported.
fn not_an_array(value: &Value, unset: bool) -> Error {
    match value {
        Value::Bool(true) | Value::Int(_) | Value::Float(_) => Error::new(
            Class::PhpError,
            if unset {
                "Cannot unset offset in a non-array variable"
            } else {
                "Cannot use a scalar value as an array"
            },
        ),
        _ => Error::new(
            Class::NotPorted,
            "a $_SESSION that is not an array is not ported",
        ),
    }
}

/// `/^[a-zA-Z0-9,-]{22,250}$/`.
fn valid_session_id(id: &[u8]) -> bool {
    (22..=250).contains(&id.len())
        && id
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b',' || *b == b'-')
}

impl SessionStorageInterface for NativeSessionStorage {
    fn start(&self) -> Result<bool> {
        if !self.bridge {
            return self.native_start();
        }
        if self.state().started {
            return Ok(true);
        }
        self.load_session()?;
        Ok(true)
    }

    fn is_started(&self) -> bool {
        self.state().started
    }

    fn get_id(&self) -> String {
        self.state().save_handler.proxy().get_id()
    }

    fn set_id(&self, id: &str) -> Result<()> {
        let save_handler = self.state().save_handler.clone();
        save_handler.proxy().set_id(id)
    }

    fn get_name(&self) -> String {
        self.state().save_handler.proxy().get_name()
    }

    fn set_name(&self, name: &str) -> Result<()> {
        let save_handler = self.state().save_handler.clone();
        save_handler.proxy().set_name(name)
    }

    /// `regenerate($destroy, $lifetime)`: `false` without an active
    /// session or once headers are sent; a new lifetime saves the session
    /// and starts it again.
    fn regenerate(&self, destroy: bool, lifetime: Option<i64>) -> Result<bool> {
        if session_status() != Status::Active {
            return Ok(false);
        }
        if php_session::headers_sent() {
            return Ok(false);
        }
        if let Some(lifetime) = lifetime {
            let current = Value::String(ini_get("cookie_lifetime").unwrap_or_default());
            if !loose_equals(&Value::Int(lifetime), &current) {
                self.save()?;
                ini_set_value("cookie_lifetime", &Value::Int(lifetime));
                self.start()?;
            }
        }
        if destroy {
            self.state().metadata_bag.stamp_new(None);
        }
        php_session::session_regenerate_id(destroy).map_err(Error::from_throwable)
    }

    fn save(&self) -> Result<()> {
        // reading $_SESSION before PHP set it warns, each time
        let undefined = || {
            if php_session::with_session_var(|s| matches!(s, SessionVar::Unset)) {
                php_session::raise_undefined_session();
            }
        };
        // Store a copy so we can restore the bags in case the session was not left empty
        undefined();
        let session = php_session::with_session_var(|s| s.clone());
        let (keys, metadata_key, save_handler) = {
            let state = self.state();
            (
                state
                    .bags
                    .iter()
                    .map(|(_, bag)| bag.get_storage_key())
                    .collect::<Vec<_>>(),
                state.metadata_bag.get_storage_key(),
                state.save_handler.clone(),
            )
        };
        for key in keys {
            Self::unset_if_empty(&Key::from_bytes(key.as_bytes()))?;
        }
        undefined();
        php_session::with_session_var(|s| {
            if let SessionVar::Array(data) = s
                && data.keys() == [Key::from_bytes(metadata_key.as_bytes())]
            {
                data.unset(&Key::from_bytes(metadata_key.as_bytes()));
            }
        });

        // Register error handler to add information about the current save handler
        let class = match &save_handler {
            SaveHandlerProxy::HandlerProxy(proxy) => proxy.get_handler().class(),
            SaveHandlerProxy::Proxy(proxy) => proxy.class().to_owned(),
        };
        let written =
            php_session::with_write_close_handler(class, php_session::session_write_close);
        // Restore only if not empty
        undefined();
        php_session::with_session_var(|s| {
            if s.is_truthy() {
                *s = session;
            }
        });
        written.map_err(Error::from_throwable)?;

        let mut state = self.state();
        state.closed = true;
        state.started = false;
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        let bags: Vec<_> = self
            .state()
            .bags
            .iter()
            .map(|(_, b)| Arc::clone(b))
            .collect();
        for bag in bags {
            bag.clear()?;
        }
        if !self.bridge {
            php_session::with_session_var(|s| *s = SessionVar::Array(Data::default()));
        }
        self.load_session()
    }

    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>> {
        let (bag, started, active) = {
            let state = self.state();
            (
                state
                    .bags
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, b)| Arc::clone(b)),
                state.started,
                state.save_handler.is_active(),
            )
        };
        let Some(bag) = bag else {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!("The SessionBagInterface \"{name}\" is not registered."),
            ));
        };
        if !started && active {
            self.load_session()?;
        } else if !started {
            self.start()?;
        }
        Ok(bag)
    }

    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>) -> Result<()> {
        let mut state = self.state();
        if state.started {
            return Err(Error::new(
                Class::PhpLogic,
                "Cannot register a bag when the session is already started.",
            ));
        }
        let name = bag.get_name();
        match state.bags.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = bag,
            None => state.bags.push((name, bag)),
        }
        Ok(())
    }

    fn get_metadata_bag(&self) -> Arc<MetadataBag> {
        Arc::clone(&self.state().metadata_bag)
    }
}

/// `$_SESSION[$key]`, for tests and callers that read the session.
pub fn session_value(key: &str) -> Option<Value> {
    php_session::with_session_var(|s| match s {
        SessionVar::Array(data) => data.value(&Key::from_bytes(key.as_bytes())),
        _ => None,
    })
}

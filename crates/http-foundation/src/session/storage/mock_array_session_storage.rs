//! Port of Symfony\Component\HttpFoundation\Session\Storage\MockArraySessionStorage.php:
//! a storage that keeps the session in memory, for tests.
//!
//! The data is a [`Data`]: keys and PHP references, which `loadSession()`
//! hands to each bag, so the bags write into it. `setSessionData()` and
//! `clear()` give the storage new data, and bags keep the old references
//! until the session is loaded again, as in PHP.
//!
//! `start()` is overridden by `MockFileSessionStorage` and called by the
//! methods below that start a session; those take the `start` to call
//! (PHP's `$this->start()`).
//!
//! Session ids are `bin2hex(random_bytes(16))`, from the OS (`getrandom`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/MockArraySessionStorage.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::PhpArray;

use super::{metadata_bag::MetadataBag, session_storage_interface::SessionStorageInterface};
use crate::{
    exception::{Class, Error, Result},
    session::{Data, session_bag_interface::SessionBagInterface},
};

/// `MockArraySessionStorage::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\MockArraySessionStorage";

/// The storage's state (its protected properties).
#[derive(Debug)]
pub(crate) struct State {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) started: bool,
    pub(crate) closed: bool,
    pub(crate) data: Data,
    pub(crate) metadata_bag: Arc<MetadataBag>,
    /// `$this->bags`, by name, in order.
    pub(crate) bags: Vec<(String, Arc<dyn SessionBagInterface>)>,
}

/// `new MockArraySessionStorage($name, $metaBag)`.
#[derive(Debug)]
pub struct MockArraySessionStorage {
    state: Mutex<State>,
}

impl Default for MockArraySessionStorage {
    fn default() -> Self {
        Self::new("MOCKSESSID", None)
    }
}

/// `bin2hex(random_bytes($length))`.
pub(crate) fn random_hex(length: usize) -> Result<String> {
    let mut bytes = vec![0_u8; length];
    getrandom::fill(&mut bytes)
        .map_err(|_| Error::new(Class::PhpRuntime, "Could not gather sufficient random data"))?;
    Ok(bytes.iter().fold(String::new(), |mut out, b| {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
        out
    }))
}

impl MockArraySessionStorage {
    pub fn new(name: impl Into<String>, meta_bag: Option<Arc<MetadataBag>>) -> Self {
        Self {
            state: Mutex::new(State {
                id: String::new(),
                name: name.into(),
                started: false,
                closed: false,
                data: Data::default(),
                metadata_bag: meta_bag.unwrap_or_default(),
                bags: Vec::new(),
            }),
        }
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `setSessionData($array)`.
    pub fn set_session_data(&self, array: &PhpArray) {
        self.state().data = Data::from_array(array);
    }

    /// `setMetadataBag($bag)`.
    pub fn set_metadata_bag(&self, bag: Option<Arc<MetadataBag>>) {
        self.state().metadata_bag = bag.unwrap_or_default();
    }

    /// `generateId()`.
    pub(crate) fn generate_id() -> Result<String> {
        random_hex(16)
    }

    /// `loadSession()`: each bag, then the metadata bag, initialized with
    /// its part of the data (an empty array when there is none).
    pub(crate) fn load_session(&self) -> Result<()> {
        let bags: Vec<Arc<dyn SessionBagInterface>> = {
            let state = self.state();
            let mut bags: Vec<_> = state.bags.iter().map(|(_, bag)| Arc::clone(bag)).collect();
            bags.push(Arc::clone(&state.metadata_bag) as Arc<dyn SessionBagInterface>);
            bags
        };
        for bag in bags {
            let key = bag.get_storage_key();
            let reference = self.state().data.entry_or_empty_array(&key);
            bag.initialize(&reference)?;
        }
        let mut state = self.state();
        state.started = true;
        state.closed = false;
        Ok(())
    }

    /// `start()` as this class has it.
    pub(crate) fn start_array(&self) -> Result<bool> {
        if self.state().started {
            return Ok(true);
        }
        self.ensure_id()?;
        self.load_session()?;
        Ok(true)
    }

    /// `if (!$this->id) { $this->id = $this->generateId(); }`.
    pub(crate) fn ensure_id(&self) -> Result<()> {
        let mut state = self.state();
        if state.id.is_empty() || state.id == "0" {
            state.id = Self::generate_id()?;
        }
        Ok(())
    }

    /// `regenerate()`, with `$this->start()` as `start`.
    pub(crate) fn regenerate_with(
        &self,
        lifetime: Option<i64>,
        start: impl Fn() -> Result<bool>,
    ) -> Result<bool> {
        if !self.state().started {
            start()?;
        }
        let metadata_bag = Arc::clone(&self.state().metadata_bag);
        metadata_bag.stamp_new(lifetime);
        self.state().id = Self::generate_id()?;
        Ok(true)
    }

    /// `getBag()`, with `$this->start()` as `start`.
    pub(crate) fn get_bag_with(
        &self,
        name: &str,
        start: impl Fn() -> Result<bool>,
    ) -> Result<Arc<dyn SessionBagInterface>> {
        let (bag, started) = {
            let state = self.state();
            let bag = state
                .bags
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, bag)| Arc::clone(bag));
            (bag, state.started)
        };
        let Some(bag) = bag else {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!("The SessionBagInterface \"{name}\" is not registered."),
            ));
        };
        if !started {
            start()?;
        }
        Ok(bag)
    }

    /// `setId()` as this class has it.
    pub(crate) fn set_id_array(&self, id: &str) -> Result<()> {
        let mut state = self.state();
        if state.started {
            return Err(Error::new(
                Class::PhpLogic,
                "Cannot set session ID after the session has started.",
            ));
        }
        id.clone_into(&mut state.id);
        Ok(())
    }
}

impl SessionStorageInterface for MockArraySessionStorage {
    fn start(&self) -> Result<bool> {
        self.start_array()
    }

    fn is_started(&self) -> bool {
        self.state().started
    }

    fn get_id(&self) -> String {
        self.state().id.clone()
    }

    fn set_id(&self, id: &str) -> Result<()> {
        self.set_id_array(id)
    }

    fn get_name(&self) -> String {
        self.state().name.clone()
    }

    fn set_name(&self, name: &str) -> Result<()> {
        name.clone_into(&mut self.state().name);
        Ok(())
    }

    fn regenerate(&self, _destroy: bool, lifetime: Option<i64>) -> Result<bool> {
        self.regenerate_with(lifetime, || self.start())
    }

    fn save(&self) -> Result<()> {
        let mut state = self.state();
        if !state.started || state.closed {
            return Err(Error::new(
                Class::PhpRuntime,
                "Trying to save a session that was not started yet or was already closed.",
            ));
        }
        // nothing to do since we don't persist the session data
        state.closed = false;
        state.started = false;
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        // clear out the bags
        let bags: Vec<_> = self
            .state()
            .bags
            .iter()
            .map(|(_, b)| Arc::clone(b))
            .collect();
        for bag in bags {
            bag.clear()?;
        }
        // clear out the session
        self.state().data = Data::default();
        // reconnect the bags to the session
        self.load_session()
    }

    fn get_bag(&self, name: &str) -> Result<Arc<dyn SessionBagInterface>> {
        self.get_bag_with(name, || self.start())
    }

    fn register_bag(&self, bag: Arc<dyn SessionBagInterface>) -> Result<()> {
        let name = bag.get_name();
        let mut state = self.state();
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

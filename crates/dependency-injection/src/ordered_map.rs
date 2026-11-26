//! Insertion-ordered string-keyed maps and per-thread loading stacks for
//! the run-time classes.
//!
//! PHP arrays keep insertion order, and `unset()` then a new assignment
//! moves a key to the end; [`OrderedMap`] does the same. [`ThreadStack`]
//! keeps a container's `$loading`/`$resolving` arrays per thread: PHP runs
//! one request per process, and a Rust container shared by several
//! threads must not see another thread's creation as a circular
//! reference.
//!
//! Upstream: none

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};

/// A string-keyed map in insertion order.
#[derive(Debug, Clone)]
pub(crate) struct OrderedMap<V> {
    entries: Vec<Option<(String, V)>>,
    index: HashMap<String, usize>,
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<V> OrderedMap<V> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        let i = *self.index.get(key)?;
        self.entries.get(i)?.as_ref().map(|(_, v)| v)
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// `$map[$key] = $value`: an existing key keeps its position.
    pub(crate) fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        if let Some(&i) = self.index.get(&key)
            && let Some(Some(entry)) = self.entries.get_mut(i)
        {
            entry.1 = value;
            return;
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push(Some((key, value)));
    }

    /// `unset($map[$key])`.
    pub(crate) fn remove(&mut self, key: &str) -> Option<V> {
        let i = self.index.remove(key)?;
        let removed = self.entries.get_mut(i)?.take().map(|(_, v)| v);
        if self.entries.len() > 32 && self.index.len() * 2 < self.entries.len() {
            self.compact();
        }
        removed
    }

    fn compact(&mut self) {
        self.entries.retain(Option::is_some);
        self.index.clear();
        for (i, (key, _)) in self.entries.iter().flatten().enumerate() {
            self.index.insert(key.clone(), i);
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().flatten().map(|(k, v)| (k.as_str(), v))
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.iter().map(|(k, _)| k)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }
}

impl<V, K: Into<String>> FromIterator<(K, V)> for OrderedMap<V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

/// A `$loading`-style array of ids (keys in insertion order), one per
/// thread.
#[derive(Debug, Default)]
pub(crate) struct ThreadStack(Mutex<HashMap<ThreadId, Vec<String>>>);

impl ThreadStack {
    fn lock(&self) -> MutexGuard<'_, HashMap<ThreadId, Vec<String>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The ids of the current thread, in insertion order.
    pub(crate) fn current(&self) -> Vec<String> {
        self.lock()
            .get(&thread::current().id())
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn contains(&self, id: &str) -> bool {
        self.lock()
            .get(&thread::current().id())
            .is_some_and(|ids| ids.iter().any(|i| i == id))
    }

    /// `$loading[$id] = true`: added at the end unless present.
    pub(crate) fn push(&self, id: &str) {
        let mut map = self.lock();
        let ids = map.entry(thread::current().id()).or_default();
        if !ids.iter().any(|i| i == id) {
            ids.push(id.to_owned());
        }
    }

    /// `unset($loading[$id])`.
    pub(crate) fn remove(&self, id: &str) {
        let mut map = self.lock();
        let key = thread::current().id();
        if let Some(ids) = map.get_mut(&key) {
            ids.retain(|i| i != id);
            if ids.is_empty() {
                map.remove(&key);
            }
        }
    }

    /// `end($loading)`.
    pub(crate) fn last(&self) -> Option<String> {
        self.lock()
            .get(&thread::current().id())
            .and_then(|ids| ids.last().cloned())
    }
}

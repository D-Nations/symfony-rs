//! An insertion-ordered map with string keys: the PHP arrays the
//! container keys by service id, parameter name, tag or class name
//! (`$definitions`, `$aliasDefinitions`, `$envPlaceholders`, ...).
//!
//! It keeps PHP's order rules: a new key goes last, assigning an existing
//! key keeps its place, and removing a key keeps the order of the others.
//! Keys stay strings: PHP turns a numeric id such as `"1"` into an int
//! key, which this map does not, since every caller reads ids as strings
//! (`getServiceIds()` casts them back with `strval`).
//!
//! Upstream: none

use std::collections::HashMap;
use std::fmt;

/// An insertion-ordered map from `String` keys to `V`.
#[derive(Clone)]
pub struct IdMap<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<V: fmt::Debug> fmt::Debug for IdMap<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V: PartialEq> PartialEq for IdMap<V> {
    /// `===` on PHP arrays: same keys and values in the same order.
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<V> IdMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        let slot = *self.index.get(key)?;
        self.entries.get(slot).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        let slot = *self.index.get(key)?;
        self.entries.get_mut(slot).map(|(_, v)| v)
    }

    /// `$map[$key] = $value`: the old value, if any, whose place the new
    /// one keeps.
    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<V> {
        let key = key.into();
        if let Some(&slot) = self.index.get(&key) {
            let entry = self.entries.get_mut(slot)?;
            return Some(std::mem::replace(&mut entry.1, value));
        }
        self.index.insert(key.clone(), self.entries.len());
        self.entries.push((key, value));
        None
    }

    /// `$map[$key] ??= $value()`, returning the entry.
    pub fn get_or_insert_with(&mut self, key: &str, value: impl FnOnce() -> V) -> &mut V {
        let slot = if let Some(&slot) = self.index.get(key) {
            slot
        } else {
            self.index.insert(key.to_string(), self.entries.len());
            self.entries.push((key.to_string(), value()));
            self.entries.len() - 1
        };
        &mut self.entries[slot].1
    }

    /// `unset($map[$key])`.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let slot = self.index.remove(key)?;
        let (_, value) = self.entries.remove(slot);
        for (k, _) in &self.entries[slot..] {
            if let Some(i) = self.index.get_mut(k) {
                *i -= 1;
            }
        }
        Some(value)
    }

    /// `$map = []`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut V)> {
        self.entries.iter_mut().map(|(k, v)| (k.as_str(), v))
    }

    /// `array_keys($map)`.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.iter().map(|(k, _)| k)
    }

    /// `array_values($map)`.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// The first entry (`reset()`).
    pub fn first(&self) -> Option<(&str, &V)> {
        self.iter().next()
    }
}

impl<V: Clone> IdMap<V> {
    /// `$this + $other`: the keys of `other` that this map lacks, added
    /// at the end.
    pub fn union(&mut self, other: &Self) {
        for (key, value) in other.iter() {
            if !self.contains_key(key) {
                self.insert(key, value.clone());
            }
        }
    }
}

impl<K: Into<String>, V> FromIterator<(K, V)> for IdMap<V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

//! An ordered map with PHP array semantics for the schema objects' keyed
//! collections (`$this->_indexes[$name] = $index`): insertion order,
//! replacing a key keeps its position, `$array[] = $x` appends under the
//! next integer key. Keys are the strings PHP uses; an integer key is its
//! decimal string, as PHP's canonical numeric strings and ints are one key.
//!
//! Upstream: none

/// A PHP array of `V` keyed by strings.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayMap<V> {
    entries: Vec<(String, V)>,
    next_index: i64,
}

impl<V> Default for ArrayMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_index: 0,
        }
    }
}

/// The integer a canonical decimal key stands for (`"5"`, `"-3"`, not
/// `"05"` or `"+1"`).
fn int_key(key: &str) -> Option<i64> {
    let value: i64 = key.parse().ok()?;
    (value.to_string() == key).then_some(value)
}

impl<V> ArrayMap<V> {
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
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// `$array[$key] = $value`.
    pub fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        if let Some(i) = int_key(&key) {
            if i >= self.next_index {
                self.next_index = i.saturating_add(1);
            }
        }
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// `$array[] = $value`.
    pub fn push(&mut self, value: V) {
        let key = self.next_index.to_string();
        self.insert(key, value);
    }

    /// `unset($array[$key])`.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let position = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(position).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.entries.iter_mut().map(|(_, v)| v)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.next_index = 0;
    }

    /// Replaces the entry under `old_key` by `new_key => value` in the
    /// same position (as `array_combine()` of changed keys does).
    pub(crate) fn rekey(&mut self, old_key: &str, new_key: String, value: V) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| k == old_key) {
            *entry = (new_key, value);
        }
    }
}

impl<V> FromIterator<(String, V)> for ArrayMap<V> {
    fn from_iter<T: IntoIterator<Item = (String, V)>>(iter: T) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

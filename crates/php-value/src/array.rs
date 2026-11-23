//! PHP arrays: ordered maps with int and string keys and value semantics
//! (a clone is a copy; writes copy on demand). Written from PHP 8.4's
//! recorded behaviour (tests/data/php.json):
//!
//! - a string key that is a canonical decimal int in range (`"7"`, `"-3"`,
//!   not `"07"`, `"+1"`, `"-0"`, `" 1"`) becomes that int; `true`/`false`
//!   become 1/0 and `null` becomes `""`;
//! - assigning an existing key keeps its place;
//! - appending uses one more than the largest int key ever used (removals
//!   do not lower it, and since PHP 8.3 a negative key counts too: after
//!   -5 comes -4), or 0 if no int key was used; after `PHP_INT_MAX`,
//!   appending fails.
//!
//! Upstream: none

use std::{collections::HashMap, fmt, sync::Arc};

use crate::{string::PhpString, value::Value};

/// An array key.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Int(i64),
    Str(PhpString),
}

impl Key {
    /// The key PHP uses for the string `bytes`.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match canonical_int(bytes) {
            Some(i) => Self::Int(i),
            None => Self::Str(PhpString::from(bytes)),
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Str(_) => None,
        }
    }

    pub fn as_str(&self) -> Option<&PhpString> {
        match self {
            Self::Int(_) => None,
            Self::Str(s) => Some(s),
        }
    }

    /// The key as a PHP value (int or string).
    pub fn to_value(&self) -> Value {
        match self {
            Self::Int(i) => Value::Int(*i),
            Self::Str(s) => Value::String(s.clone()),
        }
    }
}

fn canonical_int(bytes: &[u8]) -> Option<i64> {
    let digits = bytes.strip_prefix(b"-").unwrap_or(bytes);
    let canonical = match digits {
        [b'0'] => bytes.len() == 1,
        [b'1'..=b'9', rest @ ..] => rest.iter().all(u8::is_ascii_digit),
        _ => false,
    };
    if !canonical {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

impl Key {
    /// The key PHP uses for `$array[$value]`: ints and strings as
    /// `from_bytes`, `true`/`false` as 1/0, `null` as `""`, floats
    /// truncated toward zero (PHP warns when it drops a fraction).
    /// Arrays and objects are not keys (`None`, PHP's "Illegal offset
    /// type").
    pub fn from_value(value: &Value) -> Option<Self> {
        Some(match value {
            Value::Null => Self::Str(PhpString::default()),
            Value::Bool(b) => Self::Int(i64::from(*b)),
            Value::Int(i) => Self::Int(*i),
            Value::Float(f) => Self::Int(crate::numeric::float_key(*f)),
            Value::String(s) => Self::from(s.clone()),
            Value::Array(_) | Value::Object(_) => return None,
        })
    }
}

impl From<i64> for Key {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Self::from_bytes(s.as_bytes())
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Self::from_bytes(s.as_bytes())
    }
}

impl From<PhpString> for Key {
    fn from(s: PhpString) -> Self {
        match canonical_int(&s) {
            Some(i) => Self::Int(i),
            None => Self::Str(s),
        }
    }
}

impl From<bool> for Key {
    fn from(b: bool) -> Self {
        Self::Int(i64::from(b))
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(i) => write!(f, "{i}"),
            Self::Str(s) => write!(f, "{s:?}"),
        }
    }
}

/// PHP's `Error: Cannot add element to the array as the next element is
/// already occupied`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextElementOccupied;

impl fmt::Display for NextElementOccupied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cannot add element to the array as the next element is already occupied")
    }
}

impl std::error::Error for NextElementOccupied {}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Next {
    /// No int key yet: append uses 0.
    #[default]
    Start,
    At(i64),
    Occupied,
}

#[derive(Clone, Default)]
struct Data {
    entries: Vec<(Key, Value)>,
    index: HashMap<Key, usize>,
    next: Next,
}

/// A PHP array.
#[derive(Clone, Default)]
pub struct PhpArray(Arc<Data>);

impl PhpArray {
    pub fn new() -> Self {
        Self::default()
    }

    /// `[$a, $b, ...]`.
    pub fn list(values: impl IntoIterator<Item = Value>) -> Self {
        let mut array = Self::new();
        for value in values {
            // A fresh list cannot reach PHP_INT_MAX.
            let _ = array.push(value);
        }
        array
    }

    fn data(&mut self) -> &mut Data {
        Arc::make_mut(&mut self.0)
    }

    pub fn len(&self) -> usize {
        self.0.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.entries.is_empty()
    }

    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.0.index.get(key).map(|&i| &self.0.entries[i].1)
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        self.0.index.contains_key(key)
    }

    /// `$array[$key] = $value`.
    pub fn insert(&mut self, key: impl Into<Key>, value: Value) {
        let key = key.into();
        let data = self.data();
        if let Key::Int(i) = key {
            data.next = match data.next {
                Next::Occupied => Next::Occupied,
                Next::At(n) if i < n => Next::At(n),
                _ => i.checked_add(1).map_or(Next::Occupied, Next::At),
            };
        }
        match data.index.get(&key) {
            Some(&i) => data.entries[i].1 = value,
            None => {
                data.index.insert(key.clone(), data.entries.len());
                data.entries.push((key, value));
            }
        }
    }

    /// The key `$array[] = ...` would use, `None` when the next element is
    /// occupied (after `PHP_INT_MAX`).
    pub fn next_index(&self) -> Option<i64> {
        match self.0.next {
            Next::Start => Some(0),
            Next::At(n) => Some(n),
            Next::Occupied => None,
        }
    }

    /// `$array[] = $value`.
    pub fn push(&mut self, value: Value) -> Result<Key, NextElementOccupied> {
        let key = self.next_index().ok_or(NextElementOccupied)?;
        self.insert(key, value);
        Ok(Key::Int(key))
    }

    /// `unset($array[$key])`.
    pub fn remove(&mut self, key: &Key) -> Option<Value> {
        let i = *self.0.index.get(key)?;
        let data = self.data();
        let (_, value) = data.entries.remove(i);
        data.index.remove(key);
        for position in data.index.values_mut() {
            if *position > i {
                *position -= 1;
            }
        }
        Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Value)> {
        self.0.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.entries.iter().map(|(_, v)| v)
    }

    pub fn first(&self) -> Option<(&Key, &Value)> {
        self.0.entries.first().map(|(k, v)| (k, v))
    }

    pub fn last(&self) -> Option<(&Key, &Value)> {
        self.0.entries.last().map(|(k, v)| (k, v))
    }

    /// `array_is_list()`: keys are 0, 1, 2, ... in order.
    pub fn is_list(&self) -> bool {
        self.0
            .entries
            .iter()
            .enumerate()
            .all(|(i, (k, _))| *k == Key::Int(i as i64))
    }

    /// `$this += $other`: the keys of `other` not in `self`, appended in
    /// `other`'s order.
    pub fn union(&mut self, other: &PhpArray) {
        for (key, value) in other.iter() {
            if !self.contains_key(key) {
                self.insert(key.clone(), value.clone());
            }
        }
    }

    /// A mutable reference to the value at `key`, if present.
    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        let i = *self.0.index.get(key)?;
        Some(&mut self.data().entries[i].1)
    }

    /// `ksort($array)` (`SORT_REGULAR`): keys in [`compare_keys`] order,
    /// equal keys keeping their order (PHP's sort is stable since 8.0).
    pub fn ksort(&mut self) {
        let data = self.data();
        data.entries.sort_by(|(a, _), (b, _)| compare_keys(a, b));
        for (position, (key, _)) in data.entries.iter().enumerate() {
            data.index.insert(key.clone(), position);
        }
    }
}

/// PHP 8's `<=>` on two array keys, as `ksort()` compares them: ints by
/// value; two strings that are both numeric (`is_numeric()`) by value,
/// otherwise byte by byte; an int and a numeric string by value, an int
/// and a non-numeric string as the int's decimal text against the string
/// (PHP manual, "Comparison Operators").
pub fn compare_keys(a: &Key, b: &Key) -> std::cmp::Ordering {
    use crate::numeric::numeric_value;
    let number = |s: &PhpString| numeric_value(s.as_bytes()).map(crate::numeric::Number::as_f64);
    match (a, b) {
        (Key::Int(a), Key::Int(b)) => a.cmp(b),
        (Key::Str(a), Key::Str(b)) => match (number(a), number(b)) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            _ => a.as_bytes().cmp(b.as_bytes()),
        },
        (Key::Int(i), Key::Str(s)) => match number(s) {
            Some(y) => (*i as f64).total_cmp(&y),
            None => i.to_string().as_bytes().cmp(s.as_bytes()),
        },
        (Key::Str(_), Key::Int(_)) => compare_keys(b, a).reverse(),
    }
}

impl<K: Into<Key>> FromIterator<(K, Value)> for PhpArray {
    fn from_iter<I: IntoIterator<Item = (K, Value)>>(iter: I) -> Self {
        let mut array = Self::new();
        for (k, v) in iter {
            array.insert(k, v);
        }
        array
    }
}

/// `===`: the same key/value pairs in the same order.
impl PartialEq for PhpArray {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.entries == other.0.entries
    }
}

impl fmt::Debug for PhpArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

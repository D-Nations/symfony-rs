//! A PHP array keyed by name, as `RouteCollection` keeps its routes,
//! aliases and priorities: insertion order, assignment to an existing name
//! in place, `unset()` then assignment appends. Lookup, assignment and
//! removal are O(1): entries live in a vector, removed ones leave a hole
//! (compacted when holes outnumber entries), and a hash index maps each
//! name to its slot, as PHP's own arrays do.
//!
//! Upstream: none

use std::collections::HashMap;

use php_value::PhpString;

#[derive(Debug, Clone)]
pub(crate) struct Ordered<T> {
    slots: Vec<Option<(PhpString, T)>>,
    index: HashMap<Vec<u8>, usize>,
}

impl<T> Default for Ordered<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<T> Ordered<T> {
    /// `count($array)`.
    pub(crate) fn len(&self) -> usize {
        self.index.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// `isset($array[$name])` for a value that cannot be null.
    pub(crate) fn contains(&self, name: &[u8]) -> bool {
        self.index.contains_key(name)
    }

    /// `$array[$name] ?? null`.
    pub(crate) fn get(&self, name: &[u8]) -> Option<&T> {
        let &i = self.index.get(name)?;
        self.slots[i].as_ref().map(|(_, v)| v)
    }

    pub(crate) fn get_mut(&mut self, name: &[u8]) -> Option<&mut T> {
        let &i = self.index.get(name)?;
        self.slots[i].as_mut().map(|(_, v)| v)
    }

    /// `$array[$name] = $value`: in place for an existing name, else last.
    pub(crate) fn set(&mut self, name: PhpString, value: T) {
        if let Some(&i) = self.index.get(name.as_bytes()) {
            self.slots[i] = Some((name, value))
        } else {
            self.index
                .insert(name.as_bytes().to_vec(), self.slots.len());
            self.slots.push(Some((name, value)));
        }
    }

    /// `unset($array[$name])`.
    pub(crate) fn remove(&mut self, name: &[u8]) -> Option<(PhpString, T)> {
        let i = self.index.remove(name)?;
        let removed = self.slots[i].take();
        self.compact();
        removed
    }

    /// Keeps the entries `keep` accepts, in order.
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&PhpString, &T) -> bool) {
        for slot in &mut self.slots {
            if let Some((name, value)) = slot
                && !keep(name, value)
            {
                self.index.remove(name.as_bytes());
                *slot = None;
            }
        }
        self.compact();
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&PhpString, &T)> {
        self.slots.iter().flatten().map(|(n, v)| (n, v))
    }

    pub(crate) fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().flatten().map(|(_, v)| v)
    }

    /// Empties the array, giving its entries in order.
    pub(crate) fn take(&mut self) -> impl Iterator<Item = (PhpString, T)> + use<T> {
        self.index.clear();
        std::mem::take(&mut self.slots).into_iter().flatten()
    }

    /// Drops the holes once they outnumber the entries, so iteration and
    /// memory stay proportional to the entries.
    fn compact(&mut self) {
        let holes = self.slots.len() - self.index.len();
        if holes <= self.index.len().max(16) {
            return;
        }
        self.slots.retain(Option::is_some);
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some((name, _)) = slot {
                self.index.insert(name.as_bytes().to_vec(), i);
            }
        }
    }
}

impl<T> FromIterator<(PhpString, T)> for Ordered<T> {
    fn from_iter<I: IntoIterator<Item = (PhpString, T)>>(entries: I) -> Self {
        let mut ordered = Self::default();
        for (name, value) in entries {
            ordered.set(name, value);
        }
        ordered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(o: &Ordered<i64>) -> Vec<(String, i64)> {
        o.iter()
            .map(|(n, v)| (n.to_string_lossy().into_owned(), *v))
            .collect()
    }

    #[test]
    fn keeps_php_array_order() {
        let mut o = Ordered::default();
        o.set("a".into(), 1);
        o.set("b".into(), 2);
        o.set("c".into(), 3);
        // assignment to an existing name keeps its place
        o.set("a".into(), 10);
        assert_eq!(
            names(&o),
            [("a".into(), 10), ("b".into(), 2), ("c".into(), 3)]
        );
        // unset then assignment appends
        assert_eq!(o.remove(b"a").map(|(_, v)| v), Some(10));
        o.set("a".into(), 11);
        assert_eq!(
            names(&o),
            [("b".into(), 2), ("c".into(), 3), ("a".into(), 11)]
        );
        assert_eq!(o.get(b"c"), Some(&3));
        assert_eq!(o.get(b"x"), None);
        o.retain(|_, v| *v != 2);
        assert_eq!(names(&o), [("c".into(), 3), ("a".into(), 11)]);
        assert_eq!(o.len(), 2);
    }

    #[test]
    fn compaction_keeps_order_and_lookups() {
        let mut o = Ordered::default();
        for i in 0..1000 {
            o.set(PhpString::from(format!("r{i}")), i);
        }
        for i in (0..1000).filter(|i| i % 3 != 0) {
            o.remove(format!("r{i}").as_bytes());
        }
        let expected: Vec<i64> = (0..1000).filter(|i| i % 3 == 0).collect();
        assert_eq!(o.iter().map(|(_, v)| *v).collect::<Vec<_>>(), expected);
        assert!(o.slots.len() < 2 * o.len() + 17, "{} slots", o.slots.len());
        for i in &expected {
            assert_eq!(o.get(format!("r{i}").as_bytes()), Some(i));
        }
        assert_eq!(o.take().count(), expected.len());
        assert!(o.is_empty());
    }
}

//! PHP strings: immutable byte strings, cheap to clone.
//!
//! Upstream: none

use std::{borrow::Borrow, fmt, ops::Deref, sync::Arc};

/// A PHP string: bytes, not necessarily UTF-8.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PhpString(Arc<[u8]>);

impl PhpString {
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The string as UTF-8, if it is.
    pub fn to_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// The string as UTF-8, with invalid sequences replaced.
    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for PhpString {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl Borrow<[u8]> for PhpString {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl From<&str> for PhpString {
    fn from(s: &str) -> Self {
        Self(Arc::from(s.as_bytes()))
    }
}

impl From<String> for PhpString {
    fn from(s: String) -> Self {
        Self(Arc::from(s.into_bytes()))
    }
}

impl From<&[u8]> for PhpString {
    fn from(b: &[u8]) -> Self {
        Self(Arc::from(b))
    }
}

impl From<Vec<u8>> for PhpString {
    fn from(b: Vec<u8>) -> Self {
        Self(Arc::from(b))
    }
}

impl PartialEq<str> for PhpString {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other.as_bytes()
    }
}

impl PartialEq<&str> for PhpString {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == other.as_bytes()
    }
}

impl fmt::Debug for PhpString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_str() {
            Some(s) => write!(f, "{s:?}"),
            None => write!(f, "b{:?}", self.to_string_lossy()),
        }
    }
}

/// Lossy for bytes that are not UTF-8.
impl fmt::Display for PhpString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

/// PHP's byte-based edit distance with unit costs.
pub fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let replace = previous[j] + usize::from(ca != cb);
            let insert = current[j] + 1;
            let delete = previous[j + 1] + 1;
            current[j + 1] = replace.min(insert).min(delete);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// PHP's default trim characters (not Rust's ASCII whitespace set).
pub fn trim(s: &[u8]) -> &[u8] {
    let strip = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0B);
    let start = s.iter().position(|b| !strip(b)).unwrap_or(s.len());
    let end = s.iter().rposition(|b| !strip(b)).map_or(start, |i| i + 1);
    &s[start..end]
}

/// PHP string truthiness: only the empty string and `0` are false.
pub fn truthy(s: &[u8]) -> bool {
    !(s.is_empty() || s == b"0")
}

pub fn strnatcmp(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    natural(a, b, false)
}

pub fn strnatcasecmp(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    natural(a, b, true)
}

fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn natural(a: &[u8], b: &[u8], fold: bool) -> std::cmp::Ordering {
    if a.is_empty() || b.is_empty() {
        return a.len().cmp(&b.len());
    }
    let (mut i, mut j) = (0usize, 0usize);
    let mut leading = true;
    let mut after_digits = false;
    loop {
        let at = |s: &[u8], k: usize| s.get(k).copied().unwrap_or(0);
        // leading zeros of the first number are not significant
        if leading {
            while at(a, i) == b'0' && i + 1 < a.len() && at(a, i + 1).is_ascii_digit() {
                i += 1;
            }
            while at(b, j) == b'0' && j + 1 < b.len() && at(b, j + 1).is_ascii_digit() {
                j += 1;
            }
            leading = false;
        }
        // Whitespace is skipped, except right after a run of digits
        // (observed: "2 B" < "2-", but "a B" > "a-").
        if !after_digits {
            while is_c_space(at(a, i)) {
                i += 1;
            }
            while is_c_space(at(b, j)) {
                j += 1;
            }
        }
        after_digits = false;
        let (ca, cb) = (at(a, i), at(b, j));
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let fractional = ca == b'0' || cb == b'0';
            let result = if fractional {
                compare_left(a, &mut i, b, &mut j)
            } else {
                compare_right(a, &mut i, b, &mut j)
            };
            if result != std::cmp::Ordering::Equal {
                return result;
            }
            match (i >= a.len(), j >= b.len()) {
                (true, true) => return std::cmp::Ordering::Equal,
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {
                    after_digits = true;
                    continue;
                }
            }
        }
        let (ca, cb) = if fold {
            (ca.to_ascii_uppercase(), cb.to_ascii_uppercase())
        } else {
            (ca, cb)
        };
        match ca.cmp(&cb) {
            std::cmp::Ordering::Equal => {}
            o => return o,
        }
        i += 1;
        j += 1;
        match (i >= a.len(), j >= b.len()) {
            (true, true) => return std::cmp::Ordering::Equal,
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
    }
}

fn compare_right(a: &[u8], i: &mut usize, b: &[u8], j: &mut usize) -> std::cmp::Ordering {
    let mut bias = std::cmp::Ordering::Equal;
    loop {
        let da = a.get(*i).is_some_and(u8::is_ascii_digit);
        let db = b.get(*j).is_some_and(u8::is_ascii_digit);
        match (da, db) {
            (false, false) => return bias,
            (false, true) => return std::cmp::Ordering::Less,
            (true, false) => return std::cmp::Ordering::Greater,
            (true, true) => {
                if bias == std::cmp::Ordering::Equal {
                    bias = a[*i].cmp(&b[*j]);
                }
            }
        }
        *i += 1;
        *j += 1;
    }
}

fn compare_left(a: &[u8], i: &mut usize, b: &[u8], j: &mut usize) -> std::cmp::Ordering {
    loop {
        let da = a.get(*i).is_some_and(u8::is_ascii_digit);
        let db = b.get(*j).is_some_and(u8::is_ascii_digit);
        match (da, db) {
            (false, false) => return std::cmp::Ordering::Equal,
            (false, true) => return std::cmp::Ordering::Less,
            (true, false) => return std::cmp::Ordering::Greater,
            (true, true) => match a[*i].cmp(&b[*j]) {
                std::cmp::Ordering::Equal => {}
                o => return o,
            },
        }
        *i += 1;
        *j += 1;
    }
}

#[cfg(test)]
mod shared_primitive_tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn edit_distance_and_natural_order() {
        assert_eq!(levenshtein(b"kitten", b"sitting"), 3);
        assert_eq!(levenshtein(b"", b"abc"), 3);
        assert_eq!(strnatcmp(b"2", b"10"), Ordering::Less);
        assert_eq!(strnatcasecmp(b"A2", b"a2"), Ordering::Equal);
        assert_eq!(strnatcmp(b"2 B", b"2-"), Ordering::Less);
    }

    #[test]
    fn php_trim_and_truthiness() {
        assert_eq!(trim(b"\0 \tfoo\r\n\x0b"), b"foo");
        assert_eq!(trim(b"\x0cfoo\x0c"), b"\x0cfoo\x0c");
        assert!(!truthy(b"0"));
        assert!(!truthy(b""));
        assert!(truthy(b"00"));
        assert!(!crate::Value::Int(0).is_truthy());
        assert!(crate::Value::Int(1).is_truthy());
        assert_eq!(crate::Key::Int(-1).bytes().as_ref(), b"-1");
        assert_eq!(crate::Key::from("name").bytes().as_ref(), b"name");
    }
}

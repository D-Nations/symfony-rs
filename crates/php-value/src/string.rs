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

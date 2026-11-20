//! Port of the symfony/finder component. See README.md for the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

pub mod comparator;
mod date;
pub mod exception;
mod gitignore;
mod glob;
pub mod pcre;
pub mod php;

pub use comparator::{Comparator, DateComparator, NumberComparator, Operator};
pub use exception::Error;
pub use gitignore::Gitignore;
pub use glob::Glob;

/// One value or a list, as Finder's `string|array` parameters take them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args(pub Vec<Vec<u8>>);

impl From<&str> for Args {
    fn from(s: &str) -> Self {
        Self(vec![s.as_bytes().to_vec()])
    }
}

impl From<String> for Args {
    fn from(s: String) -> Self {
        Self(vec![s.into_bytes()])
    }
}

impl From<&String> for Args {
    fn from(s: &String) -> Self {
        Self(vec![s.as_bytes().to_vec()])
    }
}

impl From<&[u8]> for Args {
    fn from(s: &[u8]) -> Self {
        Self(vec![s.to_vec()])
    }
}

impl From<i64> for Args {
    fn from(n: i64) -> Self {
        Self(vec![n.to_string().into_bytes()])
    }
}

impl From<i32> for Args {
    fn from(n: i32) -> Self {
        Self(vec![n.to_string().into_bytes()])
    }
}

impl<const N: usize> From<[&str; N]> for Args {
    fn from(a: [&str; N]) -> Self {
        Self(a.iter().map(|s| s.as_bytes().to_vec()).collect())
    }
}

impl From<&[&str]> for Args {
    fn from(a: &[&str]) -> Self {
        Self(a.iter().map(|s| s.as_bytes().to_vec()).collect())
    }
}

impl From<Vec<&str>> for Args {
    fn from(a: Vec<&str>) -> Self {
        Self(a.iter().map(|s| s.as_bytes().to_vec()).collect())
    }
}

impl From<Vec<String>> for Args {
    fn from(a: Vec<String>) -> Self {
        Self(a.into_iter().map(String::into_bytes).collect())
    }
}

impl From<Vec<Vec<u8>>> for Args {
    fn from(a: Vec<Vec<u8>>) -> Self {
        Self(a)
    }
}

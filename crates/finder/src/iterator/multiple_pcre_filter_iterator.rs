//! Port of Symfony\Component\Finder\Iterator\MultiplePcreFilterIterator.php:
//! the patterns and their matching, shared by the filename, content and
//! path filters.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/MultiplePcreFilterIterator.php

use std::sync::LazyLock;

use crate::{
    exception::Error,
    pcre::{Pcre, PcreError},
};

/// `matchRegexps` and `noMatchRegexps`, compiled.
pub struct MultiplePcreFilter {
    match_regexps: Vec<Pcre>,
    no_match_regexps: Vec<Pcre>,
}

static REGEX_SHAPE: LazyLock<Option<Pcre>> =
    LazyLock::new(|| Pcre::new(b"/^(.{3,}?)[imsxuADUn]*$/").ok());
static BAD_DELIMITER: LazyLock<Option<Pcre>> =
    LazyLock::new(|| Pcre::new(br"/[*?[:alnum:] \\]/").ok());

fn compiles(s: &[u8]) -> Result<bool, Error> {
    match Pcre::new(s) {
        Ok(_) => Ok(true),
        Err(PcreError::Compile(_)) => Ok(false),
        Err(e @ PcreError::Unsupported(_)) => Err(e.into()),
    }
}

fn internal(e: impl std::fmt::Display) -> Error {
    Error::Runtime(e.to_string())
}

impl MultiplePcreFilter {
    /// The constructor: each pattern through `to_regex`.
    pub fn new(
        match_patterns: &[Vec<u8>],
        no_match_patterns: &[Vec<u8>],
        to_regex: impl Fn(&[u8]) -> Result<Vec<u8>, Error>,
    ) -> Result<Self, Error> {
        let compile = |p: &Vec<u8>| -> Result<Pcre, Error> { Ok(Pcre::new(&to_regex(p)?)?) };
        Ok(Self {
            match_regexps: match_patterns
                .iter()
                .map(compile)
                .collect::<Result<_, _>>()?,
            no_match_regexps: no_match_patterns
                .iter()
                .map(compile)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn has_patterns(&self) -> bool {
        !self.match_regexps.is_empty() || !self.no_match_regexps.is_empty()
    }

    /// `isAccepted($string)`: no exclusion matches, and some inclusion
    /// matches when there are any. A match error (`false`) is no match.
    pub fn is_accepted(&self, string: &[u8]) -> bool {
        // should at least not match one rule to exclude
        for regex in &self.no_match_regexps {
            if regex.is_match(string).unwrap_or(false) {
                return false;
            }
        }
        // should at least match one rule
        if !self.match_regexps.is_empty() {
            return self
                .match_regexps
                .iter()
                .any(|regex| regex.is_match(string).unwrap_or(false));
        }
        // If there is no match rules, the file is accepted
        true
    }

    /// `isRegex($str)`: a delimited pattern that compiles. A pattern PHP
    /// would compile but this port cannot translate is an error.
    pub fn is_regex(s: &[u8]) -> Result<bool, Error> {
        let shape = REGEX_SHAPE
            .as_ref()
            .ok_or_else(|| internal("isRegex pattern"))?;
        let Some(groups) = shape.captures_from(s, 0).map_err(internal)? else {
            return Ok(false);
        };
        let Some((start, end)) = groups.get(1).copied().flatten() else {
            return Ok(false);
        };
        let m = &s[start..end];
        let (first, last) = (&m[..1], &m[m.len() - 1..]);
        if first == last {
            let bad = BAD_DELIMITER
                .as_ref()
                .ok_or_else(|| internal("isRegex pattern"))?;
            return Ok(!bad.is_match(first).map_err(internal)? && compiles(s)?);
        }
        for (open, close) in [(b"{", b"}"), (b"(", b")"), (b"[", b"]"), (b"<", b">")] {
            if first == open && last == close {
                return compiles(s);
            }
        }
        Ok(false)
    }
}

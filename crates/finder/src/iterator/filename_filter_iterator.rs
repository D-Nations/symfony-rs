//! Port of Symfony\Component\Finder\Iterator\FilenameFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/FilenameFilterIterator.php

use super::{MultiplePcreFilter, next_accepted};
use crate::{exception::Error, glob::Glob, spl_file_info::FileInfo};

/// Filters files by name: a regexp, a glob, or a string.
pub struct FilenameFilterIterator<I> {
    iterator: I,
    filter: MultiplePcreFilter,
}

impl<I> FilenameFilterIterator<I> {
    pub fn new(
        iterator: I,
        match_patterns: &[Vec<u8>],
        no_match_patterns: &[Vec<u8>],
    ) -> Result<Self, Error> {
        Ok(Self {
            iterator,
            filter: MultiplePcreFilter::new(match_patterns, no_match_patterns, Self::to_regex)?,
        })
    }

    /// `toRegex($str)`: PCRE patterns are left unchanged, globs go
    /// through `Glob::toRegex()`.
    pub fn to_regex(s: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(if MultiplePcreFilter::is_regex(s)? {
            s.to_vec()
        } else {
            Glob::to_regex(s, true, true, Some(b'#'))
        })
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for FilenameFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let filter = &self.filter;
        next_accepted(&mut self.iterator, |f: &F| {
            Ok(filter.is_accepted(f.get_filename()))
        })
    }
}

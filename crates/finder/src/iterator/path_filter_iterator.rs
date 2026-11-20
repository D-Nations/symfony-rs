//! Port of Symfony\Component\Finder\Iterator\PathFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! Author: Wlodzimierz Gajda.
//!
//! Upstream: symfony/finder Iterator/PathFilterIterator.php

use super::{MultiplePcreFilter, next_accepted};
use crate::{exception::Error, pcre::preg_quote, spl_file_info::FileInfo};

/// Filters files by relative path: a regexp or a dirname (`/` separated).
pub struct PathFilterIterator<I> {
    iterator: I,
    filter: MultiplePcreFilter,
}

impl<I> PathFilterIterator<I> {
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

    /// `toRegex($str)`: `lorem/ipsum/dolor` becomes `/lorem\/ipsum\/dolor/`.
    pub fn to_regex(s: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(if MultiplePcreFilter::is_regex(s)? {
            s.to_vec()
        } else {
            let mut r = b"/".to_vec();
            r.extend(preg_quote(s, Some(b'/')));
            r.push(b'/');
            r
        })
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for PathFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let filter = &self.filter;
        next_accepted(&mut self.iterator, |f: &F| {
            Ok(filter.is_accepted(f.get_relative_pathname()))
        })
    }
}

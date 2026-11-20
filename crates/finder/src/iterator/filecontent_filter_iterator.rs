//! Port of Symfony\Component\Finder\Iterator\FilecontentFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! Author: Wlodzimierz Gajda.
//!
//! Upstream: symfony/finder Iterator/FilecontentFilterIterator.php

use super::{MultiplePcreFilter, next_accepted};
use crate::{exception::Error, pcre::preg_quote, spl_file_info::FileInfo};

/// Filters files by their contents: regexps or strings.
pub struct FilecontentFilterIterator<I> {
    iterator: I,
    filter: MultiplePcreFilter,
}

impl<I> FilecontentFilterIterator<I> {
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

    /// `toRegex($str)`: a string becomes `/<quoted>/`.
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

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for FilecontentFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let filter = &self.filter;
        next_accepted(&mut self.iterator, |f: &F| {
            if !filter.has_patterns() {
                return Ok(true);
            }
            if f.is_dir() || !f.is_readable() {
                return Ok(false);
            }
            let content = f.get_contents()?.unwrap_or_default();
            // `!$content`: an empty string or "0"
            if content.is_empty() || content == b"0" {
                return Ok(false);
            }
            Ok(filter.is_accepted(&content))
        })
    }
}

//! Port of Symfony\Component\Finder\Iterator\SizeRangeFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/SizeRangeFilterIterator.php

use super::next_accepted;
use crate::{comparator::NumberComparator, exception::Error, spl_file_info::FileInfo};

/// Keeps directories, and files whose size passes every comparator.
pub struct SizeRangeFilterIterator<I> {
    iterator: I,
    comparators: Vec<NumberComparator>,
}

impl<I> SizeRangeFilterIterator<I> {
    pub fn new(iterator: I, comparators: Vec<NumberComparator>) -> Self {
        Self {
            iterator,
            comparators,
        }
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for SizeRangeFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let comparators = &self.comparators;
        next_accepted(&mut self.iterator, |f: &F| {
            if !f.is_file() {
                return Ok(true);
            }
            let size = f.get_size()?;
            Ok(comparators.iter().all(|c| c.test(size)))
        })
    }
}

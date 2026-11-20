//! Port of Symfony\Component\Finder\Iterator\DateRangeFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/DateRangeFilterIterator.php

use super::next_accepted;
use crate::{comparator::DateComparator, exception::Error, spl_file_info::FileInfo};

/// Keeps files whose last modification passes every comparator.
pub struct DateRangeFilterIterator<I> {
    iterator: I,
    comparators: Vec<DateComparator>,
}

impl<I> DateRangeFilterIterator<I> {
    pub fn new(iterator: I, comparators: Vec<DateComparator>) -> Self {
        Self {
            iterator,
            comparators,
        }
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for DateRangeFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let comparators = &self.comparators;
        next_accepted(&mut self.iterator, |f: &F| {
            if !f.exists() {
                return Ok(false);
            }
            let date = f.get_m_time()?;
            Ok(comparators.iter().all(|c| c.test(date)))
        })
    }
}

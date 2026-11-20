//! Port of Symfony\Component\Finder\Iterator\CustomFilterIterator.php.
//!
//! PHP checks at run time that each filter is callable (`Invalid PHP
//! callback.`); here a filter is a closure by type.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/CustomFilterIterator.php

use std::sync::Arc;

use super::next_accepted;
use crate::exception::Error;

/// A filter callback: `false` removes the file.
pub type Filter<F> = Arc<dyn Fn(&F) -> bool + Send + Sync>;

pub struct CustomFilterIterator<I, F> {
    iterator: I,
    filters: Vec<Filter<F>>,
}

impl<I, F> CustomFilterIterator<I, F> {
    pub fn new(iterator: I, filters: Vec<Filter<F>>) -> Self {
        Self { iterator, filters }
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F> Iterator for CustomFilterIterator<I, F> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let filters = &self.filters;
        next_accepted(&mut self.iterator, |f| {
            Ok(filters.iter().all(|filter| filter(f)))
        })
    }
}

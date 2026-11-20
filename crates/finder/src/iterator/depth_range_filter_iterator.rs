//! Port of Symfony\Component\Finder\Iterator\DepthRangeFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/DepthRangeFilterIterator.php

use super::{RecursiveFileIterator, RecursiveIteratorIterator};
use crate::{exception::Error, spl_file_info::SplFileInfo};

/// Limits the directory depth: `$minDepth` filters, `$maxDepth` stops the
/// descent (`i64::MAX` is `PHP_INT_MAX`, no limit).
pub struct DepthRangeFilterIterator<R> {
    iterator: RecursiveIteratorIterator<R>,
    min_depth: i64,
}

impl<R: RecursiveFileIterator> DepthRangeFilterIterator<R> {
    pub fn new(
        mut iterator: RecursiveIteratorIterator<R>,
        min_depth: i64,
        max_depth: i64,
    ) -> Result<Self, Error> {
        // setMaxDepth(-1) is no limit; below that SPL throws
        let max = if max_depth == i64::MAX { -1 } else { max_depth };
        if max < -1 {
            return Err(Error::OutOfRange(
                "RecursiveIteratorIterator::setMaxDepth(): Argument #1 ($maxDepth) must be greater than or equal to -1".into(),
            ));
        }
        iterator.set_max_depth(usize::try_from(max).ok());
        Ok(Self {
            iterator,
            min_depth,
        })
    }
}

impl<R: RecursiveFileIterator> Iterator for DepthRangeFilterIterator<R> {
    type Item = Result<SplFileInfo, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iterator.next()? {
                Ok((depth, f)) => {
                    if i64::try_from(depth).unwrap_or(i64::MAX) >= self.min_depth {
                        return Some(Ok(f));
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }
}

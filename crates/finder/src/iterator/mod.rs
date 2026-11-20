//! Finder's iterators. PHP's SPL iterator protocol (`FilterIterator`,
//! `RecursiveIterator`, `RecursiveIteratorIterator`) becomes Rust
//! iterators of `Result<file, Error>`: an error is what PHP throws while
//! iterating. Filters are lazy, so callbacks and file system calls run in
//! the order PHP runs them.
//!
//! Upstream: none

mod custom_filter_iterator;
mod date_range_filter_iterator;
mod depth_range_filter_iterator;
mod exclude_directory_filter_iterator;
mod file_type_filter_iterator;
mod filecontent_filter_iterator;
mod filename_filter_iterator;
mod lazy_iterator;
mod multiple_pcre_filter_iterator;
mod path_filter_iterator;
mod recursive_directory_iterator;
mod size_range_filter_iterator;
mod sortable_iterator;
mod vcs_ignored_filter_iterator;

pub use custom_filter_iterator::{CustomFilterIterator, Filter};
pub use date_range_filter_iterator::DateRangeFilterIterator;
pub use depth_range_filter_iterator::DepthRangeFilterIterator;
pub use exclude_directory_filter_iterator::{ExcludeDirectoryFilterIterator, Exclusion};
pub use file_type_filter_iterator::{FileTypeFilterIterator, Mode};
pub use filecontent_filter_iterator::FilecontentFilterIterator;
pub use filename_filter_iterator::FilenameFilterIterator;
pub use lazy_iterator::LazyIterator;
pub use multiple_pcre_filter_iterator::MultiplePcreFilter;
pub use path_filter_iterator::PathFilterIterator;
pub use recursive_directory_iterator::{Flags, RecursiveDirectoryIterator};
pub use size_range_filter_iterator::SizeRangeFilterIterator;
pub use sortable_iterator::{Sort, SortableIterator};
pub use vcs_ignored_filter_iterator::VcsIgnoredFilterIterator;

use crate::{exception::Error, spl_file_info::SplFileInfo};

/// A PHP `RecursiveIterator` over files: its own entries, and whether and
/// how to descend into one.
pub trait RecursiveFileIterator: Sized {
    fn next_file(&mut self) -> Option<Result<SplFileInfo, Error>>;
    fn has_children(&self, file: &SplFileInfo) -> bool;
    fn get_children(&self, file: &SplFileInfo) -> Result<Self, Error>;
}

/// `new \RecursiveIteratorIterator($it, SELF_FIRST)`: each entry, then
/// its children; yields `(depth, file)`. `max_depth` is
/// `setMaxDepth()` (None for -1).
pub struct RecursiveIteratorIterator<R> {
    stack: Vec<R>,
    pending: Option<SplFileInfo>,
    max_depth: Option<usize>,
    failed: bool,
}

impl<R: RecursiveFileIterator> RecursiveIteratorIterator<R> {
    pub fn new(root: R) -> Self {
        Self {
            stack: vec![root],
            pending: None,
            max_depth: None,
            failed: false,
        }
    }

    pub fn set_max_depth(&mut self, max_depth: Option<usize>) {
        self.max_depth = max_depth;
    }
}

impl<R: RecursiveFileIterator> Iterator for RecursiveIteratorIterator<R> {
    type Item = Result<(usize, SplFileInfo), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        if let Some(dir) = self.pending.take() {
            let children = self.stack.last()?.get_children(&dir);
            match children {
                Ok(c) => self.stack.push(c),
                Err(e) => {
                    self.failed = true;
                    return Some(Err(e));
                }
            }
        }
        loop {
            let top = self.stack.last_mut()?;
            match top.next_file() {
                None => {
                    self.stack.pop();
                    if self.stack.is_empty() {
                        return None;
                    }
                }
                Some(Err(e)) => {
                    self.failed = true;
                    return Some(Err(e));
                }
                Some(Ok(file)) => {
                    let depth = self.stack.len() - 1;
                    let top = self.stack.last()?;
                    if self.max_depth.is_none_or(|m| depth < m) && top.has_children(&file) {
                        self.pending = Some(file.clone());
                    }
                    return Some(Ok((depth, file)));
                }
            }
        }
    }
}

/// The files of a `(depth, file)` iteration.
pub fn files<I>(iterator: I) -> impl Iterator<Item = Result<SplFileInfo, Error>>
where
    I: Iterator<Item = Result<(usize, SplFileInfo), Error>>,
{
    iterator.map(|r| r.map(|(_, f)| f))
}

/// The next item `accept` keeps, as `FilterIterator::next()` finds it.
pub(crate) fn next_accepted<F, I>(
    inner: &mut I,
    mut accept: impl FnMut(&F) -> Result<bool, Error>,
) -> Option<Result<F, Error>>
where
    I: Iterator<Item = Result<F, Error>>,
{
    loop {
        match inner.next()? {
            Ok(f) => match accept(&f) {
                Ok(true) => return Some(Ok(f)),
                Ok(false) => {}
                Err(e) => return Some(Err(e)),
            },
            Err(e) => return Some(Err(e)),
        }
    }
}

//! Port of Symfony\Component\Finder\Iterator\FileTypeFilterIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/FileTypeFilterIterator.php

use super::next_accepted;
use crate::{exception::Error, spl_file_info::FileInfo};

/// `ONLY_FILES` (1) or `ONLY_DIRECTORIES` (2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    OnlyFiles = 1,
    OnlyDirectories = 2,
}

/// Keeps files, directories, or both.
pub struct FileTypeFilterIterator<I> {
    iterator: I,
    mode: Mode,
}

impl<I> FileTypeFilterIterator<I> {
    pub fn new(iterator: I, mode: Mode) -> Self {
        Self { iterator, mode }
    }
}

impl<I: Iterator<Item = Result<F, Error>>, F: FileInfo> Iterator for FileTypeFilterIterator<I> {
    type Item = Result<F, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let mode = self.mode;
        next_accepted(&mut self.iterator, |f: &F| {
            if (mode == Mode::OnlyDirectories && f.is_file())
                || (mode == Mode::OnlyFiles && f.is_dir())
            {
                return Ok(false);
            }
            Ok(!f.is_link() || f.is_file() || f.is_dir())
        })
    }
}

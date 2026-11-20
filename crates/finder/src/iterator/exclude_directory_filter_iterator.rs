//! Port of Symfony\Component\Finder\Iterator\ExcludeDirectoryFilterIterator.php.
//!
//! Finder always gives it a recursive iterator, which is the only shape
//! ported (`$isRecursive` is true).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/ExcludeDirectoryFilterIterator.php

use std::{collections::HashSet, sync::Arc};

use super::{RecursiveFileIterator, custom_filter_iterator::Filter};
use crate::{
    exception::Error,
    pcre::{Pcre, preg_quote},
    spl_file_info::SplFileInfo,
};

/// A directory to exclude, or a prune callback.
#[derive(Clone)]
pub enum Exclusion {
    Directory(Vec<u8>),
    Prune(Filter<SplFileInfo>),
}

pub struct ExcludeDirectoryFilterIterator<R> {
    iterator: R,
    excluded_dirs: Arc<HashSet<Vec<u8>>>,
    excluded_pattern: Option<Arc<Pcre>>,
    prune_filters: Arc<Vec<Filter<SplFileInfo>>>,
}

impl<R: RecursiveFileIterator> ExcludeDirectoryFilterIterator<R> {
    pub fn new(iterator: R, directories: Vec<Exclusion>) -> Result<Self, Error> {
        let mut patterns: Vec<Vec<u8>> = Vec::new();
        let mut excluded_dirs = HashSet::new();
        let mut prune_filters = Vec::new();
        for directory in directories {
            match directory {
                Exclusion::Prune(f) => prune_filters.push(f),
                Exclusion::Directory(d) => {
                    let mut end = d.len();
                    while end > 0 && d[end - 1] == b'/' {
                        end -= 1;
                    }
                    let d = d[..end].to_vec();
                    if d.contains(&b'/') {
                        patterns.push(preg_quote(&d, Some(b'#')));
                    } else {
                        excluded_dirs.insert(d);
                    }
                }
            }
        }
        let excluded_pattern = if patterns.is_empty() {
            None
        } else {
            let mut p = b"#(?:^|/)(?:".to_vec();
            p.extend(patterns.join(&b'|'));
            p.extend_from_slice(b")(?:/|$)#");
            Some(Arc::new(Pcre::new(&p)?))
        };
        Ok(Self {
            iterator,
            excluded_dirs: Arc::new(excluded_dirs),
            excluded_pattern,
            prune_filters: Arc::new(prune_filters),
        })
    }

    fn accept(&self, file: &SplFileInfo) -> Result<bool, Error> {
        if self.excluded_dirs.contains(file.get_filename()) && file.is_dir() {
            return Ok(false);
        }
        if let Some(pattern) = &self.excluded_pattern {
            let path = if file.is_dir() {
                file.get_relative_pathname()
            } else {
                file.get_relative_path()
            };
            let path: Vec<u8> = path
                .iter()
                .map(|&c| if c == b'\\' { b'/' } else { c })
                .collect();
            if pattern
                .is_match(&path)
                .map_err(|e| Error::Runtime(e.to_string()))?
            {
                return Ok(false);
            }
        }
        if !self.prune_filters.is_empty() && self.iterator.has_children(file) {
            for prune in self.prune_filters.iter() {
                if !prune(file) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

impl<R: RecursiveFileIterator> RecursiveFileIterator for ExcludeDirectoryFilterIterator<R> {
    fn next_file(&mut self) -> Option<Result<SplFileInfo, Error>> {
        loop {
            match self.iterator.next_file()? {
                Ok(f) => match self.accept(&f) {
                    Ok(true) => return Some(Ok(f)),
                    Ok(false) => {}
                    Err(e) => return Some(Err(e)),
                },
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn has_children(&self, file: &SplFileInfo) -> bool {
        self.iterator.has_children(file)
    }

    fn get_children(&self, file: &SplFileInfo) -> Result<Self, Error> {
        Ok(Self {
            iterator: self.iterator.get_children(file)?,
            excluded_dirs: Arc::clone(&self.excluded_dirs),
            excluded_pattern: self.excluded_pattern.clone(),
            prune_filters: Arc::clone(&self.prune_filters),
        })
    }
}

//! Port of Symfony\Component\Finder\Iterator\RecursiveDirectoryIterator.php,
//! with the parts of PHP's `\RecursiveDirectoryIterator` (`SKIP_DOTS`,
//! `FOLLOW_SYMLINKS`) it builds on. A directory is read when the iterator
//! is created, as PHP's constructor opens it. Paths always use `/`
//! (`UNIX_PATHS` changes nothing: Linux behaviour is canonical).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/RecursiveDirectoryIterator.php

use std::{collections::HashSet, collections::VecDeque, sync::Arc};

use super::RecursiveFileIterator;
use crate::{exception::Error, fs, spl_file_info::SplFileInfo};

/// The `\FilesystemIterator` flags Finder passes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub follow_symlinks: bool,
    pub unix_paths: bool,
}

pub struct RecursiveDirectoryIterator {
    ignore_unreadable_dirs: bool,
    real_paths: Arc<HashSet<Vec<u8>>>,
    root_path: Vec<u8>,
    sub_path: Vec<u8>,
    dir_path: Vec<u8>,
    flags: Flags,
    names: VecDeque<Vec<u8>>,
}

fn open_error(path: &[u8], reason: &str) -> Error {
    Error::UnexpectedValue(format!(
        "RecursiveDirectoryIterator::__construct({}): Failed to open directory: {reason}",
        String::from_utf8_lossy(path)
    ))
}

impl RecursiveDirectoryIterator {
    /// `new RecursiveDirectoryIterator($path, $flags, $ignoreUnreadableDirs)`
    /// (with `SKIP_DOTS`).
    pub fn new(path: &[u8], flags: Flags, ignore_unreadable_dirs: bool) -> Result<Self, Error> {
        let mut real_paths = HashSet::new();
        if flags.follow_symlinks
            && let Some(real) = fs::realpath(path)
        {
            real_paths.insert(real);
        }
        Self::open(
            path,
            path.to_vec(),
            Vec::new(),
            flags,
            ignore_unreadable_dirs,
            Arc::new(real_paths),
        )
    }

    fn open(
        dir_path: &[u8],
        root_path: Vec<u8>,
        sub_path: Vec<u8>,
        flags: Flags,
        ignore_unreadable_dirs: bool,
        real_paths: Arc<HashSet<Vec<u8>>>,
    ) -> Result<Self, Error> {
        if dir_path.is_empty() {
            return Err(Error::UnexpectedValue(
                "RecursiveDirectoryIterator::__construct(): Argument #1 ($directory) cannot be empty".into(),
            ));
        }
        let names = fs::read_dir(dir_path).map_err(|reason| open_error(dir_path, &reason))?;
        Ok(Self {
            ignore_unreadable_dirs,
            real_paths,
            root_path,
            sub_path,
            dir_path: dir_path.to_vec(),
            flags,
            names: names.into(),
        })
    }

    fn entry_path(&self, name: &[u8]) -> Vec<u8> {
        let mut path = self.dir_path.clone();
        while path.len() > 1 && path.ends_with(b"/") && !path.ends_with(b"://") {
            path.pop();
        }
        if !path.ends_with(b"/") {
            path.push(b'/');
        }
        path.extend_from_slice(name);
        path
    }

    /// `current()`: a file with its path relative to the root.
    fn current(&self, name: &[u8]) -> SplFileInfo {
        let mut sub_pathname = self.sub_path.clone();
        if !sub_pathname.is_empty() {
            sub_pathname.push(b'/');
        }
        sub_pathname.extend_from_slice(name);
        let mut base = self.root_path.clone();
        if base != b"/" && !base.ends_with(b"/") {
            base.push(b'/');
        }
        base.extend_from_slice(&sub_pathname);
        SplFileInfo::new(base, self.sub_path.clone(), sub_pathname)
    }

    fn children(&self, file: &SplFileInfo) -> Result<Self, Error> {
        let path = self.entry_path(file.get_filename());
        let mut real_paths = Arc::clone(&self.real_paths);
        if self.flags.follow_symlinks
            && let Some(real) = fs::realpath(&path)
        {
            let mut set = (*real_paths).clone();
            set.insert(real);
            real_paths = Arc::new(set);
        }
        Self::open(
            &path,
            self.root_path.clone(),
            file.get_relative_pathname().to_vec(),
            self.flags,
            self.ignore_unreadable_dirs,
            real_paths,
        )
    }
}

impl RecursiveFileIterator for RecursiveDirectoryIterator {
    fn next_file(&mut self) -> Option<Result<SplFileInfo, Error>> {
        let name = self.names.pop_front()?;
        Some(Ok(self.current(&name)))
    }

    fn has_children(&self, file: &SplFileInfo) -> bool {
        // \RecursiveDirectoryIterator::hasChildren(): a directory, and not
        // a link unless links are followed
        let has_children = file.is_dir() && (self.flags.follow_symlinks || !file.is_link());
        // don't loop on a directory that resolves to one of its parents
        if has_children
            && !self.real_paths.is_empty()
            && let Some(real) = fs::realpath(&self.entry_path(file.get_filename()))
            && self.real_paths.contains(&real)
        {
            return false;
        }
        if !has_children || !self.ignore_unreadable_dirs {
            return has_children;
        }
        // If directory is unreadable and finder is set to ignore it, skip children
        self.children(file).is_ok()
    }

    fn get_children(&self, file: &SplFileInfo) -> Result<Self, Error> {
        self.children(file).map_err(|e| match e {
            Error::UnexpectedValue(m) => Error::AccessDenied(m),
            e => e,
        })
    }
}

impl Iterator for RecursiveDirectoryIterator {
    type Item = Result<SplFileInfo, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_file()
    }
}

//! Port of Symfony\Component\Finder\Finder.php.
//!
//! Finder allows to build rules to find files and directories. Methods
//! that PHP lets throw (`depth()`, `size()`, `date()`, `in()`) return a
//! `Result`; the others return `&mut Self` for chaining, as PHP returns
//! `$this`. An appended Finder is taken by value: PHP keeps a reference,
//! so a change made to it after `append()` is seen there and not here.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Finder.php

use std::{
    cmp::Ordering,
    sync::{Arc, LazyLock, RwLock},
};

use crate::{
    Args,
    comparator::{DateComparator, NumberComparator, Operator},
    exception::Error,
    fs,
    iterator::{
        CustomFilterIterator, DateRangeFilterIterator, DepthRangeFilterIterator,
        ExcludeDirectoryFilterIterator, Exclusion, FileTypeFilterIterator,
        FilecontentFilterIterator, FilenameFilterIterator, Filter, Flags, Mode, PathFilterIterator,
        RecursiveDirectoryIterator, RecursiveFileIterator, RecursiveIteratorIterator,
        SizeRangeFilterIterator, Sort, SortableIterator, VcsIgnoredFilterIterator, files,
    },
    spl_file_info::SplFileInfo,
};

pub const IGNORE_VCS_FILES: u8 = 1;
pub const IGNORE_DOT_FILES: u8 = 2;
pub const IGNORE_VCS_IGNORED_FILES: u8 = 4;

static VCS_PATTERNS: LazyLock<RwLock<Vec<Vec<u8>>>> = LazyLock::new(|| {
    RwLock::new(
        [
            ".svn",
            "_svn",
            "CVS",
            "_darcs",
            ".arch-params",
            ".monotone",
            ".bzr",
            ".git",
            ".hg",
        ]
        .iter()
        .map(|s| s.as_bytes().to_vec())
        .collect(),
    )
});

/// The files Finder yields; each one's key is its pathname.
pub type FinderIterator = Box<dyn Iterator<Item = Result<SplFileInfo, Error>> + Send>;

/// What `append()` takes: a Finder, paths, or files.
#[derive(Clone)]
pub enum Appended {
    Finder(Box<Finder>),
    /// Strings: `new \SplFileInfo($path)`.
    Paths(Vec<Vec<u8>>),
    /// Files kept as they are.
    Files(Vec<SplFileInfo>),
}

impl From<Finder> for Appended {
    fn from(f: Finder) -> Self {
        Self::Finder(Box::new(f))
    }
}

impl From<Vec<SplFileInfo>> for Appended {
    fn from(f: Vec<SplFileInfo>) -> Self {
        Self::Files(f)
    }
}

impl From<Args> for Appended {
    fn from(a: Args) -> Self {
        Self::Paths(a.0)
    }
}

impl From<Vec<String>> for Appended {
    fn from(a: Vec<String>) -> Self {
        Self::Paths(a.into_iter().map(String::into_bytes).collect())
    }
}

impl From<Vec<&str>> for Appended {
    fn from(a: Vec<&str>) -> Self {
        Self::Paths(a.iter().map(|s| s.as_bytes().to_vec()).collect())
    }
}

#[derive(Clone)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's Finder flags")]
pub struct Finder {
    mode: Option<Mode>,
    names: Vec<Vec<u8>>,
    not_names: Vec<Vec<u8>>,
    exclude: Vec<Vec<u8>>,
    filters: Vec<Filter<SplFileInfo>>,
    prune_filters: Vec<Filter<SplFileInfo>>,
    depths: Vec<NumberComparator>,
    sizes: Vec<NumberComparator>,
    follow_links: bool,
    unix_paths: bool,
    reverse_sorting: bool,
    sort: Option<Sort>,
    ignore: u8,
    dirs: Vec<Vec<u8>>,
    dates: Vec<DateComparator>,
    iterators: Vec<Appended>,
    contains: Vec<Vec<u8>>,
    not_contains: Vec<Vec<u8>>,
    paths: Vec<Vec<u8>>,
    not_paths: Vec<Vec<u8>>,
    ignore_unreadable_dirs: bool,
}

impl Default for Finder {
    fn default() -> Self {
        Self::new()
    }
}

/// `$target + $n` on a comparator's numeric target, as an int (a float
/// is truncated when PHP passes it to an `int` parameter).
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "PHP's float addition and (int) truncation"
)]
fn target_plus(target: &[u8], n: i64) -> i64 {
    let integer_form = !target.iter().any(|&c| matches!(c, b'.' | b'e' | b'E'));
    if integer_form {
        php_value::numeric::string_to_int(target).saturating_add(n)
    } else {
        let f = php_value::numeric::string_to_float(target) + n as f64;
        f as i64
    }
}

impl Finder {
    pub fn new() -> Self {
        Self {
            mode: None,
            names: Vec::new(),
            not_names: Vec::new(),
            exclude: Vec::new(),
            filters: Vec::new(),
            prune_filters: Vec::new(),
            depths: Vec::new(),
            sizes: Vec::new(),
            follow_links: false,
            unix_paths: false,
            reverse_sorting: false,
            sort: None,
            ignore: IGNORE_VCS_FILES | IGNORE_DOT_FILES,
            dirs: Vec::new(),
            dates: Vec::new(),
            iterators: Vec::new(),
            contains: Vec::new(),
            not_contains: Vec::new(),
            paths: Vec::new(),
            not_paths: Vec::new(),
            ignore_unreadable_dirs: false,
        }
    }

    /// `Finder::create()`.
    pub fn create() -> Self {
        Self::new()
    }

    /// Restricts the matching to directories only.
    pub fn directories(&mut self) -> &mut Self {
        self.mode = Some(Mode::OnlyDirectories);
        self
    }

    /// Restricts the matching to files only.
    pub fn files(&mut self) -> &mut Self {
        self.mode = Some(Mode::OnlyFiles);
        self
    }

    /// Adds tests for the directory depth (`'> 1'`, `'< 3'`, `0`, ...).
    pub fn depth(&mut self, levels: impl Into<Args>) -> Result<&mut Self, Error> {
        for level in levels.into().0 {
            self.depths.push(NumberComparator::new(Some(&level))?);
        }
        Ok(self)
    }

    /// Adds tests for file dates (last modified).
    pub fn date(&mut self, dates: impl Into<Args>) -> Result<&mut Self, Error> {
        for date in dates.into().0 {
            self.dates.push(DateComparator::new(&date)?);
        }
        Ok(self)
    }

    /// Adds rules that files must match: patterns, globs or strings.
    pub fn name(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.names.extend(patterns.into().0);
        self
    }

    /// Adds rules that files must not match.
    pub fn not_name(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.not_names.extend(patterns.into().0);
        self
    }

    /// Adds tests that file contents must match.
    pub fn contains(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.contains.extend(patterns.into().0);
        self
    }

    /// Adds tests that file contents must not match.
    pub fn not_contains(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.not_contains.extend(patterns.into().0);
        self
    }

    /// Adds rules that relative paths must match (`/` separated).
    pub fn path(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.paths.extend(patterns.into().0);
        self
    }

    /// Adds rules that relative paths must not match.
    pub fn not_path(&mut self, patterns: impl Into<Args>) -> &mut Self {
        self.not_paths.extend(patterns.into().0);
        self
    }

    /// Adds tests for file sizes (`'> 10K'`, `'<= 1Ki'`, `4`).
    pub fn size(&mut self, sizes: impl Into<Args>) -> Result<&mut Self, Error> {
        for size in sizes.into().0 {
            self.sizes.push(NumberComparator::new(Some(&size))?);
        }
        Ok(self)
    }

    /// Excludes directories, relative to the ones given to `in()`.
    pub fn exclude(&mut self, dirs: impl Into<Args>) -> &mut Self {
        self.exclude.extend(dirs.into().0);
        self
    }

    fn set_ignore(&mut self, flag: u8, on: bool) -> &mut Self {
        if on {
            self.ignore |= flag;
        } else {
            self.ignore &= !flag;
        }
        self
    }

    /// Excludes "hidden" directories and files (starting with a dot).
    pub fn ignore_dot_files(&mut self, ignore_dot_files: bool) -> &mut Self {
        self.set_ignore(IGNORE_DOT_FILES, ignore_dot_files)
    }

    /// Forces the finder to ignore version control directories.
    pub fn ignore_vcs(&mut self, ignore_vcs: bool) -> &mut Self {
        self.set_ignore(IGNORE_VCS_FILES, ignore_vcs)
    }

    /// Forces Finder to obey .gitignore and ignore files based on rules
    /// listed there.
    pub fn ignore_vcs_ignored(&mut self, ignore_vcs_ignored: bool) -> &mut Self {
        self.set_ignore(IGNORE_VCS_IGNORED_FILES, ignore_vcs_ignored)
    }

    /// `Finder::addVCSPattern()` (process-wide, as PHP's static).
    pub fn add_vcs_pattern(pattern: impl Into<Args>) {
        let mut patterns = VCS_PATTERNS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for p in pattern.into().0 {
            if !patterns.contains(&p) {
                patterns.push(p);
            }
        }
    }

    /// Sorts files and directories by an anonymous function.
    pub fn sort(
        &mut self,
        closure: impl Fn(&SplFileInfo, &SplFileInfo) -> Ordering + Send + Sync + 'static,
    ) -> &mut Self {
        self.sort = Some(Sort::Callback(Arc::new(closure)));
        self
    }

    pub fn sort_by_extension(&mut self) -> &mut Self {
        self.sort = Some(Sort::Extension);
        self
    }

    pub fn sort_by_name(&mut self, use_natural_sort: bool) -> &mut Self {
        self.sort = Some(if use_natural_sort {
            Sort::NameNatural
        } else {
            Sort::Name
        });
        self
    }

    pub fn sort_by_case_insensitive_name(&mut self, use_natural_sort: bool) -> &mut Self {
        self.sort = Some(if use_natural_sort {
            Sort::NameNaturalCaseInsensitive
        } else {
            Sort::NameCaseInsensitive
        });
        self
    }

    pub fn sort_by_size(&mut self) -> &mut Self {
        self.sort = Some(Sort::Size);
        self
    }

    pub fn sort_by_type(&mut self) -> &mut Self {
        self.sort = Some(Sort::Type);
        self
    }

    pub fn sort_by_accessed_time(&mut self) -> &mut Self {
        self.sort = Some(Sort::AccessedTime);
        self
    }

    /// Reverses the sorting.
    pub fn reverse_sorting(&mut self) -> &mut Self {
        self.reverse_sorting = true;
        self
    }

    pub fn sort_by_changed_time(&mut self) -> &mut Self {
        self.sort = Some(Sort::ChangedTime);
        self
    }

    pub fn sort_by_modified_time(&mut self) -> &mut Self {
        self.sort = Some(Sort::ModifiedTime);
        self
    }

    /// Filters with an anonymous function: `false` removes the file;
    /// with `prune`, a directory it removes is not searched.
    pub fn filter(
        &mut self,
        closure: impl Fn(&SplFileInfo) -> bool + Send + Sync + 'static,
        prune: bool,
    ) -> &mut Self {
        let closure: Filter<SplFileInfo> = Arc::new(closure);
        if prune {
            self.prune_filters.push(Arc::clone(&closure));
        }
        self.filters.push(closure);
        self
    }

    /// Forces the following of symlinks.
    pub fn follow_links(&mut self) -> &mut Self {
        self.follow_links = true;
        self
    }

    /// Forces forward slashes in returned paths (always the case here).
    pub fn use_unix_paths(&mut self) -> &mut Self {
        self.unix_paths = true;
        self
    }

    /// Tells finder to ignore unreadable directories.
    pub fn ignore_unreadable_dirs(&mut self, ignore: bool) -> &mut Self {
        self.ignore_unreadable_dirs = ignore;
        self
    }

    /// Searches files and directories which match defined rules.
    pub fn r#in(&mut self, dirs: impl Into<Args>) -> Result<&mut Self, Error> {
        let mut resolved = Vec::new();
        for dir in dirs.into().0 {
            if fs::is_dir(&dir) {
                resolved.push(Self::normalize_dir(&dir));
            } else {
                let mut glob = fs::glob_dirs(&dir);
                if glob.is_empty() {
                    return Err(Error::DirectoryNotFound(format!(
                        "The \"{}\" directory does not exist.",
                        String::from_utf8_lossy(&dir)
                    )));
                }
                glob.sort();
                resolved.extend(glob.iter().map(|d| Self::normalize_dir(d)));
            }
        }
        self.dirs.extend(resolved);
        Ok(self)
    }

    /// Returns an Iterator for the current Finder configuration.
    pub fn get_iterator(&self) -> Result<FinderIterator, Error> {
        if self.dirs.is_empty() && self.iterators.is_empty() {
            return Err(Error::Logic(
                "You must call one of in() or append() methods before iterating over a Finder."
                    .into(),
            ));
        }
        let iterator: FinderIterator = if self.dirs.len() == 1 && self.iterators.is_empty() {
            self.search_in_directory(&self.dirs[0])?
        } else {
            let mut parts: Vec<FinderIterator> = Vec::new();
            for dir in &self.dirs {
                let this = self.clone();
                let dir = dir.clone();
                parts.push(Box::new(Lazy::new(move || this.search_in_directory(&dir))));
            }
            for it in &self.iterators {
                let it = it.clone();
                parts.push(Box::new(Lazy::new(
                    move || -> Result<FinderIterator, Error> {
                        Ok(match it {
                            Appended::Finder(f) => f.get_iterator()?,
                            Appended::Files(files) => Box::new(files.into_iter().map(Ok)),
                            Appended::Paths(paths) => {
                                Box::new(paths.into_iter().map(|p| Ok(SplFileInfo::from_path(p))))
                            }
                        })
                    },
                )));
            }
            Box::new(parts.into_iter().flatten())
        };
        if self.sort.is_some() || self.reverse_sorting {
            let sort = self.sort.clone().unwrap_or(Sort::None);
            return Ok(SortableIterator::new(iterator, sort, self.reverse_sorting).get_iterator());
        }
        Ok(iterator)
    }

    /// Appends an existing set of files/directories to the finder.
    pub fn append(&mut self, iterator: impl Into<Appended>) -> &mut Self {
        self.iterators.push(iterator.into());
        self
    }

    /// Check if any results were found.
    pub fn has_results(&self) -> Result<bool, Error> {
        match self.get_iterator()?.next() {
            Some(Err(e)) => Err(e),
            Some(Ok(_)) => Ok(true),
            None => Ok(false),
        }
    }

    /// Counts all the results collected by the iterators.
    pub fn count(&self) -> Result<usize, Error> {
        let mut n = 0;
        for item in self.get_iterator()? {
            item?;
            n += 1;
        }
        Ok(n)
    }

    fn search_in_directory(&self, dir: &[u8]) -> Result<FinderIterator, Error> {
        let mut exclude: Vec<Exclusion> = self
            .exclude
            .iter()
            .map(|d| Exclusion::Directory(d.clone()))
            .collect();
        let mut not_paths = self.not_paths.clone();
        exclude.extend(self.prune_filters.iter().cloned().map(Exclusion::Prune));
        if self.ignore & IGNORE_VCS_FILES != 0 {
            let patterns = VCS_PATTERNS
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            exclude.extend(patterns.iter().cloned().map(Exclusion::Directory));
        }
        if self.ignore & IGNORE_DOT_FILES != 0 {
            not_paths.push(br"#(^|/)\..+(/|$)#".to_vec());
        }

        let mut min_depth: i64 = 0;
        let mut max_depth: i64 = i64::MAX;
        for comparator in &self.depths {
            let target = comparator.get_target();
            match comparator.operator() {
                Operator::Greater => min_depth = target_plus(target, 1),
                Operator::GreaterOrEqual => min_depth = target_plus(target, 0),
                Operator::Less => max_depth = target_plus(target, -1),
                Operator::LessOrEqual => max_depth = target_plus(target, 0),
                _ => {
                    min_depth = target_plus(target, 0);
                    max_depth = min_depth;
                }
            }
        }

        let flags = Flags {
            follow_symlinks: self.follow_links,
            unix_paths: self.unix_paths,
        };
        let iterator = RecursiveDirectoryIterator::new(dir, flags, self.ignore_unreadable_dirs)?;
        let mut iterator: FinderIterator = if exclude.is_empty() {
            Self::depth_filtered(iterator, min_depth, max_depth)?
        } else {
            Self::depth_filtered(
                ExcludeDirectoryFilterIterator::new(iterator, exclude)?,
                min_depth,
                max_depth,
            )?
        };

        if let Some(mode) = self.mode {
            iterator = Box::new(FileTypeFilterIterator::new(iterator, mode));
        }
        if !self.names.is_empty() || !self.not_names.is_empty() {
            iterator = Box::new(FilenameFilterIterator::new(
                iterator,
                &self.names,
                &self.not_names,
            )?);
        }
        if !self.contains.is_empty() || !self.not_contains.is_empty() {
            iterator = Box::new(FilecontentFilterIterator::new(
                iterator,
                &self.contains,
                &self.not_contains,
            )?);
        }
        if !self.sizes.is_empty() {
            iterator = Box::new(SizeRangeFilterIterator::new(iterator, self.sizes.clone()));
        }
        if !self.dates.is_empty() {
            iterator = Box::new(DateRangeFilterIterator::new(iterator, self.dates.clone()));
        }
        if !self.filters.is_empty() {
            iterator = Box::new(CustomFilterIterator::new(iterator, self.filters.clone()));
        }
        if !self.paths.is_empty() || !not_paths.is_empty() {
            iterator = Box::new(PathFilterIterator::new(iterator, &self.paths, &not_paths)?);
        }
        if self.ignore & IGNORE_VCS_IGNORED_FILES != 0 {
            iterator = Box::new(VcsIgnoredFilterIterator::new(iterator, dir)?);
        }
        Ok(iterator)
    }

    fn depth_filtered<R>(iterator: R, min: i64, max: i64) -> Result<FinderIterator, Error>
    where
        R: RecursiveFileIterator + Send + 'static,
    {
        let iterator = RecursiveIteratorIterator::new(iterator);
        Ok(if min > 0 || max < i64::MAX {
            Box::new(DepthRangeFilterIterator::new(iterator, min, max)?)
        } else {
            Box::new(files(iterator))
        })
    }

    /// Normalizes given directory names by removing trailing slashes,
    /// except for stream wrapper schemes such as ftp:// or s3://.
    pub fn normalize_dir(dir: &[u8]) -> Vec<u8> {
        if dir == b"/" {
            return dir.to_vec();
        }
        let mut end = dir.len();
        while end > 0 && dir[end - 1] == b'/' {
            end -= 1;
        }
        let mut dir = dir[..end].to_vec();
        if fs::scheme(&dir).is_some() {
            dir.push(b'/');
        }
        dir
    }
}

/// `new \IteratorIterator(new LazyIterator(...))` inside the
/// `\AppendIterator`: built when reached; a failure to build it is thrown
/// then.
struct Lazy<F> {
    factory: Option<F>,
    iterator: Option<FinderIterator>,
}

impl<F> Lazy<F> {
    fn new(factory: F) -> Self {
        Self {
            factory: Some(factory),
            iterator: None,
        }
    }
}

impl<F: FnOnce() -> Result<FinderIterator, Error>> Iterator for Lazy<F> {
    type Item = Result<SplFileInfo, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(factory) = self.factory.take() {
            match factory() {
                Ok(it) => self.iterator = Some(it),
                Err(e) => return Some(Err(e)),
            }
        }
        self.iterator.as_mut()?.next()
    }
}

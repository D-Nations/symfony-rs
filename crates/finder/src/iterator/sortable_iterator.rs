//! Port of Symfony\Component\Finder\Iterator\SortableIterator.php.
//!
//! PHP 8's sort is stable, as Rust's is. The values a comparison reads
//! (real paths, times, sizes) are read once per file before sorting; a
//! file whose time or size cannot be read throws, as `getATime()` and
//! the others do.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Iterator/SortableIterator.php

use std::{cmp::Ordering, fmt, sync::Arc};

use crate::{
    exception::Error,
    php::{strcasecmp, strcmp, strnatcasecmp, strnatcmp},
    spl_file_info::SplFileInfo,
};

/// A comparison callback.
pub type Compare = Arc<dyn Fn(&SplFileInfo, &SplFileInfo) -> Ordering + Send + Sync>;

/// The sort types (`SORT_BY_*`) or a callback.
#[derive(Clone)]
pub enum Sort {
    None,
    Name,
    Type,
    AccessedTime,
    ChangedTime,
    ModifiedTime,
    NameNatural,
    NameCaseInsensitive,
    NameNaturalCaseInsensitive,
    Extension,
    Size,
    Callback(Compare),
}

impl fmt::Debug for Sort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::None => "None",
            Self::Name => "Name",
            Self::Type => "Type",
            Self::AccessedTime => "AccessedTime",
            Self::ChangedTime => "ChangedTime",
            Self::ModifiedTime => "ModifiedTime",
            Self::NameNatural => "NameNatural",
            Self::NameCaseInsensitive => "NameCaseInsensitive",
            Self::NameNaturalCaseInsensitive => "NameNaturalCaseInsensitive",
            Self::Extension => "Extension",
            Self::Size => "Size",
            Self::Callback(_) => "Callback",
        })
    }
}

impl Sort {
    /// The `SORT_BY_*` constant's value, or the constructor's exception.
    pub fn from_int(sort: i64) -> Result<Self, Error> {
        Ok(match sort {
            0 => Self::None,
            1 => Self::Name,
            2 => Self::Type,
            3 => Self::AccessedTime,
            4 => Self::ChangedTime,
            5 => Self::ModifiedTime,
            6 => Self::NameNatural,
            7 => Self::NameCaseInsensitive,
            8 => Self::NameNaturalCaseInsensitive,
            9 => Self::Extension,
            10 => Self::Size,
            _ => {
                return Err(Error::InvalidArgument(
                    "The SortableIterator takes a PHP callable or a valid built-in sort algorithm as an argument.".into(),
                ));
            }
        })
    }
}

/// Sorts files from another iteration.
pub struct SortableIterator<I> {
    iterator: I,
    sort: Sort,
    reverse_order: bool,
}

/// What a comparison reads from a file.
enum Key {
    Path(Vec<u8>),
    Int(i64),
    None,
}

impl<I: Iterator<Item = Result<SplFileInfo, Error>>> SortableIterator<I> {
    pub fn new(iterator: I, sort: Sort, reverse_order: bool) -> Self {
        Self {
            iterator,
            sort,
            reverse_order,
        }
    }

    /// `getIterator()`.
    pub fn get_iterator(self) -> Box<dyn Iterator<Item = Result<SplFileInfo, Error>> + Send>
    where
        I: Send + 'static,
    {
        if matches!(self.sort, Sort::None) && !self.reverse_order {
            return Box::new(self.iterator);
        }
        let mut values = Vec::new();
        for item in self.iterator {
            match item {
                Ok(v) => values.push(v),
                // the exception leaves getIterator() before anything is yielded
                Err(e) => return Box::new(std::iter::once(Err(e))),
            }
        }
        if matches!(self.sort, Sort::None) {
            values.reverse();
            return Box::new(values.into_iter().map(Ok));
        }
        let order = |o: Ordering| if self.reverse_order { o.reverse() } else { o };
        let path = |f: &SplFileInfo| {
            f.get_real_path()
                .unwrap_or_else(|| f.get_pathname().to_vec())
        };
        let keys: Result<Vec<Key>, Error> = values
            .iter()
            .map(|f| {
                Ok(match &self.sort {
                    Sort::Name
                    | Sort::NameNatural
                    | Sort::NameCaseInsensitive
                    | Sort::NameNaturalCaseInsensitive
                    | Sort::Type => Key::Path(path(f)),
                    Sort::AccessedTime => Key::Int(f.get_a_time()?),
                    Sort::ChangedTime => Key::Int(f.get_c_time()?),
                    Sort::ModifiedTime => Key::Int(f.get_m_time()?),
                    Sort::Size => Key::Int(f.get_size()?),
                    _ => Key::None,
                })
            })
            .collect();
        let keys = match keys {
            Ok(k) => k,
            Err(e) => return Box::new(std::iter::once(Err(e))),
        };
        let mut indexed: Vec<(usize, SplFileInfo)> = values.into_iter().enumerate().collect();
        let sort = self.sort.clone();
        indexed.sort_by(|(i, a), (j, b)| {
            let (ka, kb) = (&keys[*i], &keys[*j]);
            let paths = || match (ka, kb) {
                (Key::Path(x), Key::Path(y)) => (x.as_slice(), y.as_slice()),
                _ => (&b""[..], &b""[..]),
            };
            match &sort {
                Sort::Name => order(strcmp(paths().0, paths().1)),
                Sort::NameNatural => order(strnatcmp(paths().0, paths().1)),
                Sort::NameCaseInsensitive => order(strcasecmp(paths().0, paths().1)),
                Sort::NameNaturalCaseInsensitive => order(strnatcasecmp(paths().0, paths().1)),
                Sort::Type => {
                    if a.is_dir() && b.is_file() {
                        order(Ordering::Less)
                    } else if a.is_file() && b.is_dir() {
                        order(Ordering::Greater)
                    } else {
                        order(strcmp(paths().0, paths().1))
                    }
                }
                Sort::AccessedTime | Sort::ChangedTime | Sort::ModifiedTime | Sort::Size => {
                    match (ka, kb) {
                        (Key::Int(x), Key::Int(y)) => order(x.cmp(y)),
                        _ => Ordering::Equal,
                    }
                }
                Sort::Extension => order(strnatcmp(a.get_extension(), b.get_extension())),
                Sort::Callback(f) => order(f(a, b)),
                Sort::None => Ordering::Equal,
            }
        });
        Box::new(indexed.into_iter().map(|(_, f)| Ok(f)))
    }
}

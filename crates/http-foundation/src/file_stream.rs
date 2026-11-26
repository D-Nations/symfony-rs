//! Port of Symfony\Component\HttpFoundation\File\Stream.php: a PHP stream
//! of unknown size, a `File` whose `getSize()` is `false`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation File/Stream.php

use crate::{
    exception::Result,
    file::{File, FileLike},
};

/// `new Stream($path, $checkPath)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream(File);

impl Stream {
    /// `new Stream($path, $checkPath)`, as `new File()`.
    pub fn new(path: impl Into<Vec<u8>>, check_path: bool) -> Result<Self> {
        File::new(path, check_path).map(Self)
    }
}

impl FileLike for Stream {
    fn get_pathname(&self) -> &[u8] {
        self.0.get_pathname()
    }

    /// `getSize()`: `false`.
    fn get_size(&self) -> Result<Option<i64>> {
        Ok(None)
    }
}

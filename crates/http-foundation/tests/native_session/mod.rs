//! What the native session tests' `setUp()` and `tearDown()` do: a save
//! path of the test's own (`sys_get_temp_dir().'/sftest'`, made unique
//! here since tests run at once), `session.save_handler` `files`; then the
//! session written and closed, and its files removed.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/NativeSessionStorageTest.php
#![allow(dead_code)]

use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use php_value::path::from_path;
use symfony_http_foundation::php_session::{ini_set, session_write_close};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A save path removed when dropped.
pub struct SavePath {
    pub dir: PathBuf,
}

impl SavePath {
    /// `setUp()`.
    pub fn set_up() -> Self {
        let dir = Self::unique("sftest");
        let _ = std::fs::create_dir_all(&dir);
        ini_set("save_handler", "files");
        ini_set("save_path", from_path(&dir));
        Self { dir }
    }

    /// A directory under the system's temporary one that no other test
    /// uses (not created).
    pub fn unique(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "symfony-rs-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// The path as PHP shows it.
    pub fn bytes(&self) -> Vec<u8> {
        from_path(&self.dir)
    }
}

impl Drop for SavePath {
    /// `tearDown()`.
    fn drop(&mut self) {
        let _ = session_write_close();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

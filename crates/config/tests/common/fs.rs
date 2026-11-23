//! Filesystem helpers for the loader and resource tests: a private copy
//! of upstream's Tests/Fixtures (upstream's tests write into that
//! directory; each Rust test gets its own copy so they can run in
//! parallel), `touch()` with a time, and `time()`.
//!
//! Paths are bytes with `/` separators, as symfony-finder's `fs` gives
//! them (its `realpath()` on Windows included).
//!
//! Upstream: none

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use symfony_finder::fs;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory, removed on drop.
pub struct TempDir {
    path: PathBuf,
    real: Vec<u8>,
}

impl TempDir {
    /// An empty directory.
    pub fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "symfony-config-test-{}-{n}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create a temporary directory");
        let real = fs::realpath(path.to_string_lossy().as_bytes()).expect("realpath");
        Self { path, real }
    }

    /// A copy of tests/fixtures (upstream's Tests/Fixtures).
    pub fn fixtures() -> Self {
        let dir = Self::new();
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
            &dir.path,
        );
        dir
    }

    /// A copy of tests/fixtures at `sub` inside this directory.
    pub fn copy_fixtures_to(&self, sub: &str) {
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
            &self.path.join(sub),
        );
    }

    /// The real path, `/`-separated.
    pub fn path(&self) -> Vec<u8> {
        self.real.clone()
    }

    /// `path() . $suffix`.
    pub fn join(&self, suffix: &str) -> Vec<u8> {
        let mut p = self.real.clone();
        p.extend_from_slice(suffix.as_bytes());
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create a fixture directory");
    for entry in std::fs::read_dir(from).expect("read the fixtures") {
        let entry = entry.expect("a fixture entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy a fixture");
        }
    }
}

/// A local path from `/`-separated bytes.
pub fn os(path: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(path).into_owned())
}

/// `time()`.
pub fn time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

/// `touch($path, $time)`: creates the file if needed and sets its
/// modification time (now when `time` is None). Directories too.
pub fn touch(path: &[u8], time: Option<i64>) {
    let p = os(path);
    if !p.exists() {
        std::fs::File::create(&p).expect("create a file");
    }
    let when = match time {
        Some(t) if t >= 0 => UNIX_EPOCH + Duration::from_secs(t.unsigned_abs()),
        Some(t) => UNIX_EPOCH - Duration::from_secs(t.unsigned_abs()),
        None => SystemTime::now(),
    };
    let file = open_for_times(&p);
    file.set_modified(when).expect("set the modification time");
}

#[cfg(windows)]
fn open_for_times(p: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    // FILE_FLAG_BACKUP_SEMANTICS opens directories too.
    std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(0x0200_0000)
        .open(p)
        .expect("open for set_modified")
}

#[cfg(not(windows))]
fn open_for_times(p: &Path) -> std::fs::File {
    std::fs::File::open(p).expect("open for set_modified")
}

/// `unlink($path)`.
pub fn unlink(path: &[u8]) {
    std::fs::remove_file(os(path)).expect("unlink");
}

/// `mkdir($path)`.
pub fn mkdir(path: &[u8]) {
    std::fs::create_dir(os(path)).expect("mkdir");
}

/// `rmdir($path)`.
pub fn rmdir(path: &[u8]) {
    std::fs::remove_dir(os(path)).expect("rmdir");
}

/// Removes a directory and its contents.
pub fn remove_directory(path: &[u8]) {
    std::fs::remove_dir_all(os(path)).expect("remove a directory");
}

/// Bytes as text, for assertion messages.
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

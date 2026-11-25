//! FilesystemTest.php at symfony/filesystem v8.1.9: the tests of the
//! ported subset (`dumpFile()`, `readFile()`, `chmod()`, `tempnam()`).
//! Names and assertions are PHPUnit's. The workspace is a private
//! temporary directory (`FilesystemTestCase::setUp()`). Rust cannot set the
//! process umask safely, so testDumpFile expects `0666 & ~umask()` for
//! the umask the tests run with, where upstream sets it to 0o002 first.
//! Mode tests are skipped on Windows, as upstream's are.
//!
//! Upstream: symfony/filesystem Tests/FilesystemTest.php Tests/FilesystemTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use symfony_filesystem::{Class, Error, Filesystem, php};

type TestResult = Result<(), Box<dyn std::error::Error>>;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Workspace {
    path: PathBuf,
    real: Vec<u8>,
}

impl Workspace {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "symfony-filesystem-test-{}-{n}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create the workspace");
        let real =
            symfony_finder::fs::realpath(path.to_string_lossy().as_bytes()).expect("realpath");
        Self { path, real }
    }

    fn join(&self, suffix: &str) -> Vec<u8> {
        let mut p = self.real.clone();
        p.extend_from_slice(suffix.as_bytes());
        p
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn os(path: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(path).into_owned())
}

fn contents(path: &[u8]) -> Vec<u8> {
    std::fs::read(os(path)).expect("read a file")
}

#[cfg(unix)]
fn perms(path: &[u8]) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(os(path))
        .expect("stat")
        .permissions()
        .mode()
        & 0o777
}

#[track_caller]
fn expect_io<T: std::fmt::Debug>(result: Result<T, Error>, pattern: &str) {
    match result {
        Ok(v) => panic!("expected an IOException, got Ok({v:?})"),
        Err(e) => {
            assert_eq!(e.class(), Class::Io, "{e}");
            let message = String::from_utf8_lossy(e.message()).into_owned();
            let re = regex_lite(pattern);
            assert!(re(&message), "{message:?} does not match {pattern:?}");
        }
    }
}

/// A `^...$` pattern with `.+` wildcards only, enough for upstream's
/// `expectExceptionMessageMatches()` here.
fn regex_lite(pattern: &str) -> impl Fn(&str) -> bool {
    let parts: Vec<String> = pattern.split(".+").map(str::to_owned).collect();
    move |text: &str| {
        let mut rest = text;
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                let Some(r) = rest.strip_prefix(part.as_str()) else {
                    return false;
                };
                rest = r;
            } else if i == parts.len() - 1 {
                return rest.len() > part.len() && rest.ends_with(part.as_str());
            } else {
                let Some(at) = rest.get(1..).and_then(|r| r.find(part.as_str())) else {
                    return false;
                };
                rest = &rest[1 + at + part.len()..];
            }
        }
        parts.len() == 1 && rest.is_empty()
    }
}

#[test]
fn testDumpFile() -> TestResult {
    let ws = Workspace::new();
    let filename = ws.join("/foo/baz.txt");
    Filesystem::new().dump_file(&filename, b"bar")?;
    assert!(os(&filename).exists());
    assert_eq!(contents(&filename), b"bar");
    #[cfg(unix)]
    assert_eq!(perms(&filename), 0o666 & !php::umask());
    Ok(())
}

#[test]
fn testDumpFileOverwritesAnExistingFile() -> TestResult {
    let ws = Workspace::new();
    let filename = ws.join("/foo.txt");
    std::fs::write(os(&filename), "FOO BAR")?;
    Filesystem::new().dump_file(&filename, b"bar")?;
    assert!(os(&filename).exists());
    assert_eq!(contents(&filename), b"bar");
    Ok(())
}

#[cfg(unix)]
#[test]
fn testDumpFileFollowsSymlink() -> TestResult {
    let ws = Workspace::new();
    let filesystem = Filesystem::new();
    let filename = ws.join("/foo.txt");
    std::fs::write(os(&filename), "FOO BAR")?;
    let link_a = ws.join("/bar.txt");
    let link_b = ws.join("/baz.txt");
    std::os::unix::fs::symlink(os(&filename), os(&link_a))?;
    std::os::unix::fs::symlink(os(&link_a), os(&link_b))?;

    filesystem.dump_file(&link_b, b"bar")?;

    for path in [&filename, &link_a, &link_b] {
        assert!(os(path).exists());
        assert_eq!(contents(path), b"bar");
    }

    std::fs::remove_file(os(&filename))?;
    filesystem.dump_file(&link_b, b"baz")?;

    assert!(os(&filename).exists());
    for path in [&filename, &link_a, &link_b] {
        assert_eq!(contents(path), b"baz");
    }
    Ok(())
}

#[test]
fn testDumpFileWithFileScheme() -> TestResult {
    let ws = Workspace::new();
    let mut filename = b"file://".to_vec();
    filename.extend_from_slice(&ws.join("/foo/baz.txt"));
    Filesystem::new().dump_file(&filename, b"bar")?;
    assert!(symfony_finder::fs::file_exists(&filename));
    assert_eq!(contents(&ws.join("/foo/baz.txt")), b"bar");
    Ok(())
}

#[cfg(unix)]
#[test]
fn testDumpFileCleansUpAfterFailure() -> TestResult {
    let ws = Workspace::new();
    let filesystem = Filesystem::new();
    let target = ws.join("/dump-file");
    std::fs::write(os(&target), "")?;
    filesystem.chmod(&[&target], 0o444, 0)?;
    let _ = filesystem.dump_file(&target, b"any content");
    filesystem.chmod(&[&target], 0o666, 0)?;

    let entries: Vec<_> = std::fs::read_dir(&ws.path)?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<Result<_, _>>()?;
    assert_eq!(entries, vec![std::ffi::OsString::from("dump-file")]);
    Ok(())
}

#[test]
fn testReadFile() -> TestResult {
    let file = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let read = Filesystem::new().read_file(file.as_bytes())?;
    assert_eq!(read, std::fs::read(&file)?);
    Ok(())
}

#[test]
fn testReadNonExistentFile() {
    let file = format!("{}/tests/invalid", env!("CARGO_MANIFEST_DIR"));
    expect_io(
        Filesystem::new().read_file(file.as_bytes()),
        "Failed to read file \".+tests/invalid\": file_get_contents(.+tests/invalid): Failed to open stream: No such file or directory",
    );
}

#[test]
fn testReadDirectory() {
    let dir = format!("{}/tests", env!("CARGO_MANIFEST_DIR"));
    expect_io(
        Filesystem::new().read_file(dir.as_bytes()),
        "Failed to read file \".+tests\": File is a directory.",
    );
}

#[cfg(unix)]
#[test]
fn testChmodChangesFileMode() -> TestResult {
    let ws = Workspace::new();
    let dir = ws.join("/dir");
    std::fs::create_dir(os(&dir))?;
    let file = ws.join("/dir/file");
    std::fs::write(os(&file), "")?;
    let filesystem = Filesystem::new();
    filesystem.chmod(&[&file], 0o400, 0)?;
    filesystem.chmod(&[&dir], 0o753, 0)?;
    assert_eq!(perms(&dir), 0o753);
    assert_eq!(perms(&file), 0o400);
    Ok(())
}

#[cfg(unix)]
#[test]
fn testChmodAppliesUmask() -> TestResult {
    let ws = Workspace::new();
    let file = ws.join("/file");
    std::fs::write(os(&file), "")?;
    Filesystem::new().chmod(&[&file], 0o770, 0o022)?;
    assert_eq!(perms(&file), 0o750);
    Ok(())
}

#[cfg(unix)]
#[test]
fn testChmodChangesModeOfArrayOfFiles() -> TestResult {
    let ws = Workspace::new();
    let directory = ws.join("/directory");
    let file = ws.join("/file");
    std::fs::create_dir(os(&directory))?;
    std::fs::write(os(&file), "")?;
    Filesystem::new().chmod(&[&directory, &file], 0o753, 0)?;
    assert_eq!(perms(&file), 0o753);
    assert_eq!(perms(&directory), 0o753);
    Ok(())
}

#[test]
fn testTempnam() -> TestResult {
    let ws = Workspace::new();
    let filename = Filesystem::new().tempnam(&ws.real, b"foo")?;
    assert!(os(&filename).exists());
    Ok(())
}

#[test]
fn testTempnamTrimsTrailingWhitespaceFromTruncatedPrefix() -> TestResult {
    let ws = Workspace::new();
    let prefix = format!("{} suffix", "a".repeat(62));
    let filename = Filesystem::new().tempnam(&ws.real, prefix.as_bytes())?;
    assert!(os(&filename).exists());
    let base = symfony_filesystem::filesystem::basename(&filename);
    assert!(!base.contains(&b' '));
    Ok(())
}

#[test]
fn testTempnamWithFileScheme() -> TestResult {
    let ws = Workspace::new();
    let mut dirname = b"file://".to_vec();
    dirname.extend_from_slice(&ws.real);
    let filename = Filesystem::new().tempnam(&dirname, b"foo")?;
    assert!(filename.starts_with(b"file://"));
    assert!(symfony_finder::fs::file_exists(&filename));
    Ok(())
}

#[test]
fn testTempnamOnUnwritableFallsBackToSysTmp() -> TestResult {
    let ws = Workspace::new();
    let mut dirname = b"file://".to_vec();
    dirname.extend_from_slice(&ws.join("/does_not_exist"));
    let filename = Filesystem::new().tempnam(&dirname, b"bar")?;
    let real_temp = symfony_finder::fs::realpath(&php::sys_get_temp_dir()).unwrap_or_default();
    let mut prefix = b"file://".to_vec();
    prefix.extend_from_slice(&real_temp);
    while prefix.last() == Some(&b'/') {
        prefix.pop();
    }
    assert!(filename.starts_with(&prefix));
    assert!(symfony_finder::fs::file_exists(&filename));
    php::unlink(&filename);
    Ok(())
}

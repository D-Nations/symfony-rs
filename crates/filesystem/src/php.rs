//! The PHP built-ins the Filesystem subset calls, on local paths and
//! `file://` URLs, written from PHP's documentation and its recorded
//! answers (`oracle/recorders/config_cache.php`); no PHP source code
//! (PLAN.md decision 8):
//!
//! - `umask()` (read only: changing it needs a system call Rust does not
//!   wrap safely);
//! - `tempnam()`: `<dir>/<prefix>` and 19 random characters (13 lowercase
//!   letters or digits, then 6 letters or digits, the form PHP 8.4's
//!   names take), created with mode 0600 and `O_EXCL`; the system's
//!   temporary directory when `dir` is missing or not writable (with
//!   PHP's notice);
//! - `file_get_contents()`, `file_put_contents()`, `chmod()`, `fileperms()`, `rename()` (a copy
//!   and an unlink across file systems, as PHP's plain wrapper does),
//!   `mkdir()` (recursive), `readlink()`, `sys_get_temp_dir()`;
//! - the warning each one gives on failure, which Filesystem appends to
//!   its messages (`self::$lastError`);
//! - `parse_url($url, PHP_URL_SCHEME)`, from Config's recorded corpus
//!   (`crates/config/tests/data/loaders.json`).
//!
//! On Windows, as PHP there, a mode only sets or clears the read-only
//! attribute (from its 0o200 bit), and `umask()` is 0.
//!
//! Upstream: none

use std::{ffi::OsString, io::Write, path::PathBuf};

/// A path as bytes.
#[cfg(unix)]
pub fn os_bytes(s: OsString) -> Vec<u8> {
    use std::os::unix::ffi::OsStringExt;
    s.into_vec()
}

#[cfg(not(unix))]
pub fn os_bytes(s: OsString) -> Vec<u8> {
    s.to_string_lossy().into_owned().into_bytes()
}

/// The local file a path names: the path itself, or the part after
/// `file://`. `None` for other stream wrappers and for `""`.
pub fn local_path(path: &[u8]) -> Option<PathBuf> {
    let path = match symfony_finder::fs::scheme(path) {
        Some(s) if s.eq_ignore_ascii_case(b"file") => &path[7..],
        Some(_) => return None,
        None => path,
    };
    if path.is_empty() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(path)))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(path).ok().map(PathBuf::from)
    }
}

fn shown(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
}

/// The reason PHP's warnings give for a failed system call: the C
/// library's `strerror()` text.
pub fn reason(e: &std::io::Error) -> String {
    let text = e.to_string();
    match text.find(" (os error") {
        Some(at) => text[..at].to_string(),
        None => text,
    }
}

/// `umask()`: the process's file mode creation mask.
pub fn umask() -> u32 {
    #[cfg(target_os = "linux")]
    if let Some(mask) = proc_umask() {
        return mask;
    }
    #[cfg(unix)]
    {
        probe_umask()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// The `Umask:` line of `/proc/self/status` (Linux 4.7 and later).
#[cfg(target_os = "linux")]
fn proc_umask() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find_map(|l| l.strip_prefix("Umask:"))?;
    u32::from_str_radix(line.trim(), 8).ok()
}

/// The mask as the kernel applies it: the mode a file created with 0o777
/// in the temporary directory gets.
#[cfg(unix)]
fn probe_umask() -> u32 {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    for _ in 0..10 {
        let mut name = sys_get_temp_dir();
        name.extend_from_slice(b"/umask");
        name.extend_from_slice(&random_suffix());
        let Some(path) = local_path(&name) else {
            break;
        };
        let opened = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o777)
            .open(&path);
        if let Ok(file) = opened {
            let mode = file.metadata().map(|m| m.permissions().mode() & 0o777);
            drop(file);
            let _ = std::fs::remove_file(&path);
            if let Ok(mode) = mode {
                return 0o777 & !mode;
            }
        }
    }
    0o022
}

/// `sys_get_temp_dir()`: `TMPDIR` without a trailing `/`, else `/tmp`
/// (the system's temporary directory on Windows).
pub fn sys_get_temp_dir() -> Vec<u8> {
    #[cfg(unix)]
    {
        if let Some(dir) = std::env::var_os("TMPDIR").map(os_bytes) {
            let end = dir.iter().rposition(|&c| c != b'/').map_or(0, |i| i + 1);
            if end > 0 {
                return dir[..end].to_vec();
            }
            if !dir.is_empty() {
                return b"/".to_vec();
            }
        }
        b"/tmp".to_vec()
    }
    #[cfg(not(unix))]
    {
        let dir = os_bytes(std::env::temp_dir().into_os_string());
        let dir = String::from_utf8_lossy(&dir).replace('\\', "/");
        dir.trim_end_matches('/').as_bytes().to_vec()
    }
}

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// `tempnam()`'s 19 random characters: 13 from `[a-z0-9]`, then 6 from
/// `[a-zA-Z0-9]`.
fn random_suffix() -> Vec<u8> {
    let mut bytes = [0u8; 19];
    if getrandom::fill(&mut bytes).is_err() {
        // no OS randomness: fall back to the clock and a counter, which
        // O_EXCL keeps safe (a taken name is retried)
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let mixed = now ^ COUNTER.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = mixed.rotate_left(u32::try_from(i * 7).unwrap_or(0)) as u8;
        }
    }
    let lower = &ALPHABET[..26];
    let digits = &ALPHABET[52..];
    let base36: Vec<u8> = lower.iter().chain(digits).copied().collect();
    bytes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let set: &[u8] = if i < 13 { &base36 } else { ALPHABET };
            set[usize::from(*b) % set.len()]
        })
        .collect()
}

/// What a call returned and the last warning it gave (Filesystem's
/// `self::box()` keeps the warning as `$lastError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boxed<T> {
    pub value: T,
    pub warning: Option<String>,
}

/// `tempnam($dir, $prefix)` on a local directory: the created file's
/// path, or `None` (PHP's false) with the warning.
pub fn tempnam(dir: &[u8], prefix: &[u8]) -> Boxed<Option<Vec<u8>>> {
    let prefix: Vec<u8> = {
        let base = prefix
            .iter()
            .rposition(|&c| c == b'/')
            .map_or(prefix, |at| &prefix[at + 1..]);
        base[..base.len().min(63)].to_vec()
    };
    let in_dir = |dir: &[u8]| -> Option<Vec<u8>> {
        let dir = symfony_finder::fs::realpath(dir)?;
        if !symfony_finder::fs::is_dir(&dir) {
            return None;
        }
        for _ in 0..100 {
            let mut name = dir.clone();
            if name.last() != Some(&b'/') {
                name.push(b'/');
            }
            name.extend_from_slice(&prefix);
            name.extend_from_slice(&random_suffix());
            let path = local_path(&name)?;
            match create_private(&path) {
                Ok(()) => return Some(name),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    };
    let local = local_path(dir).map(|p| os_bytes(p.into_os_string()));
    if let Some(name) = local.as_deref().filter(|d| !d.is_empty()).and_then(in_dir) {
        return Boxed {
            value: Some(name),
            warning: None,
        };
    }
    match in_dir(&sys_get_temp_dir()) {
        Some(name) => Boxed {
            value: Some(name),
            warning: Some("tempnam(): file created in the system's temporary directory".into()),
        },
        None => Boxed {
            value: None,
            warning: Some("tempnam(): file created in the system's temporary directory".into()),
        },
    }
}

/// A new, empty file only its owner can read and write (`O_EXCL`).
fn create_private(path: &std::path::Path) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map(drop)
}

/// `file_put_contents($filename, $data)`: the bytes written, or `None`.
pub fn file_put_contents(filename: &[u8], data: &[u8]) -> Boxed<Option<usize>> {
    let failed = |reason: String| Boxed {
        value: None,
        warning: Some(format!(
            "file_put_contents({}): Failed to open stream: {reason}",
            shown(filename)
        )),
    };
    let Some(path) = local_path(filename) else {
        return failed("No such file or directory".into());
    };
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(e) => return failed(reason(&e)),
    };
    match file.write_all(data) {
        Ok(()) => Boxed {
            value: Some(data.len()),
            warning: None,
        },
        Err(_) => Boxed {
            value: None,
            warning: Some(format!(
                "file_put_contents(): Only 0 of {} bytes written, possibly out of free disk space",
                data.len()
            )),
        },
    }
}

/// `file_get_contents($filename)`, or PHP's warning. A path through a
/// file (`a.txt/x`) is "No such file or directory", as PHP reports it
/// (the C library says "Not a directory").
pub fn file_get_contents(filename: &[u8]) -> Result<Vec<u8>, String> {
    let failed = |reason: &str| {
        format!(
            "file_get_contents({}): Failed to open stream: {reason}",
            shown(filename)
        )
    };
    let Some(path) = local_path(filename) else {
        return Err(failed("No such file or directory"));
    };
    std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotADirectory => failed("No such file or directory"),
        _ => failed(&reason(&e)),
    })
}

/// `fileperms($filename)`: `st_mode`, or `None` (PHP's false).
pub fn fileperms(filename: &[u8]) -> Option<u32> {
    symfony_finder::fs::stat(filename).map(|s| s.mode)
}

/// `chmod($filename, $mode)`.
pub fn chmod(filename: &[u8], mode: u32) -> Boxed<bool> {
    let failed = |reason: String| Boxed {
        value: false,
        warning: Some(format!("chmod(): {reason}")),
    };
    let Some(path) = local_path(filename) else {
        return failed("No such file or directory".into());
    };
    match set_mode(&path, mode) {
        Ok(()) => Boxed {
            value: true,
            warning: None,
        },
        Err(e) => failed(reason(&e)),
    }
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o7777))
}

#[cfg(not(unix))]
fn set_mode(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(mode & 0o200 == 0);
    std::fs::set_permissions(path, permissions)
}

/// `rename($from, $to)` on local files; across file systems the file is
/// copied (with its mode) and the original removed.
pub fn rename(from: &[u8], to: &[u8]) -> Boxed<bool> {
    let failed = |reason: String| Boxed {
        value: false,
        warning: Some(format!("rename({},{}): {reason}", shown(from), shown(to))),
    };
    let (Some(source), Some(target)) = (local_path(from), local_path(to)) else {
        return failed("No such file or directory".into());
    };
    match std::fs::rename(&source, &target) {
        Ok(()) => Boxed {
            value: true,
            warning: None,
        },
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let copied = std::fs::copy(&source, &target)
                .and_then(|_| std::fs::metadata(&source))
                .and_then(|m| std::fs::set_permissions(&target, m.permissions()))
                .and_then(|()| std::fs::remove_file(&source));
            match copied {
                Ok(()) => Boxed {
                    value: true,
                    warning: None,
                },
                Err(e) => failed(reason(&e)),
            }
        }
        Err(e) => failed(reason(&e)),
    }
}

/// `mkdir($dir, $mode, true)`.
pub fn mkdir_recursive(dir: &[u8], mode: u32) -> Boxed<bool> {
    let failed = |reason: String| Boxed {
        value: false,
        warning: Some(format!("mkdir(): {reason}")),
    };
    let Some(path) = local_path(dir) else {
        return failed("No such file or directory".into());
    };
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    if symfony_finder::fs::file_exists(dir) {
        return failed("File exists".into());
    }
    match builder.create(&path) {
        Ok(()) => Boxed {
            value: true,
            warning: None,
        },
        Err(e) => failed(reason(&e)),
    }
}

/// `readlink($path)`: the link's target as stored, or `None`.
pub fn readlink(path: &[u8]) -> Option<Vec<u8>> {
    let path = local_path(path)?;
    std::fs::read_link(path)
        .ok()
        .map(|t| os_bytes(t.into_os_string()))
}

/// `unlink($filename)`.
pub fn unlink(filename: &[u8]) -> bool {
    local_path(filename).is_some_and(|p| std::fs::remove_file(p).is_ok())
}

/// `is_writable($filename)`.
pub fn is_writable(filename: &[u8]) -> bool {
    local_path(filename)
        .is_some_and(|p| std::fs::metadata(&p).is_ok_and(|m| !m.permissions().readonly()))
}

/// What `parse_url($url, PHP_URL_SCHEME)` returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlScheme {
    Scheme(Vec<u8>),
    /// `null`: no scheme.
    Null,
    /// `false`: a URL PHP rejects.
    False,
}

/// `parse_url($url, PHP_URL_SCHEME)`: the scheme, or `None` where PHP
/// gives null or false (see [`parse_url_scheme_value`]).
pub fn parse_url_scheme(url: &[u8]) -> Option<Vec<u8>> {
    match parse_url_scheme_value(url) {
        UrlScheme::Scheme(s) => Some(s),
        UrlScheme::Null | UrlScheme::False => None,
    }
}

/// `parse_url($url, PHP_URL_SCHEME)`, telling null (no scheme) from
/// false (a URL it rejects).
///
/// The rules, from PHP's recorded answers (tests/data/loaders.json): the
/// scheme is what precedes the first `:` when that is non-empty and made
/// of letters, digits, `+`, `-` and `.`; but a `:` followed by one to
/// five digits that end the string or reach a `/` reads as a port (no
/// scheme). After `scheme://`, the authority (up to `/`, `?` or `#`) must
/// have a host (after any `user@`) and a port, if any, of digits up to
/// 65535; an empty authority is rejected unless the scheme is `file`
/// and a path follows.
pub fn parse_url_scheme_value(url: &[u8]) -> UrlScheme {
    let Some(colon) = url.iter().position(|&c| c == b':') else {
        return UrlScheme::Null;
    };
    let scheme = &url[..colon];
    if scheme.is_empty()
        || !scheme
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
    {
        return UrlScheme::Null;
    }
    let rest = &url[colon + 1..];
    if rest.first() != Some(&b'/') {
        let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && digits < 6 && (digits == rest.len() || rest[digits] == b'/') {
            return UrlScheme::Null;
        }
        return UrlScheme::Scheme(scheme.to_vec());
    }
    let Some(after) = rest.strip_prefix(b"//") else {
        return UrlScheme::Scheme(scheme.to_vec());
    };
    let end = after
        .iter()
        .position(|&c| matches!(c, b'/' | b'?' | b'#'))
        .unwrap_or(after.len());
    let authority = &after[..end];
    if authority.is_empty() {
        let has_path = after.len() > end && after[end] == b'/';
        return if scheme.eq_ignore_ascii_case(b"file") && has_path {
            UrlScheme::Scheme(scheme.to_vec())
        } else {
            UrlScheme::False
        };
    }
    let host_port = match authority.iter().rposition(|&c| c == b'@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    let (host, port) = if host_port.first() == Some(&b'[') {
        match host_port.iter().position(|&c| c == b']') {
            Some(close) => (
                &host_port[..=close],
                host_port[close + 1..].strip_prefix(b":"),
            ),
            None => (host_port, None),
        }
    } else {
        match host_port.iter().rposition(|&c| c == b':') {
            Some(at) => (&host_port[..at], Some(&host_port[at + 1..])),
            None => (host_port, None),
        }
    };
    if host.is_empty() {
        return UrlScheme::False;
    }
    if let Some(port) = port.filter(|p| !p.is_empty()) {
        let valid = port.len() <= 5
            && port.iter().all(u8::is_ascii_digit)
            && std::str::from_utf8(port)
                .ok()
                .and_then(|p| p.parse::<u32>().ok())
                .is_some_and(|p| p <= 65535);
        if !valid {
            return UrlScheme::False;
        }
    }
    UrlScheme::Scheme(scheme.to_vec())
}

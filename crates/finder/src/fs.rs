//! The filesystem as PHP's stream layer presents it to Finder: local
//! paths, and `scheme://` URLs served by registered stream wrappers
//! (`stream_wrapper_register()`), with `stat()`, `opendir()`/`readdir()`,
//! `realpath()`, `is_readable()`, `file_get_contents()` and `glob()`.
//! Written from PHP's documentation; no PHP source is used.
//!
//! Paths are bytes, as PHP strings are. Linux behaviour is canonical
//! (README, "Rust shape"): the directory separator is `/` on every
//! system, and `realpath()` on Windows answers with `/` separators and no
//! `\\?\` prefix.
//!
//! Upstream: none

use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, LazyLock, RwLock},
};

/// What `url_stat()` / `stat()` report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stat {
    /// `st_mode`: the file type bits decide `is_dir()`, `is_file()` and
    /// `is_link()`.
    pub mode: u32,
    pub size: i64,
    pub atime: i64,
    pub mtime: i64,
    pub ctime: i64,
}

pub const S_IFMT: u32 = 0o170_000;
pub const S_IFDIR: u32 = 0o040_000;
pub const S_IFREG: u32 = 0o100_000;
pub const S_IFLNK: u32 = 0o120_000;

impl Stat {
    pub fn is_dir(&self) -> bool {
        self.mode & S_IFMT == S_IFDIR
    }

    pub fn is_file(&self) -> bool {
        self.mode & S_IFMT == S_IFREG
    }

    pub fn is_link(&self) -> bool {
        self.mode & S_IFMT == S_IFLNK
    }
}

/// A user stream wrapper (PHP's `streamWrapper` prototype), as far as
/// Finder uses one.
pub trait StreamWrapper: Send + Sync {
    /// `dir_opendir()` and the names `dir_readdir()` returns, or None when
    /// the directory cannot be opened.
    fn dir_opendir(&self, url: &[u8]) -> Option<Vec<Vec<u8>>>;
    /// `url_stat()`, or None when the URL does not exist.
    fn url_stat(&self, url: &[u8]) -> Option<Stat>;
    /// `stream_open()` + reads, for `file_get_contents()`.
    fn read(&self, _url: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

type Registry = RwLock<HashMap<Vec<u8>, Arc<dyn StreamWrapper>>>;

static WRAPPERS: LazyLock<Registry> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// `stream_wrapper_register($protocol, ...)`; false when it is taken.
pub fn stream_wrapper_register(protocol: &str, wrapper: Arc<dyn StreamWrapper>) -> bool {
    let mut map = WRAPPERS.write().unwrap_or_else(|e| e.into_inner());
    let key = protocol.as_bytes().to_ascii_lowercase();
    if map.contains_key(&key) || key == b"file" {
        return false;
    }
    map.insert(key, wrapper);
    true
}

/// `stream_wrapper_unregister($protocol)`.
pub fn stream_wrapper_unregister(protocol: &str) -> bool {
    let mut map = WRAPPERS.write().unwrap_or_else(|e| e.into_inner());
    map.remove(&protocol.as_bytes().to_ascii_lowercase())
        .is_some()
}

/// The scheme of a `scheme://` URL (`^[a-zA-Z][a-zA-Z0-9+.-]*://`).
pub fn scheme(path: &[u8]) -> Option<&[u8]> {
    let first = *path.first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let len = path
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'.' | b'-'))
        .count();
    path[len..].starts_with(b"://").then(|| &path[..len])
}

enum Target {
    Local(PathBuf),
    Wrapper(Arc<dyn StreamWrapper>),
    /// A scheme no wrapper serves: every operation fails.
    Unknown,
}

fn target(path: &[u8]) -> Target {
    match scheme(path) {
        Some(s) if s.eq_ignore_ascii_case(b"file") => match local_path(&path[7..]) {
            Some(p) => Target::Local(p),
            None => Target::Unknown,
        },
        Some(s) => {
            let map = WRAPPERS.read().unwrap_or_else(|e| e.into_inner());
            match map.get(&s.to_ascii_lowercase()) {
                Some(w) => Target::Wrapper(Arc::clone(w)),
                None => Target::Unknown,
            }
        }
        None => match local_path(path) {
            Some(p) => Target::Local(p),
            None => Target::Unknown,
        },
    }
}

/// Whether a path is served by a stream wrapper (not a local file).
pub fn is_url(path: &[u8]) -> bool {
    scheme(path).is_some_and(|s| !s.eq_ignore_ascii_case(b"file"))
}

#[cfg(unix)]
fn local_path(path: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(path)))
}

#[cfg(not(unix))]
fn local_path(path: &[u8]) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    std::str::from_utf8(path).ok().map(PathBuf::from)
}

#[cfg(unix)]
fn os_bytes(s: OsString) -> Vec<u8> {
    use std::os::unix::ffi::OsStringExt;
    s.into_vec()
}

#[cfg(not(unix))]
fn os_bytes(s: OsString) -> Vec<u8> {
    s.to_string_lossy().into_owned().into_bytes()
}

#[cfg(not(unix))]
fn secs(t: std::io::Result<std::time::SystemTime>) -> i64 {
    match t {
        Ok(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
        },
        Err(_) => 0,
    }
}

#[cfg(unix)]
fn stat_of(m: &std::fs::Metadata) -> Stat {
    use std::os::unix::fs::MetadataExt;
    Stat {
        mode: m.mode(),
        size: i64::try_from(m.size()).unwrap_or(i64::MAX),
        atime: m.atime(),
        mtime: m.mtime(),
        ctime: m.ctime(),
    }
}

#[cfg(not(unix))]
fn stat_of(m: &std::fs::Metadata) -> Stat {
    let ft = m.file_type();
    let kind = if ft.is_symlink() {
        S_IFLNK
    } else if ft.is_dir() {
        S_IFDIR
    } else {
        S_IFREG
    };
    let perms = if m.permissions().readonly() {
        0o555
    } else {
        0o777
    };
    Stat {
        mode: kind | perms,
        size: i64::try_from(m.len()).unwrap_or(i64::MAX),
        atime: secs(m.accessed()),
        mtime: secs(m.modified()),
        // as PHP on Windows: the creation time
        ctime: secs(m.created()),
    }
}

/// `stat()` (following symbolic links).
pub fn stat(path: &[u8]) -> Option<Stat> {
    match target(path) {
        Target::Local(p) => std::fs::metadata(p).ok().map(|m| stat_of(&m)),
        Target::Wrapper(w) => w.url_stat(path),
        Target::Unknown => None,
    }
}

/// `lstat()`.
pub fn lstat(path: &[u8]) -> Option<Stat> {
    match target(path) {
        Target::Local(p) => std::fs::symlink_metadata(p).ok().map(|m| stat_of(&m)),
        Target::Wrapper(w) => w.url_stat(path),
        Target::Unknown => None,
    }
}

pub fn file_exists(path: &[u8]) -> bool {
    stat(path).is_some()
}

pub fn is_dir(path: &[u8]) -> bool {
    stat(path).is_some_and(|s| s.is_dir())
}

pub fn is_file(path: &[u8]) -> bool {
    stat(path).is_some_and(|s| s.is_file())
}

pub fn is_link(path: &[u8]) -> bool {
    lstat(path).is_some_and(|s| s.is_link())
}

/// `is_readable()`: whether the file or directory can be opened.
pub fn is_readable(path: &[u8]) -> bool {
    match target(path) {
        Target::Local(p) => match std::fs::metadata(&p) {
            Ok(m) if m.is_dir() => std::fs::read_dir(&p).is_ok(),
            Ok(_) => std::fs::File::open(&p).is_ok(),
            Err(_) => false,
        },
        Target::Wrapper(w) => w.url_stat(path).is_some(),
        Target::Unknown => false,
    }
}

/// The reason PHP's warnings give for a failed system call.
fn reason(e: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => "No such file or directory".into(),
        ErrorKind::PermissionDenied => "Permission denied".into(),
        ErrorKind::NotADirectory => "Not a directory".into(),
        ErrorKind::IsADirectory => "Is a directory".into(),
        _ => {
            let text = e.to_string();
            match text.find(" (os error") {
                Some(at) => text[..at].to_string(),
                None => text,
            }
        }
    }
}

/// `opendir()` + `readdir()`: the entry names, without `.` and `..`, in
/// the order the system returns them; the error is PHP's reason.
pub fn read_dir(path: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    match target(path) {
        Target::Local(p) => {
            let entries = std::fs::read_dir(&p).map_err(|e| reason(&e))?;
            let mut names = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|e| reason(&e))?;
                names.push(os_bytes(entry.file_name()));
            }
            Ok(names)
        }
        Target::Wrapper(w) => w
            .dir_opendir(path)
            .map(|names| {
                names
                    .into_iter()
                    .filter(|n| n != b"." && n != b"..")
                    .collect()
            })
            .ok_or_else(|| "operation failed".to_string()),
        Target::Unknown => Err("not implemented".into()),
    }
}

/// `file_get_contents()`, or PHP's warning text.
pub fn read(path: &[u8]) -> Result<Vec<u8>, String> {
    let shown = String::from_utf8_lossy(path).into_owned();
    match target(path) {
        Target::Local(p) => std::fs::read(&p).map_err(|e| {
            format!(
                "file_get_contents({shown}): Failed to open stream: {}",
                reason(&e)
            )
        }),
        Target::Wrapper(w) => w.read(path).ok_or_else(|| {
            format!("file_get_contents({shown}): Failed to open stream: operation failed")
        }),
        Target::Unknown => Err(format!(
            "file_get_contents(): Unable to find the wrapper \"{}\"",
            String::from_utf8_lossy(scheme(path).unwrap_or_default())
        )),
    }
}

/// `realpath()`; None for URLs and missing files.
pub fn realpath(path: &[u8]) -> Option<Vec<u8>> {
    let Target::Local(p) = target(path) else {
        return None;
    };
    let real = std::fs::canonicalize(p).ok()?;
    let bytes = os_bytes(real.into_os_string());
    if cfg!(windows) {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let text = text
            .strip_prefix(r"\\?\UNC\")
            .map(|rest| format!(r"\\{rest}"))
            .unwrap_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(&text).to_string());
        return Some(text.replace('\\', "/").into_bytes());
    }
    Some(bytes)
}

/// PHP's `dirname($path)` (one level, `/` separators).
pub fn dirname(path: &[u8]) -> Vec<u8> {
    let mut end = path.len();
    while end > 1 && path[end - 1] == b'/' {
        end -= 1;
    }
    let trimmed = &path[..end];
    match trimmed.iter().rposition(|&c| c == b'/') {
        None => {
            if path.first() == Some(&b'/') {
                b"/".to_vec()
            } else {
                b".".to_vec()
            }
        }
        Some(at) => {
            let mut stop = at;
            while stop > 0 && trimmed[stop - 1] == b'/' {
                stop -= 1;
            }
            if stop == 0 {
                b"/".to_vec()
            } else {
                trimmed[..stop].to_vec()
            }
        }
    }
}

/// `glob($pattern, GLOB_BRACE | GLOB_ONLYDIR | GLOB_NOSORT)` on local
/// paths (glibc's rules: `*`, `?`, `[...]` with `!`/`^`, `\` escapes, a
/// leading dot only matched explicitly, `{a,b}` alternatives), keeping
/// directories only. A trailing `/` is kept on the results, as glibc
/// does.
pub fn glob_dirs(pattern: &[u8]) -> Vec<Vec<u8>> {
    if is_url(pattern) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for p in brace_expand(pattern) {
        glob_one(&p, true, &mut out);
    }
    out
}

/// `glob($pattern, GLOB_NOSORT)`, or with `brace` `GLOB_NOSORT |
/// GLOB_BRACE`, on local paths: files and directories, by the rules of
/// [`glob_dirs`], where a segment starting with `.` also matches the `.`
/// and `..` entries. A pattern ending in `/` matches directories only,
/// and they keep the `/`.
pub fn glob(pattern: &[u8], brace: bool) -> Vec<Vec<u8>> {
    if is_url(pattern) {
        return Vec::new();
    }
    let mut out = Vec::new();
    if brace {
        for p in brace_expand(pattern) {
            glob_one(&p, false, &mut out);
        }
    } else {
        glob_one(pattern, false, &mut out);
    }
    out
}

fn brace_expand(pattern: &[u8]) -> Vec<Vec<u8>> {
    // the first "{" with a matching "}"
    let mut i = 0;
    while i < pattern.len() {
        match pattern[i] {
            b'\\' => i += 2,
            b'{' => {
                let mut depth = 0;
                let mut commas = vec![];
                let mut j = i;
                let mut close = None;
                while j < pattern.len() {
                    match pattern[j] {
                        b'\\' => j += 1,
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(j);
                                break;
                            }
                        }
                        b',' if depth == 1 => commas.push(j),
                        _ => {}
                    }
                    j += 1;
                }
                let Some(close) = close else {
                    return vec![pattern.to_vec()];
                };
                let mut bounds = vec![i];
                bounds.extend(commas);
                bounds.push(close);
                let mut out = Vec::new();
                for w in bounds.windows(2) {
                    let mut p = pattern[..i].to_vec();
                    p.extend_from_slice(&pattern[w[0] + 1..w[1]]);
                    p.extend_from_slice(&pattern[close + 1..]);
                    out.extend(brace_expand(&p));
                }
                return out;
            }
            _ => i += 1,
        }
    }
    vec![pattern.to_vec()]
}

fn has_magic(segment: &[u8]) -> bool {
    segment.iter().any(|c| matches!(c, b'*' | b'?' | b'['))
}

fn unescape(segment: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < segment.len() {
        if segment[i] == b'\\' && i + 1 < segment.len() {
            i += 1;
        }
        out.push(segment[i]);
        i += 1;
    }
    out
}

fn glob_one(pattern: &[u8], dirs_only: bool, out: &mut Vec<Vec<u8>>) {
    let trailing_slash = pattern.ends_with(b"/");
    let absolute = pattern.starts_with(b"/");
    let segments: Vec<&[u8]> = pattern
        .split(|&c| c == b'/')
        .filter(|s| !s.is_empty())
        .collect();
    let mut candidates: Vec<Vec<u8>> = vec![if absolute { b"/".to_vec() } else { Vec::new() }];
    for segment in &segments {
        let mut next = Vec::new();
        for base in &candidates {
            let join = |name: &[u8]| {
                let mut p = base.clone();
                if !p.is_empty() && !p.ends_with(b"/") {
                    p.push(b'/');
                }
                p.extend_from_slice(name);
                p
            };
            if has_magic(segment) {
                let dir: &[u8] = if base.is_empty() { b"." } else { base };
                let Ok(mut names) = read_dir(dir) else {
                    continue;
                };
                names.sort();
                // glibc's readdir() lists "." and "..", which a segment
                // starting with a dot matches (the full glob() only;
                // glob_dirs() keeps its behaviour).
                if !dirs_only && segment.first() == Some(&b'.') {
                    names.splice(0..0, [b".".to_vec(), b"..".to_vec()]);
                }
                for name in names {
                    if fnmatch(segment, &name) {
                        next.push(join(&name));
                    }
                }
            } else {
                next.push(join(&unescape(segment)));
            }
        }
        candidates = next;
    }
    for c in candidates {
        if is_dir(&c) {
            let mut c = c;
            if trailing_slash && !c.ends_with(b"/") {
                c.push(b'/');
            }
            out.push(c);
        } else if !dirs_only && !trailing_slash && lstat(&c).is_some() {
            out.push(c);
        }
    }
}

/// `fnmatch($pattern, $name, FNM_PERIOD)` for one path segment.
fn fnmatch(pattern: &[u8], name: &[u8]) -> bool {
    if name.first() == Some(&b'.') && pattern.first() != Some(&b'.') {
        return false;
    }
    match_from(pattern, name)
}

fn match_from(p: &[u8], s: &[u8]) -> bool {
    let Some(&c) = p.first() else {
        return s.is_empty();
    };
    match c {
        b'*' => (0..=s.len()).any(|k| match_from(&p[1..], &s[k..])),
        b'?' => !s.is_empty() && match_from(&p[1..], &s[1..]),
        b'[' => match bracket(p, s.first().copied()) {
            Some((true, len)) => match_from(&p[len..], &s[1..]),
            Some((false, _)) => false,
            None => s.first() == Some(&b'[') && match_from(&p[1..], &s[1..]),
        },
        b'\\' if p.len() > 1 => s.first() == Some(&p[1]) && match_from(&p[2..], &s[1..]),
        _ => s.first() == Some(&c) && match_from(&p[1..], &s[1..]),
    }
}

/// Whether `c` is in the bracket expression at the start of `p`, and its
/// length; None when it is not closed.
fn bracket(p: &[u8], c: Option<u8>) -> Option<(bool, usize)> {
    let mut i = 1;
    let negate = matches!(p.get(i), Some(b'!' | b'^'));
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    loop {
        let &x = p.get(i)?;
        if x == b']' && !first {
            break;
        }
        first = false;
        let lo = if x == b'\\' {
            i += 1;
            *p.get(i)?
        } else {
            x
        };
        i += 1;
        if p.get(i) == Some(&b'-') && p.get(i + 1).is_some_and(|&n| n != b']') {
            let hi = *p.get(i + 1)?;
            i += 2;
            if c.is_some_and(|c| lo <= c && c <= hi) {
                matched = true;
            }
        } else if c == Some(lo) {
            matched = true;
        }
    }
    Some((c.is_some() && matched != negate, i + 1))
}

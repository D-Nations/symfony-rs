//! PHP's `files` save handler, and `\SessionHandler`, the class that
//! calls it from a user save handler (`NativeFileSessionHandler` extends
//! it). Written from the PHP manual and PHP 8.4.27's answers in the oracle
//! (no PHP source code, PLAN.md decision 8).
//!
//! `session.save_path` is `[N;[MODE;]]DIR`: a session is the file
//! `DIR/sess_ID`, under `N` levels of directories named after the id's
//! first characters, created with `MODE` (octal, `0600` by default). An
//! id other than one to 256 of `[a-zA-Z0-9,-]` is refused with PHP's
//! warning. Reading creates the file and holds an exclusive lock on it
//! until the session is closed, as PHP does; garbage collection removes
//! the files of the top directory not changed for `gc_maxlifetime`
//! seconds.
//!
//! Upstream: none

use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use php_value::{PhpString, path::to_path};

use super::{warn, with_session};
use crate::session::storage::handler::{
    HandlerResult, SessionHandlerInterface, SessionIdInterface,
};

/// The session file open since `read()`, locked.
#[derive(Debug)]
pub struct OpenFile {
    id: PhpString,
    file: File,
}

/// The files module's state for this request.
#[derive(Debug, Clone, Default)]
pub struct FilesState {
    save_path: Option<SavePath>,
    open: Option<Arc<Mutex<OpenFile>>>,
}

#[derive(Debug, Clone)]
struct SavePath {
    depth: usize,
    /// The new file's permissions, which only Unix has.
    #[cfg_attr(not(unix), allow(dead_code))]
    mode: u32,
    dir: Vec<u8>,
}

/// `[N;[MODE;]]DIR`; `None` for more parts than that.
fn parse_save_path(save_path: &[u8]) -> Option<SavePath> {
    let parts: Vec<&[u8]> = save_path.split(|b| *b == b';').collect();
    let number = |part: &[u8], radix: u32| {
        std::str::from_utf8(part)
            .ok()
            .and_then(|t| u32::from_str_radix(t.trim(), radix).ok())
            .unwrap_or(0)
    };
    let (depth, mode, dir) = match parts.as_slice() {
        [dir] => (0, 0o600, *dir),
        [depth, dir] => (number(depth, 10), 0o600, *dir),
        [depth, mode, dir] => (number(depth, 10), number(mode, 8), *dir),
        _ => return None,
    };
    let dir = if dir.is_empty() {
        php_value::path::from_path(&std::env::temp_dir())
    } else {
        dir.to_vec()
    };
    Some(SavePath {
        depth: usize::try_from(depth).unwrap_or(0),
        mode,
        dir,
    })
}

/// One to 256 of `[a-zA-Z0-9,-]`.
pub fn valid_id(id: &[u8]) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b',' || *b == b'-')
}

fn lock(m: &Mutex<OpenFile>) -> std::sync::MutexGuard<'_, OpenFile> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn save_path() -> Option<SavePath> {
    with_session(|s| s.files.save_path.clone())
}

/// The file's path as PHP shows it, and for the system calls (`None`
/// for a path the system cannot name).
fn file_path(save_path: &SavePath, id: &[u8]) -> (Vec<u8>, Option<PathBuf>) {
    let mut shown = save_path.dir.clone();
    for c in id.iter().take(save_path.depth) {
        shown.push(b'/');
        shown.push(*c);
    }
    shown.extend_from_slice(b"/sess_");
    shown.extend_from_slice(id);
    let path = to_path(&shown);
    (shown, path)
}

/// `strerror(errno) (errno)`, as Linux words it.
fn os_error(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => "No such file or directory (2)".to_owned(),
        std::io::ErrorKind::PermissionDenied => "Permission denied (13)".to_owned(),
        _ => format!("{error} ({})", error.raw_os_error().unwrap_or(0)),
    }
}

/// `open($savePath, $sessionName)`.
pub fn open(save_path: &[u8]) -> bool {
    let parsed = parse_save_path(save_path);
    let ok = parsed.is_some();
    with_session(|s| s.files.save_path = parsed);
    ok
}

/// `close()`: the lock is released.
pub fn close() -> bool {
    with_session(|s| s.files.open = None);
    true
}

fn open_file(function: &str, id: &[u8]) -> Option<Arc<Mutex<OpenFile>>> {
    if let Some(open) = with_session(|s| s.files.open.clone())
        && lock(&open).id.as_bytes() == id
    {
        return Some(open);
    }
    let save_path = save_path()?;
    let (shown, path) = file_path(&save_path, id);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(save_path.mode);
    }
    let opened = path
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        .and_then(|path| options.open(path));
    let file = match opened {
        Ok(file) => file,
        Err(error) => {
            warn(
                function,
                format!(
                    "open({}, O_RDWR) failed: {}",
                    String::from_utf8_lossy(&shown),
                    os_error(&error)
                ),
            );
            return None;
        }
    };
    // PHP holds the session file locked until the session is closed
    let _ = file.lock();
    let open = Arc::new(Mutex::new(OpenFile {
        id: PhpString::from(id),
        file,
    }));
    with_session(|s| s.files.open = Some(open.clone()));
    Some(open)
}

/// `read($id)`: `None` is PHP's `false`.
pub fn read(function: &str, id: &[u8]) -> Option<PhpString> {
    if !valid_id(id) {
        warn(
            function,
            "Session ID is too long or contains illegal characters. Only the A-Z, a-z, 0-9, \"-\", and \",\" characters are allowed",
        );
        return None;
    }
    let open = open_file(function, id)?;
    let mut guard = lock(&open);
    let mut data = Vec::new();
    guard.file.seek(SeekFrom::Start(0)).ok()?;
    guard.file.read_to_end(&mut data).ok()?;
    Some(PhpString::from(data))
}

/// `write($id, $data)`.
pub fn write(function: &str, id: &[u8], data: &[u8]) -> bool {
    if !valid_id(id) {
        return false;
    }
    let Some(open) = open_file(function, id) else {
        return false;
    };
    let mut guard = lock(&open);
    guard.file.set_len(0).is_ok()
        && guard.file.seek(SeekFrom::Start(0)).is_ok()
        && guard.file.write_all(data).is_ok()
}

/// `updateTimestamp($id, $data)`: the file touched.
pub fn update_timestamp(function: &str, id: &[u8]) -> bool {
    if !valid_id(id) {
        return false;
    }
    let Some(open) = open_file(function, id) else {
        return false;
    };
    let guard = lock(&open);
    guard.file.set_modified(SystemTime::now()).is_ok()
}

/// `validateId($id)`: whether its file exists.
pub fn validate(id: &[u8]) -> bool {
    valid_id(id)
        && save_path().is_some_and(|p| file_path(&p, id).1.is_some_and(|path| path.is_file()))
}

/// `destroy($id)`: its file removed (one that is not there is no failure).
pub fn destroy(id: &[u8]) -> bool {
    with_session(|s| {
        if s.files
            .open
            .as_ref()
            .is_some_and(|o| lock(o).id.as_bytes() == id)
        {
            s.files.open = None;
        }
    });
    let Some(save_path) = save_path() else {
        return false;
    };
    if !valid_id(id) {
        return false;
    }
    let Some(path) = file_path(&save_path, id).1 else {
        return false;
    };
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

/// `gc($maxlifetime)`: the number of session files removed.
pub fn gc(max_lifetime: i64) -> Option<i64> {
    let save_path = save_path()?;
    if save_path.depth > 0 {
        return Some(0);
    }
    let cutoff = SystemTime::now().checked_sub(Duration::from_secs(
        u64::try_from(max_lifetime).unwrap_or(0),
    ))?;
    let mut removed = 0;
    let entries = std::fs::read_dir(to_path(&save_path.dir)?).ok()?;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("sess_") {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|modified| modified < cutoff);
        if old && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Some(removed)
}

/// `new \SessionHandler()`: PHP's files handler as an object.
#[derive(Debug, Default)]
pub struct SessionHandler;

/// `\SessionHandler::class`.
pub const CLASS: &str = "SessionHandler";

impl SessionHandlerInterface for SessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], _session_name: &[u8]) -> HandlerResult<bool> {
        Ok(open(save_path))
    }

    fn close(&self) -> HandlerResult<bool> {
        Ok(close())
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Ok(read("SessionHandler::read", session_id))
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        Ok(write("SessionHandler::write", session_id, data))
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Ok(destroy(session_id))
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        Ok(gc(max_lifetime))
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }

    fn is_native(&self) -> bool {
        true
    }
}

impl SessionIdInterface for SessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        Ok(super::random_id().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_paths_are_depth_mode_and_directory() {
        let p = parse_save_path(b"2;0640;/var/sess");
        assert_eq!(
            Some((2, 0o640, b"/var/sess".to_vec())),
            p.map(|p| (p.depth, p.mode, p.dir))
        );
        assert!(parse_save_path(b"a;b;c;d").is_none());
        assert_eq!(
            b"/d/a/b/sess_abc".to_vec(),
            file_path(
                &SavePath {
                    depth: 2,
                    mode: 0o600,
                    dir: b"/d".to_vec()
                },
                b"abc"
            )
            .0
        );
    }

    #[test]
    fn ids_are_256_safe_characters_at_most() {
        assert!(valid_id(b"abc,-XYZ09"));
        assert!(!valid_id(b"bad!chars"));
        assert!(!valid_id(b""));
        assert!(valid_id(&[b'a'; 256]));
        assert!(!valid_id(&[b'a'; 257]));
    }
}

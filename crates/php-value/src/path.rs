//! File paths: PHP's byte-string paths and the standard library's
//! [`Path`]/[`PathBuf`], and PHP's `basename()` and `dirname()`.
//!
//! PHP keeps a path as a byte string and `DIRECTORY_SEPARATOR` is `/` on
//! Linux, the system the recorded vectors come from. The crates keep that
//! form for every path PHP shows (messages, return values, keys), and use
//! [`Path`]/[`PathBuf`] for every call to the operating system. This module
//! is the one place that converts between the two, so no crate needs its
//! own `#[cfg(unix)]` code for it.
//!
//! - On Unix the conversion is exact in both directions (`OsStr` is bytes).
//! - On other systems a path must be UTF-8 to name a file: [`to_path`]
//!   gives `None` for other bytes, so a caller never opens a file whose
//!   name was changed by a lossy conversion. [`from_path`] replaces a name
//!   that is not Unicode with U+FFFD, as only such a name can be shown.
//!
//! Upstream: none

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The [`PathBuf`] for PHP's path bytes, for a call to the system.
/// `None` on systems other than Unix when the bytes are not UTF-8.
/// An empty path gives an empty [`PathBuf`]; callers that treat `""` as
/// no file check for it first.
pub fn to_path(bytes: &[u8]) -> Option<PathBuf> {
    to_os_str(bytes).map(PathBuf::from)
}

/// The [`OsStr`] for PHP's bytes: a path, a program argument or an
/// environment value. `None` on systems other than Unix when the bytes
/// are not UTF-8.
pub fn to_os_str(bytes: &[u8]) -> Option<&OsStr> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes).ok().map(OsStr::new)
    }
}

/// The bytes PHP has for a path the system gave.
pub fn from_path(path: &Path) -> Vec<u8> {
    from_os_str(path.as_os_str())
}

/// The bytes PHP has for a string the system gave (a path, an argument,
/// an environment value).
pub fn from_os_str(s: &OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        s.to_string_lossy().into_owned().into_bytes()
    }
}

/// [`from_os_str`] for an owned string, without a copy on Unix.
pub fn from_os_string(s: OsString) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        s.into_vec()
    }
    #[cfg(not(unix))]
    {
        from_os_str(&s)
    }
}

/// PHP's `basename($path)` (no suffix), `/` being the separator: the part
/// after the last `/`, trailing `/` ignored. `basename('/')` is `''`.
pub fn basename(path: &[u8]) -> &[u8] {
    let end = path.iter().rposition(|&c| c != b'/').map_or(0, |i| i + 1);
    let trimmed = &path[..end];
    let start = trimmed
        .iter()
        .rposition(|&c| c == b'/')
        .map_or(0, |i| i + 1);
    &trimmed[start..]
}

/// PHP extension of a basename, including leading dots.
pub fn basename_extension(name: &[u8]) -> &[u8] {
    name.iter()
        .rposition(|&c| c == b'.')
        .map_or(b"".as_slice(), |at| &name[at + 1..])
}

/// PHP extension of a path, after applying PHP basename rules.
pub fn pathinfo_extension(path: &[u8]) -> &[u8] {
    basename_extension(basename(path))
}

/// [`basename`] of a UTF-8 path. The result is UTF-8, as `/` is ASCII.
pub fn basename_str(path: &str) -> &str {
    let end = path.trim_end_matches('/').len();
    let start = path[..end].rfind('/').map_or(0, |i| i + 1);
    &path[start..end]
}

/// PHP's `dirname($path)` (one level), `/` being the separator: the path
/// without its last part and the `/` before it. `'.'` when no `/` is
/// left, `'/'` for a path at the root, and `''` for `''`.
pub fn dirname(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        return Vec::new();
    }
    let mut end = path.len();
    while end > 1 && path[end - 1] == b'/' {
        end -= 1;
    }
    let trimmed = &path[..end];
    match trimmed.iter().rposition(|&c| c == b'/') {
        None => {
            if path[0] == b'/' {
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

/// [`dirname`] of a UTF-8 path. The result is UTF-8, as `/` is ASCII.
pub fn dirname_str(path: &str) -> String {
    let dir = dirname(path.as_bytes());
    // `dir` is a prefix of `path` cut before a `/`, or "/" or "."
    String::from_utf8(dir).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // [path, basename(path), dirname(path)] as PHP 8.4 gives them on Linux
    // (PHP manual; the old copies in finder and filesystem agreed). Not yet
    // recorded in the oracle.
    const CASES: &[(&str, &str, &str)] = &[
        ("", "", ""),
        ("/", "", "/"),
        ("//", "", "/"),
        ("a", "a", "."),
        ("a/", "a", "."),
        ("/a", "a", "/"),
        ("/a/", "a", "/"),
        ("//a", "a", "/"),
        ("a//b", "b", "a"),
        ("/a/b", "b", "/a"),
        ("/a/b/", "b", "/a"),
        ("a/b//", "b", "a"),
        (".", ".", "."),
        ("..", "..", "."),
        ("a\\b", "a\\b", "."),
        ("/é/ü", "ü", "/é"),
    ];

    #[test]
    fn basename_and_dirname() {
        for &(path, base, dir) in CASES {
            assert_eq!(
                base.as_bytes(),
                basename(path.as_bytes()),
                "basename({path:?})"
            );
            assert_eq!(base, basename_str(path), "basename({path:?})");
            assert_eq!(
                dir.as_bytes(),
                dirname(path.as_bytes()),
                "dirname({path:?})"
            );
            assert_eq!(dir, dirname_str(path), "dirname({path:?})");
        }
    }

    #[test]
    fn round_trip() {
        for path in ["", "/tmp/a b", "relative/é", "C:/x"] {
            let p = to_path(path.as_bytes()).unwrap_or_default();
            assert_eq!(path.as_bytes(), from_path(&p));
            assert_eq!(path.as_bytes(), from_os_string(p.into_os_string()));
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_bytes_are_kept_on_unix() {
        let bytes = b"/tmp/\xff\xfe";
        let p = to_path(bytes).unwrap_or_default();
        assert_eq!(bytes.to_vec(), from_path(&p));
    }

    #[cfg(not(unix))]
    #[test]
    fn non_utf8_bytes_name_no_file_elsewhere() {
        assert_eq!(None, to_path(b"/tmp/\xff\xfe"));
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;

    #[test]
    fn extension_preserves_php_dot_rules() {
        assert_eq!(basename_extension(b".env"), b"env");
        assert_eq!(basename_extension(b"name."), b"");
        assert_eq!(pathinfo_extension(b"/dir.with.dot/file"), b"");
        assert_eq!(pathinfo_extension(b"/dir/file.tar.gz"), b"gz");
    }
}

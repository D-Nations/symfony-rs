//! The checks Symfony's file loaders make on a resource name before
//! reading it: `stream_is_local()` and `file_exists()` as PHP's stream
//! wrappers answer them (recorded): `http`, `https`, `ftp`, `ftps`, `data`
//! and a `file://` URL with a host other than `localhost` are not local;
//! `php`, `glob` and `compress.zlib` URLs do not exist; a `file://` URL
//! names its path; an unknown scheme is a plain path. A `phar://` URL into
//! an existing archive is refused; one without an archive does not exist.
//!
//! Upstream: none

use std::path::PathBuf;

/// What a resource string that is not XML stands for.
pub(crate) enum Location {
    /// `stream_is_local()` is false.
    Remote,
    /// A wrapper whose files never exist for `file_exists()`.
    Absent,
    Phar,
    Path(PathBuf),
}

fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// The scheme of `scheme://…` (letters, digits, `+`, `-`, `.`), lowercased.
fn scheme(resource: &[u8]) -> Option<(String, &[u8])> {
    let end = resource
        .iter()
        .position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')))?;
    if end == 0 {
        return None;
    }
    let rest = resource[end..].strip_prefix(b"://")?;
    Some((
        String::from_utf8_lossy(&resource[..end]).to_ascii_lowercase(),
        rest,
    ))
}

pub(crate) fn locate(resource: &[u8]) -> Location {
    if resource.len() >= 5 && resource[..5].eq_ignore_ascii_case(b"data:") {
        return Location::Remote;
    }
    match scheme(resource) {
        Some((scheme, rest)) => match scheme.as_str() {
            "http" | "https" | "ftp" | "ftps" | "data" => Location::Remote,
            "file" if rest.starts_with(b"/") => Location::Path(bytes_to_path(rest)),
            "file" if rest.len() >= 10 && rest[..10].eq_ignore_ascii_case(b"localhost/") => {
                Location::Path(bytes_to_path(&rest[9..]))
            }
            "file" => Location::Remote,
            "php" | "glob" | "compress.zlib" => Location::Absent,
            "phar" if phar_archive(rest) => Location::Phar,
            "phar" => Location::Absent,
            _ => Location::Path(bytes_to_path(resource)),
        },
        None => Location::Path(bytes_to_path(resource)),
    }
}

/// Whether a `phar://` URL's path starts with an existing file (the
/// archive PHP would open).
fn phar_archive(rest: &[u8]) -> bool {
    (1..=rest.len()).any(|end| {
        (end == rest.len() || rest[end] == b'/') && bytes_to_path(&rest[..end]).is_file()
    })
}

pub(crate) fn quoted(before: &str, resource: &[u8], after: &str) -> Vec<u8> {
    let mut m = before.as_bytes().to_vec();
    m.extend_from_slice(resource);
    m.extend_from_slice(after.as_bytes());
    m
}

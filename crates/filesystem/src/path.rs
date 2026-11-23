//! Port of Symfony\Component\Filesystem\Path.php: the methods
//! `Filesystem::dumpFile()` needs to follow a relative symbolic link
//! (`makeAbsolute()`, `canonicalize()`, `isAbsolute()`) and the ones they
//! share (`normalize()`, `getRoot()`, `getHomeDirectory()`).
//!
//! Paths are bytes. `DIRECTORY_SEPARATOR` is `/` on every system, as in
//! the rest of this workspace (Linux behaviour is canonical), so the
//! Windows branches upstream takes when it is `\` are not ported:
//! `normalize()` keeps backslashes and `C:` is not a root.
//!
//! The environment (`getenv('HOME')`, `HOMEDRIVE`, `HOMEPATH`) is read
//! through an [`Env`]: the process's by default, or given, because Rust
//! cannot change its own environment safely (upstream's tests use
//! `putenv()`). Upstream's buffer of canonical paths is a cache only and
//! is not ported.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/filesystem Path.php

use crate::{Class, Error, php};

/// The variables `getHomeDirectory()` reads; `None` is `getenv()`'s false.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    pub home: Option<Vec<u8>>,
    pub homedrive: Option<Vec<u8>>,
    pub homepath: Option<Vec<u8>>,
}

impl Env {
    /// The process environment.
    pub fn process() -> Self {
        let get = |name: &str| std::env::var_os(name).map(php::os_bytes);
        Self {
            home: get("HOME"),
            homedrive: get("HOMEDRIVE"),
            homepath: get("HOMEPATH"),
        }
    }
}

/// PHP's truthiness of a `getenv()` result: false, `""` and `"0"` are
/// false.
fn truthy(value: &Option<Vec<u8>>) -> bool {
    value.as_deref().is_some_and(|v| !v.is_empty() && v != b"0")
}

/// `Path`: static methods only.
pub struct Path;

impl Path {
    /// `canonicalize($path)`, with the process environment for `~`.
    pub fn canonicalize(path: &[u8]) -> Result<Vec<u8>, Error> {
        Self::canonicalize_in(path, &Env::process())
    }

    /// `canonicalize($path)` with `env` for `~`: `.` and empty segments
    /// removed, `..` collapsed (and dropped above a root), `~` replaced
    /// by the home directory, the scheme kept.
    pub fn canonicalize_in(path: &[u8], env: &Env) -> Result<Vec<u8>, Error> {
        if path.is_empty() {
            return Ok(Vec::new());
        }
        let mut path = path.to_vec();
        if path[0] == b'~' {
            let mut home = Self::get_home_directory_in(env)?;
            home.extend_from_slice(&path[1..]);
            path = home;
        }
        let path = Self::normalize(&path);
        let (root, rest) = split(&path);
        let parts = find_canonical_parts(&root, &rest);
        let mut out = root;
        out.extend_from_slice(&parts.join(&b'/'));
        Ok(out)
    }

    /// `normalize($path)`: unchanged, `DIRECTORY_SEPARATOR` being `/`.
    pub fn normalize(path: &[u8]) -> Vec<u8> {
        path.to_vec()
    }

    /// `getHomeDirectory()` from the process environment.
    pub fn get_home_directory() -> Result<Vec<u8>, Error> {
        Self::get_home_directory_in(&Env::process())
    }

    /// `getHomeDirectory()`: `HOME`, else `HOMEDRIVE` + `HOMEPATH`,
    /// canonicalized; a RuntimeException when neither is set.
    pub fn get_home_directory_in(env: &Env) -> Result<Vec<u8>, Error> {
        if truthy(&env.home) {
            let home = env.home.clone().unwrap_or_default();
            return Self::canonicalize_in(&home, env);
        }
        if truthy(&env.homedrive) && truthy(&env.homepath) {
            let mut home = env.homedrive.clone().unwrap_or_default();
            home.extend_from_slice(env.homepath.as_deref().unwrap_or_default());
            return Self::canonicalize_in(&home, env);
        }
        Err(Error::new(
            Class::Runtime,
            "Cannot find the home directory path: Your environment or operating system isn't supported.",
        ))
    }

    /// `getRoot($path)`: the scheme and `/`, or `""` for a relative path.
    pub fn get_root(path: &[u8]) -> Vec<u8> {
        if path.is_empty() {
            return Vec::new();
        }
        let (scheme, rest) = match find(path, b"://") {
            Some(at) => (&path[..at + 3], &path[at + 3..]),
            None => (&path[..0], path),
        };
        // upstream reads $path[0]; on an empty rest PHP warns and reads
        // "", which is not "/"
        if rest.first() == Some(&b'/') {
            let mut root = scheme.to_vec();
            root.push(b'/');
            return root;
        }
        Vec::new()
    }

    /// `isAbsolute($path)`: starts with `/`, or contains `://` and
    /// `parse_url()` gives a scheme or false (anything but null).
    pub fn is_absolute(path: &[u8]) -> bool {
        if path.is_empty() {
            return false;
        }
        if find(path, b"://").is_some() && php::parse_url_scheme_value(path) != php::UrlScheme::Null
        {
            return true;
        }
        path[0] == b'/'
    }

    /// `isRelative($path)`.
    pub fn is_relative(path: &[u8]) -> bool {
        !Self::is_absolute(path)
    }

    /// `makeAbsolute($path, $basePath)` with the process environment.
    pub fn make_absolute(path: &[u8], base_path: &[u8]) -> Result<Vec<u8>, Error> {
        Self::make_absolute_in(path, base_path, &Env::process())
    }

    /// `makeAbsolute($path, $basePath)`: `path` canonicalized against an
    /// absolute base, or alone when it is absolute.
    pub fn make_absolute_in(path: &[u8], base_path: &[u8], env: &Env) -> Result<Vec<u8>, Error> {
        if base_path.is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                "The base path must be a non-empty string. Got: \"\".",
            ));
        }
        if !Self::is_absolute(base_path) {
            let mut message = b"The base path \"".to_vec();
            message.extend_from_slice(base_path);
            message.extend_from_slice(b"\" is not an absolute path.");
            return Err(Error::new(Class::InvalidArgument, message));
        }
        if Self::is_absolute(path) {
            return Self::canonicalize_in(path, env);
        }
        let (scheme, base) = match find(base_path, b"://") {
            Some(at) => (&base_path[..at + 3], &base_path[at + 3..]),
            None => (&base_path[..0], base_path),
        };
        // rtrim($basePath, '/'.DIRECTORY_SEPARATOR)
        let end = base.iter().rposition(|&c| c != b'/').map_or(0, |i| i + 1);
        let mut joined = base[..end].to_vec();
        joined.push(b'/');
        joined.extend_from_slice(path);
        let mut out = scheme.to_vec();
        out.extend_from_slice(&Self::canonicalize_in(&joined, env)?);
        Ok(out)
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `split($path)`: the root (scheme and `/`) and the rest.
fn split(path: &[u8]) -> (Vec<u8>, Vec<u8>) {
    if path.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let (mut root, rest) = match find(path, b"://") {
        Some(at) => (path[..at + 3].to_vec(), &path[at + 3..]),
        None => (Vec::new(), path),
    };
    if rest.first() == Some(&b'/') {
        root.push(b'/');
        return (root, rest[1..].to_vec());
    }
    (root, rest.to_vec())
}

/// `findCanonicalParts($root, $pathWithoutRoot)`.
fn find_canonical_parts(root: &[u8], path: &[u8]) -> Vec<Vec<u8>> {
    let mut parts: Vec<Vec<u8>> = Vec::new();
    for part in path.split(|&c| c == b'/') {
        if part == b"." || part.is_empty() {
            continue;
        }
        if part == b".." && parts.last().is_some_and(|last| last != b"..") {
            parts.pop();
            continue;
        }
        if part != b".." || root.is_empty() {
            parts.push(part.to_vec());
        }
    }
    parts
}

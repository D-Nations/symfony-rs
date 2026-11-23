//! Port of Symfony\Component\Config\FileLocator.php, with
//! FileLocatorInterface.php.
//!
//! Paths are bytes, as PHP strings are, and the filesystem is the one
//! symfony-finder presents (`symfony_finder::fs`): local paths with `/`
//! as `DIRECTORY_SEPARATOR` on every system (Linux, the oracle's), and
//! `scheme://` URLs served by registered stream wrappers.
//!
//! `isAbsolutePath()` calls `parse_url($file, PHP_URL_SCHEME)`; that
//! function is written here from PHP's answers on a recorded corpus
//! (`oracle/recorders/config_loaders.php`), not from its source.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config FileLocator.php FileLocatorInterface.php

use symfony_finder::fs;

use crate::{Class, Error, exception::file_locator_file_not_found_exception, php::implode};

/// What `locate()` returns: a path (`$first`), or every path found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Path(Vec<u8>),
    Paths(Vec<Vec<u8>>),
}

impl Located {
    /// The value as PHP holds it (`string|string[]`).
    pub fn to_value(&self) -> php_value::Value {
        use php_value::{PhpArray, Value};
        match self {
            Self::Path(p) => Value::string(p.clone()),
            Self::Paths(ps) => {
                Value::Array(PhpArray::list(ps.iter().map(|p| Value::string(p.clone()))))
            }
        }
    }
}

/// `FileLocatorInterface`.
pub trait FileLocatorInterface: Send + Sync {
    /// `locate($name, $currentPath, $first)`: the full path of a file
    /// name, or with `first == false` every match.
    fn locate(
        &self,
        name: &[u8],
        current_path: Option<&[u8]>,
        first: bool,
    ) -> Result<Located, Error>;
}

/// `FileLocator`: looks for files in pre-defined paths.
#[derive(Debug, Clone, Default)]
pub struct FileLocator {
    paths: Vec<Vec<u8>>,
}

impl FileLocator {
    /// `new FileLocator($paths)`: one path or several.
    pub fn new<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        Self {
            paths: paths.into_iter().map(|p| p.as_ref().to_vec()).collect(),
        }
    }

    /// `isAbsolutePath()` (private upstream; upstream's test reaches it
    /// by reflection).
    pub fn is_absolute_path(file: &[u8]) -> bool {
        let first = file.first().copied().unwrap_or_default();
        first == b'/'
            || first == b'\\'
            || (file.len() > 3
                && first.is_ascii_alphabetic()
                && file[1] == b':'
                && (file[2] == b'\\' || file[2] == b'/'))
            || parse_url_scheme(file).is_some_and(|s| s.as_slice() != b"0")
            // "parse_url()" doesn't handle absolute phar path, despite
            // being valid
            || file.starts_with(b"phar:///")
    }
}

impl FileLocatorInterface for FileLocator {
    fn locate(
        &self,
        name: &[u8],
        current_path: Option<&[u8]>,
        first: bool,
    ) -> Result<Located, Error> {
        if name.is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                "An empty file name is not valid to be located.",
            ));
        }

        if Self::is_absolute_path(name) {
            if !fs::file_exists(name) {
                let mut message = b"The file \"".to_vec();
                message.extend_from_slice(name);
                message.extend_from_slice(b"\" does not exist.");
                return Err(file_locator_file_not_found_exception(
                    message,
                    vec![name.to_vec()],
                ));
            }

            return Ok(Located::Path(name.to_vec()));
        }

        let mut paths = self.paths.clone();

        if let Some(current) = current_path {
            paths.insert(0, current.to_vec());
        }

        // array_unique(): the first of equal strings stays.
        let mut unique: Vec<Vec<u8>> = Vec::new();
        for p in paths {
            if !unique.contains(&p) {
                unique.push(p);
            }
        }
        let paths = unique;
        let mut filepaths = Vec::new();
        let mut notfound = Vec::new();

        for path in &paths {
            let mut file = path.clone();
            file.push(b'/');
            file.extend_from_slice(name);
            if fs::file_exists(&file) {
                if first {
                    return Ok(Located::Path(file));
                }
                filepaths.push(file);
            } else {
                notfound.push(file);
            }
        }

        if filepaths.is_empty() {
            let mut message = b"The file \"".to_vec();
            message.extend_from_slice(name);
            message.extend_from_slice(b"\" does not exist (in: \"");
            message.extend_from_slice(&implode(b"\", \"", paths.iter().cloned()));
            message.extend_from_slice(b"\").");
            return Err(file_locator_file_not_found_exception(message, notfound));
        }

        Ok(Located::Paths(filepaths))
    }
}

/// `parse_url($url, PHP_URL_SCHEME)`: the scheme, or `None` where PHP
/// gives null (no scheme) or false (a URL it rejects).
///
/// The rules, from PHP's recorded answers (tests/data/loaders.json): the
/// scheme is what precedes the first `:` when that is non-empty and made
/// of letters, digits, `+`, `-` and `.`; but a `:` followed by one to
/// five digits that end the string or reach a `/` reads as a port (no
/// scheme). After `scheme://`, the authority (up to `/`, `?` or `#`) must
/// have a host (after any `user@`) and a port, if any, of digits up to
/// 65535; an empty authority is rejected unless the scheme is `file`
/// and a path follows.
pub fn parse_url_scheme(url: &[u8]) -> Option<Vec<u8>> {
    let colon = url.iter().position(|&c| c == b':')?;
    let scheme = &url[..colon];
    if scheme.is_empty()
        || !scheme
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.'))
    {
        return None;
    }
    let rest = &url[colon + 1..];
    if rest.first() != Some(&b'/') {
        let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && digits < 6 && (digits == rest.len() || rest[digits] == b'/') {
            return None;
        }
        return Some(scheme.to_vec());
    }
    let Some(after) = rest.strip_prefix(b"//") else {
        return Some(scheme.to_vec());
    };
    let end = after
        .iter()
        .position(|&c| matches!(c, b'/' | b'?' | b'#'))
        .unwrap_or(after.len());
    let authority = &after[..end];
    if authority.is_empty() {
        let has_path = after.len() > end && after[end] == b'/';
        return (scheme.eq_ignore_ascii_case(b"file") && has_path).then(|| scheme.to_vec());
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
        return None;
    }
    if let Some(port) = port.filter(|p| !p.is_empty()) {
        let valid = port.len() <= 5
            && port.iter().all(u8::is_ascii_digit)
            && std::str::from_utf8(port)
                .ok()
                .and_then(|p| p.parse::<u32>().ok())
                .is_some_and(|p| p <= 65535);
        if !valid {
            return None;
        }
    }
    Some(scheme.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_basics() {
        assert_eq!(
            parse_url_scheme(b"https://server/foo.xml"),
            Some(b"https".to_vec())
        );
        assert_eq!(parse_url_scheme(b"foo.xml"), None);
        assert_eq!(parse_url_scheme(b"localhost:80"), None);
    }
}

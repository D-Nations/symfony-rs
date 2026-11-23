//! Port of Symfony\Component\Config\Resource\GlobResource.php: the files
//! a glob pattern finds under a prefix, fresh while that list is the
//! same.
//!
//! The pattern is turned into a regex by symfony-finder's `Glob`, as
//! upstream does when symfony/finder is installed (it is a dependency
//! here), and matched by `php-pcre`. PHP's `glob()` is
//! `symfony_finder::fs::glob` (glibc's rules; `GLOB_BRACE` is defined on
//! the oracle's Linux). The walk of a matched directory is
//! `RecursiveDirectoryIterator` with `SKIP_DOTS | FOLLOW_SYMLINKS` under
//! upstream's filter, leaves only. Paths are yielded in upstream's order
//! (`strnatcmp()` of the keys). The iterator's `SplFileInfo` values are
//! represented by their path, which is all callers here read.
//!
//! `phar://` prefixes are refused: no PHP archive reader exists here.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Resource/GlobResource.php

use std::sync::OnceLock;

use php_value::{Key, PhpArray, Value, php_array};
use symfony_finder::{Finder, Glob, fs, php::strnatcmp};

use super::{ResourceInterface, SelfCheckingResourceInterface, resolve, xxh128};
use crate::{Class, Error, php};

pub const CLASS: &str = "Symfony\\Component\\Config\\Resource\\GlobResource";

/// A path `glob()` found and the regex its walk filters with.
type Candidate = (Vec<u8>, Option<Vec<u8>>);

/// `defined('GLOB_BRACE') ? GLOB_BRACE : 0` on the oracle's glibc.
const GLOB_BRACE: bool = true;

#[derive(Debug)]
pub struct GlobResource {
    prefix: Vec<u8>,
    pattern: Vec<u8>,
    recursive: bool,
    hash: OnceLock<Vec<u8>>,
    for_exclusion: bool,
    excluded_prefixes: PhpArray,
    glob_brace: bool,
}

impl Clone for GlobResource {
    fn clone(&self) -> Self {
        let hash = OnceLock::new();
        if let Some(h) = self.hash.get() {
            let _ = hash.set(h.clone());
        }
        Self {
            prefix: self.prefix.clone(),
            pattern: self.pattern.clone(),
            recursive: self.recursive,
            hash,
            for_exclusion: self.for_exclusion,
            excluded_prefixes: self.excluded_prefixes.clone(),
            glob_brace: self.glob_brace,
        }
    }
}

impl GlobResource {
    /// `new GlobResource($prefix, $pattern, $recursive, $forExclusion,
    /// $excludedPrefixes)`. `excluded_prefixes` is upstream's array of
    /// paths (keys) to skip, set when the value is not null.
    pub fn new(
        prefix: &[u8],
        pattern: &[u8],
        recursive: bool,
        for_exclusion: bool,
        excluded_prefixes: PhpArray,
    ) -> Result<Self, Error> {
        let mut excluded_prefixes = excluded_prefixes;
        // ksort($excludedPrefixes)
        let mut keys = php::keys_of(&excluded_prefixes);
        php::sort_keys(&mut keys);
        let mut sorted = PhpArray::new();
        for k in keys {
            if let Some(v) = excluded_prefixes.get(&k) {
                sorted.insert(k, v.clone());
            }
        }
        excluded_prefixes = sorted;

        let Some(resolved) = resolve(prefix) else {
            let mut message = b"The path \"".to_vec();
            message.extend_from_slice(prefix);
            message.extend_from_slice(b"\" does not exist.");
            return Err(Error::new(Class::InvalidArgument, message));
        };

        Ok(Self {
            prefix: resolved,
            pattern: pattern.to_vec(),
            recursive,
            hash: OnceLock::new(),
            for_exclusion,
            excluded_prefixes,
            glob_brace: GLOB_BRACE,
        })
    }

    /// `__unserialize($data)`: the values in order (prefix, pattern,
    /// recursive, hash, forExclusion, excludedPrefixes), whatever their
    /// keys. An object among them is refused, as a Stringable one is
    /// upstream (none here converts to a string property).
    pub fn from_serialized(data: &PhpArray) -> Result<Self, Error> {
        if data.values().any(|v| matches!(v, Value::Object(_))) {
            return Err(Error::new(
                Class::Other {
                    class: "BadMethodCallException".to_owned(),
                    exception: true,
                },
                format!("Cannot unserialize {CLASS}"),
            ));
        }
        let mut values = data.values();
        let mut next = |name: &str| {
            values.next().cloned().ok_or_else(|| {
                Error::new(
                    Class::TypeError,
                    format!("Cannot assign null to property {CLASS}::${name}"),
                )
            })
        };
        let string = |v: Value, name: &str| match v {
            Value::String(s) => Ok(s.as_bytes().to_vec()),
            other => Err(Error::new(
                Class::TypeError,
                format!(
                    "Cannot assign {} to property {CLASS}::${name} of type string",
                    other.debug_type()
                ),
            )),
        };
        let boolean = |v: Value, name: &str| match v {
            Value::Bool(b) => Ok(b),
            other => Err(Error::new(
                Class::TypeError,
                format!(
                    "Cannot assign {} to property {CLASS}::${name} of type bool",
                    other.debug_type()
                ),
            )),
        };
        let prefix = string(next("prefix")?, "prefix")?;
        let pattern = string(next("pattern")?, "pattern")?;
        let recursive = boolean(next("recursive")?, "recursive")?;
        let hash = string(next("hash")?, "hash")?;
        let for_exclusion = boolean(next("forExclusion")?, "forExclusion")?;
        let excluded_prefixes = match next("excludedPrefixes")? {
            Value::Array(a) => a,
            other => {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "Cannot assign {} to property {CLASS}::$excludedPrefixes of type array",
                        other.debug_type()
                    ),
                ));
            }
        };
        let cell = OnceLock::new();
        let _ = cell.set(hash);
        Ok(Self {
            prefix,
            pattern,
            recursive,
            hash: cell,
            for_exclusion,
            excluded_prefixes,
            glob_brace: GLOB_BRACE,
        })
    }

    /// `getPrefix()`.
    pub fn get_prefix(&self) -> &[u8] {
        &self.prefix
    }

    /// Upstream's private `$globBrace`, which its test sets to 0 by
    /// reflection to take the brace fallback.
    pub fn set_glob_brace(&mut self, glob_brace: bool) {
        self.glob_brace = glob_brace;
    }

    pub fn glob_brace(&self) -> bool {
        self.glob_brace
    }

    fn is_excluded(&self, path: &[u8]) -> bool {
        self.excluded_prefixes
            .get(&Key::from_bytes(path))
            .is_some_and(|v| !v.is_null())
    }

    /// The `do { … } while (…)` of upstream: whether the path or one of
    /// its parents up to the prefix is excluded.
    fn under_excluded(&self, path: &[u8], prefix: &[u8]) -> bool {
        let mut normalized = normalize(path);
        loop {
            let dir_path = normalized.clone();
            if self.is_excluded(&dir_path) {
                return true;
            }
            if prefix == dir_path.as_slice() {
                return false;
            }
            normalized = fs::dirname(&dir_path);
            if dir_path == normalized {
                return false;
            }
        }
    }

    /// `getIterator()`: the paths found, in upstream's order.
    pub fn get_iterator(&self) -> Result<Vec<Vec<u8>>, Error> {
        let mut out = Vec::new();
        if (!self.recursive && self.pattern.is_empty()) || !fs::file_exists(&self.prefix) {
            return Ok(out);
        }

        let mut prefix = normalize(&self.prefix);
        let pattern = if fs::is_file(&prefix) {
            prefix = fs::dirname(&prefix);
            let mut p = basename(&prefix).to_vec();
            p.extend_from_slice(&self.pattern);
            p
        } else {
            self.pattern.clone()
        };

        let mut regex = Glob::to_regex(&pattern, true, true, Some(b'#'));
        if self.recursive {
            // substr_replace($regex, …, -2, 1)
            let at = regex.len().saturating_sub(2);
            let with: &[u8] = if pattern.ends_with(b"/") {
                b""
            } else {
                b"(/|$)"
            };
            regex.splice(at..at + 1, with.iter().copied());
        }

        let prefix_len = prefix.len();
        let mut paths: Option<Vec<Candidate>> = None;
        let add =
            |paths: &mut Vec<(Vec<u8>, Option<Vec<u8>>)>, path: Vec<u8>, rx: Option<Vec<u8>>| {
                if !paths.iter().any(|(p, _)| *p == path) {
                    paths.push((path, rx));
                }
            };

        if self.pattern.is_empty() && fs::is_file(&self.prefix) {
            paths = Some(vec![(self.prefix.clone(), None)]);
        } else if !self.prefix.starts_with(b"phar://") {
            let has_globstar = php::contains(&self.pattern, b"/**/");
            if !has_globstar && (self.glob_brace || !self.pattern.contains(&b'{')) {
                let mut found = Vec::new();
                let mut full = self.prefix.clone();
                full.extend_from_slice(&self.pattern);
                for path in fs::glob(&full, self.glob_brace) {
                    add(&mut found, path, None);
                }
                paths = Some(found);
            } else if !self.pattern.contains(&b'\\') || !escaped_brace(&self.pattern) {
                let mut found = Vec::new();
                for p in expand_glob(&self.pattern) {
                    let (p, globstar) = match find(&p, b"/**/") {
                        Some(i) => {
                            let mut q = p[..i].to_vec();
                            q.extend_from_slice(b"/*");
                            (q, true)
                        }
                        None => (p, false),
                    };
                    let mut full = self.prefix.clone();
                    full.extend_from_slice(&p);
                    for path in fs::glob(&full, false) {
                        add(
                            &mut found,
                            path,
                            if globstar { Some(regex.clone()) } else { None },
                        );
                    }
                }
                paths = Some(found);
            }
        } else {
            return Err(Error::new(
                Class::Runtime,
                format!(
                    "GlobResource cannot read \"{}\": phar:// archives are not supported.",
                    String::from_utf8_lossy(&self.prefix)
                ),
            ));
        }

        if let Some(mut paths) = paths {
            paths.sort_by(|a, b| strnatcmp(&a.0, &b.0));
            for (path, rx) in paths {
                if !self.excluded_prefixes.is_empty() && self.under_excluded(&path, &prefix) {
                    continue;
                }

                let rx = rx.as_deref();
                let matches = |p: &[u8]| -> bool {
                    match rx {
                        None => true,
                        Some(rx) => preg_match(rx, substr(&normalize(p), prefix_len)),
                    }
                };

                if matches(&path) && fs::is_file(&path) {
                    out.push(path.clone());
                }
                if !fs::is_dir(&path) {
                    continue;
                }
                if self.for_exclusion && matches(&path) {
                    out.push(path.clone());
                    continue;
                }
                if !(self.recursive || rx.is_some()) || self.is_excluded(&normalize(&path)) {
                    continue;
                }
                let mut files = Vec::new();
                self.walk(&path, rx, prefix_len, &mut files)?;
                files.sort_by(|a, b| strnatcmp(a, b));
                out.extend(files);
            }

            return Ok(out);
        }

        // Finder, for the extended patterns glob() cannot take.
        let mut finder = Finder::new();
        let excluded = self.clone();
        let filter_prefix = prefix.clone();
        let rx = regex;
        finder
            .follow_links()
            .filter(
                move |info| {
                    let normalized = normalize(info.get_pathname());
                    if !preg_match(&rx, substr(&normalized, prefix_len))
                        || !fs::is_file(info.get_pathname())
                    {
                        return false;
                    }
                    !(!excluded.excluded_prefixes.is_empty()
                        && excluded.under_excluded(&normalized, &filter_prefix))
                },
                false,
            )
            .sort_by_name(false);
        finder
            .r#in(vec![prefix.clone()])
            .map_err(|e| Error::new(Class::Runtime, e.to_string()))?;
        for item in finder
            .get_iterator()
            .map_err(|e| Error::new(Class::Runtime, e.to_string()))?
        {
            let item = item.map_err(|e| Error::new(Class::Runtime, e.to_string()))?;
            out.push(item.get_pathname().to_vec());
        }
        Ok(out)
    }

    /// `RecursiveIteratorIterator(RecursiveCallbackFilterIterator(
    /// RecursiveDirectoryIterator($path, SKIP_DOTS | FOLLOW_SYMLINKS),
    /// …), LEAVES_ONLY)`, keeping files.
    fn walk(
        &self,
        dir: &[u8],
        rx: Option<&[u8]>,
        prefix_len: usize,
        out: &mut Vec<Vec<u8>>,
    ) -> Result<(), Error> {
        let names = fs::read_dir(dir).map_err(|reason| {
            Error::new(
                Class::UnexpectedValue,
                format!(
                    "RecursiveDirectoryIterator::__construct({}): Failed to open directory: {reason}",
                    String::from_utf8_lossy(dir)
                ),
            )
        })?;
        let base = dir.strip_suffix(b"/").unwrap_or(dir);
        for name in names {
            let mut path = base.to_vec();
            path.push(b'/');
            path.extend_from_slice(&name);
            let normalized = normalize(&path);
            let is_dir = fs::is_dir(&path);
            let accepted = !self.is_excluded(&normalized)
                && (rx.is_none_or(|rx| preg_match(rx, substr(&normalized, prefix_len))) || is_dir)
                && name.first() != Some(&b'.');
            if !accepted {
                continue;
            }
            if is_dir {
                self.walk(&path, rx, prefix_len, out)?;
            } else if fs::is_file(&path) {
                out.push(path);
            }
        }
        Ok(())
    }

    fn compute_hash(&self) -> Result<Vec<u8>, Error> {
        let mut data = Vec::new();
        for path in self.get_iterator()? {
            data.extend_from_slice(&path);
            data.push(b'\n');
        }
        Ok(xxh128(&data))
    }
}

impl ResourceInterface for GlobResource {
    fn class(&self) -> &str {
        CLASS
    }

    fn to_string(&self) -> Vec<u8> {
        let mut s = b"glob.".to_vec();
        s.extend_from_slice(&self.prefix);
        s.push(if self.recursive { b'1' } else { b'0' });
        s.extend_from_slice(&self.pattern);
        s.push(if self.for_exclusion { b'1' } else { b'0' });
        s.extend_from_slice(&php::implode(
            b"\0",
            self.excluded_prefixes.values().map(php::to_bytes),
        ));
        s
    }

    fn serialize(&self) -> Result<PhpArray, Error> {
        let hash = match self.hash.get() {
            Some(h) => h.clone(),
            None => {
                let h = self.compute_hash()?;
                let _ = self.hash.set(h.clone());
                h
            }
        };
        Ok(php_array![
            "prefix" => Value::string(self.prefix.clone()),
            "pattern" => Value::string(self.pattern.clone()),
            "recursive" => self.recursive,
            "hash" => Value::string(hash),
            "forExclusion" => self.for_exclusion,
            "excludedPrefixes" => Value::Array(self.excluded_prefixes.clone()),
        ])
    }

    fn as_self_checking(&self) -> Option<&dyn SelfCheckingResourceInterface> {
        Some(self)
    }
}

impl SelfCheckingResourceInterface for GlobResource {
    fn is_fresh(&self, _timestamp: i64) -> Result<bool, Error> {
        let hash = self.compute_hash()?;
        let first = self.hash.get_or_init(|| hash.clone());

        Ok(*first == hash)
    }
}

/// `str_replace('\\', '/', $path)`.
fn normalize(path: &[u8]) -> Vec<u8> {
    path.iter()
        .map(|&c| if c == b'\\' { b'/' } else { c })
        .collect()
}

/// `basename($path)` for `/`-separated paths.
fn basename(path: &[u8]) -> &[u8] {
    let trimmed = {
        let mut end = path.len();
        while end > 1 && path[end - 1] == b'/' {
            end -= 1;
        }
        &path[..end]
    };
    match trimmed.iter().rposition(|&c| c == b'/') {
        Some(at) if trimmed.len() > 1 => &trimmed[at + 1..],
        _ => trimmed,
    }
}

/// `substr($s, $start)`, empty past the end.
fn substr(s: &[u8], start: usize) -> &[u8] {
    s.get(start..).unwrap_or_default()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `preg_match($regex, $subject)` as a condition: an error is false.
fn preg_match(regex: &[u8], subject: &[u8]) -> bool {
    php_pcre::Pcre::new(regex)
        .ok()
        .and_then(|r| r.is_match(subject).ok())
        .unwrap_or(false)
}

/// `preg_match('/\\\\[,{}]/', $pattern)`: a backslash before `,`, `{`
/// or `}`.
fn escaped_brace(pattern: &[u8]) -> bool {
    pattern
        .windows(2)
        .any(|w| w[0] == b'\\' && matches!(w[1], b',' | b'{' | b'}'))
}

/// `preg_split('/\{([^{}]*+)\}/', $pattern, -1, PREG_SPLIT_DELIM_CAPTURE)`.
fn split_braces(pattern: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut last = 0;
    let mut i = 0;
    while i < pattern.len() {
        if pattern[i] == b'{' {
            let run = pattern[i + 1..]
                .iter()
                .take_while(|&&c| c != b'{' && c != b'}')
                .count();
            let close = i + 1 + run;
            if pattern.get(close) == Some(&b'}') {
                out.push(pattern[last..i].to_vec());
                out.push(pattern[i + 1..close].to_vec());
                last = close + 1;
                i = close + 1;
                continue;
            }
        }
        i += 1;
    }
    out.push(pattern[last..].to_vec());
    out
}

/// `expandGlob()`: the patterns a `{a,b}` pattern stands for.
fn expand_glob(pattern: &[u8]) -> Vec<Vec<u8>> {
    let segments = split_braces(pattern);
    let mut paths = vec![segments[0].clone()];
    let mut patterns: Vec<Vec<u8>> = Vec::new();

    let mut i = 1;
    while i < segments.len() {
        patterns = Vec::new();

        for s in segments[i].split(|&c| c == b',') {
            for p in &paths {
                let mut q = p.clone();
                q.extend_from_slice(s);
                q.extend_from_slice(&segments[1 + i]);
                patterns.push(q);
            }
        }

        paths = patterns.clone();
        i += 2;
    }

    let mut j: isize = 0;
    for (i, p) in patterns.iter().enumerate() {
        if p.contains(&b'{') {
            let p = expand_glob(p);
            let at = (i as isize + j).clamp(0, paths.len() as isize) as usize;
            let end = (at + 1).min(paths.len());
            let count = p.len() as isize;
            paths.splice(at..end, p);
            j += count - 1;
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_glob_nested() {
        let got: Vec<String> = expand_glob(b"/*{/*/*.txt,.x{m,n}l}")
            .into_iter()
            .map(|p| String::from_utf8_lossy(&p).into_owned())
            .collect();
        assert_eq!(got, ["/*/*/*.txt", "/*.xml", "/*/*/*.txt", "/*.xnl"]);
    }
}

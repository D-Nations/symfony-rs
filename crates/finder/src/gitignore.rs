//! Port of Symfony\Component\Finder\Gitignore.php.
//!
//! Upstream's `preg_*` calls run on its own patterns through pcre.rs.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! Authors: Michael Voříšek, Ahmed Abdou.
//!
//! Upstream: symfony/finder Gitignore.php

use std::sync::LazyLock;

use crate::{
    exception::Error,
    pcre::{Pcre, preg_quote},
};

pub(crate) struct Patterns {
    pub(crate) newline: Pcre,
    trailing_spaces: Pcre,
    escape: Pcre,
    bracket: Pcre,
    double_star: Pcre,
    star: Pcre,
    question: Pcre,
    range: Pcre,
}

pub(crate) static PATTERNS: LazyLock<Result<Patterns, Error>> = LazyLock::new(|| {
    Ok(Patterns {
        newline: Pcre::new(br"~\r\n?|\n~")?,
        trailing_spaces: Pcre::new(br"~(?<!\\)((?:\\\\)*+) +$~")?,
        escape: Pcre::new(br"~\\(.)~s")?,
        bracket: Pcre::new(br"~\\\[((?:\\[!^])?+)((?:\\\])?+[^\]]*)\\\]~")?,
        double_star: Pcre::new(br"~(?:(?:\\\*){2,}(/?))+~")?,
        star: Pcre::new(br"~\\\*~")?,
        question: Pcre::new(br"~\\\?~")?,
        range: Pcre::new(br"~(\\x[0-9A-F]{2}|\\?.)\\-(\\x[0-9A-F]{2}|\\?.)~")?,
    })
});

pub(crate) fn patterns() -> Result<&'static Patterns, Error> {
    PATTERNS.as_ref().map_err(Clone::clone)
}

fn matching(e: crate::pcre::MatchError) -> Error {
    Error::Runtime(e.to_string())
}

/// `preg_split($pattern, $subject)` (no limit, no flags), or with
/// `PREG_SPLIT_DELIM_CAPTURE` when `captures` is set.
pub(crate) fn preg_split(
    pattern: &Pcre,
    subject: &[u8],
    captures: bool,
) -> Result<Vec<Vec<u8>>, Error> {
    let mut parts = Vec::new();
    let mut last = 0;
    let mut pos = 0;
    while pos <= subject.len() {
        let Some(groups) = pattern.captures_from(subject, pos).map_err(matching)? else {
            break;
        };
        let Some((start, end)) = groups[0] else { break };
        if end == start {
            // an empty match splits nothing; try one byte further
            if start >= subject.len() {
                break;
            }
            pos = start + 1;
            continue;
        }
        parts.push(subject[last..start].to_vec());
        if captures {
            for (s, e) in groups.iter().skip(1).flatten() {
                parts.push(subject[*s..*e].to_vec());
            }
        }
        last = end;
        pos = end;
    }
    parts.push(subject[last..].to_vec());
    Ok(parts)
}

/// Gitignore matches against text.
pub struct Gitignore;

impl Gitignore {
    /// `Gitignore::toRegex($gitignoreFileContent)`: a regexp equivalent
    /// to the gitignore patterns
    /// (<https://git-scm.com/docs/gitignore#_pattern_format>).
    pub fn to_regex(content: &[u8]) -> Result<Vec<u8>, Error> {
        Self::build_regex(content, false)
    }

    /// `Gitignore::toRegexMatchingNegatedPatterns($gitignoreFileContent)`.
    pub fn to_regex_matching_negated_patterns(content: &[u8]) -> Result<Vec<u8>, Error> {
        Self::build_regex(content, true)
    }

    fn build_regex(content: &[u8], inverted: bool) -> Result<Vec<u8>, Error> {
        let p = patterns()?;
        let lines = preg_split(&p.newline, content, false)?;
        let mut res = Self::line_to_regex(b"")?;
        let mut alternatives: Vec<Vec<u8>> = Vec::new();
        for line in lines {
            // only a line starting with "#" is a comment, and only trailing spaces are stripped
            if line.starts_with(b"#") {
                continue;
            }
            let line = p.trailing_spaces.replace(&line, b"$1").map_err(matching)?;
            let (line, is_negative) = match line.strip_prefix(b"!") {
                Some(rest) => (rest.to_vec(), true),
                None => (line, false),
            };
            if !line.is_empty() {
                if is_negative ^ inverted {
                    // a negative pattern only cancels the patterns before it, so it opens a new nesting level
                    let mut r = b"(?!".to_vec();
                    r.extend(Self::line_to_regex(&line)?);
                    r.extend_from_slice(b"$)");
                    r.extend(Self::alternate(res, &mut alternatives));
                    res = r;
                } else {
                    alternatives.push(Self::line_to_regex(&line)?);
                }
            }
        }
        let mut out = b"~^(?:".to_vec();
        out.extend(Self::alternate(res, &mut alternatives));
        out.extend_from_slice(b")~s");
        Ok(out)
    }

    /// Consecutive patterns share a single group, so deep nesting stays
    /// proportional to the number of negative patterns.
    fn alternate(res: Vec<u8>, alternatives: &mut Vec<Vec<u8>>) -> Vec<u8> {
        if alternatives.is_empty() {
            return res;
        }
        let mut regex = b"(?:".to_vec();
        regex.extend(res);
        for a in alternatives.drain(..) {
            regex.push(b'|');
            regex.extend(a);
        }
        regex.push(b')');
        regex
    }

    fn line_to_regex(line: &[u8]) -> Result<Vec<u8>, Error> {
        if line.is_empty() {
            return Ok(b"$f".to_vec()); // always false
        }
        let p = patterns()?;
        let mut line = line;
        let slash = line.iter().position(|&c| c == b'/');
        let is_absolute = match slash {
            Some(pos) if line.len() - 1 != pos => {
                if pos == 0 {
                    line = &line[1..];
                }
                true
            }
            _ => false,
        };
        let parts = preg_split(&p.escape, line, true)?;
        if parts.last().is_some_and(|last| last.ends_with(b"\\")) {
            return Ok(b"$f".to_vec()); // a trailing backslash never matches
        }
        let mut regex = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            // escaped characters are written in hex so that the replacements below leave them alone
            if i % 2 == 1 {
                regex.extend(format!(r"\x{:02X}", part.first().copied().unwrap_or(0)).bytes());
            } else {
                regex.extend(preg_quote(part, Some(b'~')));
            }
        }
        let mut failure = None;
        let regex = p
            .bracket
            .replace_callback(&regex, |m| {
                let mut out = if m.get(1).copied().flatten().is_some_and(|g| !g.is_empty()) {
                    b"(?!/)[^".to_vec()
                } else {
                    b"[".to_vec()
                };
                match Self::ranges_to_regex(m.get(2).copied().flatten().unwrap_or_default()) {
                    Ok(r) => out.extend(r),
                    Err(e) => failure = Some(e),
                }
                out.push(b']');
                out
            })
            .map_err(matching)?;
        if let Some(e) = failure {
            return Err(e);
        }
        let regex = p
            .double_star
            .replace(&regex, b"(?:(?:(?!//).(?<!//))+$1)?")
            .map_err(matching)?;
        let regex = p.star.replace(&regex, b"[^/]*").map_err(matching)?;
        let regex = p.question.replace(&regex, b"[^/]").map_err(matching)?;
        let mut out = if is_absolute {
            Vec::new()
        } else {
            b"(?:[^/]+/)*".to_vec()
        };
        out.extend(regex);
        if !line.ends_with(b"/") {
            out.extend_from_slice(b"(?:$|/)");
        }
        Ok(out)
    }

    /// Unescapes the "-" of each range. A range that ends before its
    /// start only matches its start, as in git.
    fn ranges_to_regex(class: &[u8]) -> Result<Vec<u8>, Error> {
        let p = patterns()?;
        let code = |c: &[u8]| -> u32 {
            if c.len() == 4 {
                std::str::from_utf8(&c[2..])
                    .ok()
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .unwrap_or(0)
            } else {
                c.last().copied().map_or(0, u32::from)
            }
        };
        p.range
            .replace_callback(class, |m| {
                let (a, b) = (
                    m.get(1).copied().flatten().unwrap_or_default(),
                    m.get(2).copied().flatten().unwrap_or_default(),
                );
                if code(a) <= code(b) {
                    let mut out = a.to_vec();
                    out.push(b'-');
                    out.extend_from_slice(b);
                    out
                } else {
                    a.to_vec()
                }
            })
            .map_err(matching)
    }
}

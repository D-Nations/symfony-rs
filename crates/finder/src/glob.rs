//! Port of Symfony\Component\Finder\Glob.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! Based on Richard Clamp's Perl Text::Glob.
//!
//! Upstream: symfony/finder Glob.php

use crate::pcre::preg_quote;

/// Glob matches globbing patterns against text.
pub struct Glob;

const LEADING_DOT: &[u8] = br"(?=[^\.])";

impl Glob {
    /// `Glob::toRegex($glob, $strictLeadingDot, $strictWildcardSlash,
    /// $delimiter)`; an empty delimiter is `''`.
    pub fn to_regex(
        glob: &[u8],
        strict_leading_dot: bool,
        strict_wildcard_slash: bool,
        delimiter: Option<u8>,
    ) -> Vec<u8> {
        let mut first_byte = true;
        let mut escaping = false;
        let mut curlies: Vec<Vec<usize>> = Vec::new();
        let mut bracket_scan_end = 0usize;
        let mut regex: Vec<u8> = Vec::new();

        let unanchored = glob.starts_with(b"**/");
        let glob: Vec<u8> = if unanchored {
            let mut g = b"/".to_vec();
            g.extend_from_slice(glob);
            g
        } else {
            glob.to_vec()
        };
        let size = glob.len();
        let mut i = 0usize;
        while i < size {
            let mut car: Vec<u8> = vec![glob[i]];
            if first_byte && strict_leading_dot && glob[i] != b'.' {
                regex.extend_from_slice(LEADING_DOT);
            }
            first_byte = glob[i] == b'/';
            if first_byte
                && strict_wildcard_slash
                && i + 2 < size
                && glob[i + 1] == b'*'
                && glob[i + 2] == b'*'
                && (i + 3 >= size || glob[i + 3] == b'/')
            {
                let mut c = br"[^/]++/".to_vec();
                if i + 3 >= size {
                    c.push(b'?');
                }
                if strict_leading_dot {
                    let mut d = LEADING_DOT.to_vec();
                    d.extend_from_slice(&c);
                    c = d;
                }
                let mut d = b"/(?:".to_vec();
                d.extend_from_slice(&c);
                d.extend_from_slice(b")*");
                c = d;
                i += 2 + usize::from(i + 3 < size);
                if delimiter == Some(b'/') {
                    c = replace(&c, b"/", br"\/");
                }
                car = c;
            }
            let single = if car.len() == 1 { Some(car[0]) } else { None };
            let class = if single == Some(b'[') && !escaping && delimiter != Some(b'[') {
                Self::bracket_expression_to_regex(&glob, &mut i, delimiter, &mut bracket_scan_end)
            } else {
                None
            };
            match single {
                Some(c)
                    if Some(c) == delimiter
                        || matches!(c, b'.' | b'(' | b')' | b'|' | b'+' | b'^' | b'$') =>
                {
                    regex.push(b'\\');
                    regex.push(c);
                }
                Some(b'[') if class.is_some() => {
                    regex.extend(class.unwrap_or_default());
                }
                Some(b'*') => {
                    if escaping || strict_wildcard_slash || !regex.ends_with(b".*") {
                        // ".*.*" matches the same as ".*" but backtracks quadratically
                        regex.extend_from_slice(if escaping {
                            br"\*"
                        } else if strict_wildcard_slash {
                            b"[^/]*"
                        } else {
                            b".*"
                        });
                    }
                }
                Some(b'?') => regex.extend_from_slice(if escaping {
                    br"\?"
                } else if strict_wildcard_slash {
                    b"[^/]"
                } else {
                    b"."
                }),
                Some(b'{') => {
                    if !escaping {
                        curlies.push(vec![regex.len()]);
                    }
                    regex.extend_from_slice(if escaping { br"\{" } else { b"(" });
                }
                Some(b'}') if !curlies.is_empty() => {
                    regex.extend_from_slice(if escaping { b"}" } else { b")" });
                    if !escaping {
                        curlies.pop();
                    }
                }
                Some(b',') if !curlies.is_empty() => {
                    if !escaping {
                        let len = regex.len();
                        if let Some(last) = curlies.last_mut() {
                            last.push(len);
                        }
                    }
                    regex.extend_from_slice(if escaping { b"," } else { b"|" });
                }
                Some(b'\\') => {
                    if escaping {
                        regex.extend_from_slice(br"\\");
                        escaping = false;
                    } else {
                        escaping = true;
                    }
                    i += 1;
                    continue;
                }
                Some(c @ (b'[' | b']')) if escaping => {
                    regex.push(b'\\');
                    regex.push(c);
                }
                _ => regex.extend_from_slice(&car),
            }
            escaping = false;
            i += 1;
        }

        // A "{" that no "}" closes is a literal character, and so are the "," it contains
        let mut prefix = Vec::new();
        let mut start = 0usize;
        for offset in curlies.into_iter().flatten() {
            prefix.extend_from_slice(&regex[start..offset]);
            prefix.extend_from_slice(if regex[offset] == b'(' { br"\{" } else { b"," });
            start = offset + 1;
        }
        prefix.extend_from_slice(&regex[start.min(regex.len())..]);
        let mut regex = prefix;

        if unanchored {
            let at = 1
                + usize::from(delimiter == Some(b'/'))
                + if strict_leading_dot {
                    LEADING_DOT.len()
                } else {
                    0
                };
            let at = at.min(regex.len());
            regex.insert(at, b'?');
        }

        let mut out = Vec::with_capacity(regex.len() + 4);
        out.extend(delimiter);
        out.push(b'^');
        out.extend(regex);
        out.push(b'$');
        out.extend(delimiter);
        out
    }

    /// Returns None when a "/" comes before the closing "]", and "\[" when
    /// no "]" closes the "[" at position `i`.
    ///
    /// `scan_end` is where the last scan stopped without finding a "]":
    /// every "[" before that position gets the same result.
    fn bracket_expression_to_regex(
        glob: &[u8],
        i: &mut usize,
        delimiter: Option<u8>,
        scan_end: &mut usize,
    ) -> Option<Vec<u8>> {
        let size = glob.len();
        if *i < *scan_end {
            return if *scan_end < size {
                None
            } else {
                Some(br"\[".to_vec())
            };
        }
        let mut class = b"[".to_vec();
        let mut j = *i + 1;
        while j < size {
            let car = glob[j];
            if car == b']' && j > *i + 1 {
                *i = j;
                class.push(b']');
                return Some(class);
            }
            if car == b'/' || (car == b'\\' && glob.get(j + 1) == Some(&b'/')) {
                *scan_end = j;
                return None;
            }
            if car == b'['
                && let Some(len) = posix_class_at(glob, j)
            {
                class.extend_from_slice(&glob[j..j + len]);
                j += len;
                continue;
            }
            if car == b'\\' {
                j += 1;
                if j >= size {
                    break;
                }
                class.extend(preg_quote(&glob[j..=j], delimiter));
            } else if Some(car) == delimiter || b".()|+^$".contains(&car) {
                class.push(b'\\');
                class.push(car);
            } else {
                class.push(car);
            }
            j += 1;
        }
        *scan_end = size;
        Some(br"\[".to_vec())
    }
}

/// The length of `[:name:]` (lower-case letters) at `j`, as
/// `/\[:[a-z]++:\]/A` matches it.
fn posix_class_at(glob: &[u8], j: usize) -> Option<usize> {
    let rest = &glob[j..];
    if !rest.starts_with(b"[:") {
        return None;
    }
    let letters = rest[2..]
        .iter()
        .take_while(|c| c.is_ascii_lowercase())
        .count();
    if letters == 0 || !rest[2 + letters..].starts_with(b":]") {
        return None;
    }
    Some(letters + 4)
}

fn replace(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

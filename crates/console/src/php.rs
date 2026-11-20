//! PHP built-ins Console calls that no other crate has: `strip_tags()`
//! (for `OUTPUT_PLAIN`).
//!
//! Written from the PHP manual (no php-src, PLAN.md decision 8) and checked
//! against 6,137 recorded calls (`tests/data/strip_tags.json`,
//! `scripts/record.sh console`). The manual: HTML and PHP tags and
//! comments are removed. As observed:
//! - a `<` followed by whitespace opens nothing, in a tag or not;
//! - in a tag, `<` nests and `>` closes one level, and quotes hide both;
//!   once a nested `<` is followed by `!`, the rest of the tag is a
//!   declaration, in which `<` no longer nests;
//! - `<!--` runs to `-->`;
//! - `<?` at the top level runs to `?>` outside quotes, the opening `?`
//!   included (`<?>` is complete); inside a tag, quotes do not count, the
//!   first `>` is passed over and the next `?>` ends the whole tag, and in
//!   a nested tag it never ends;
//! - an unclosed tag runs to the end.
//!
//! Upstream: none

/// `strip_tags($string)` with no allowed tags.
pub fn strip_tags(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' || opens_nothing(b, i) {
            out.push(b[i]);
            i += 1;
            continue;
        }
        i = skip_tag(b, i);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn opens_nothing(b: &[u8], at: usize) -> bool {
    b.get(at + 1).is_some_and(u8::is_ascii_whitespace)
}

/// From the `<` at `start`, the index after the tag (or the end).
fn skip_tag(b: &[u8], start: usize) -> usize {
    if b[start..].starts_with(b"<!--") {
        return find_after(b, start + 4, b"-->");
    }
    if b.get(start + 1) == Some(&b'?') {
        return skip_php(b, start + 1);
    }
    let mut declaration = b.get(start + 1) == Some(&b'!');
    let mut depth = 1;
    let mut quote: Option<u8> = None;
    let mut i = start + 1;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'<' if !declaration && !opens_nothing(b, i) => {
                    if b.get(i + 1) == Some(&b'?') {
                        return skip_nested_php(b, i + 2, depth);
                    }
                    depth += 1;
                    if b.get(i + 1) == Some(&b'!') {
                        declaration = true;
                    }
                }
                b'>' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    b.len()
}

/// A PHP section from `from`: to the first `?>` outside quotes.
fn skip_php(b: &[u8], from: usize) -> usize {
    let mut quote: Option<u8> = None;
    let mut i = from;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'?' if b.get(i + 1) == Some(&b'>') => return i + 2,
                _ => {}
            },
        }
        i += 1;
    }
    b.len()
}

/// A PHP section opened inside a tag, from after its `<?`: quotes do not
/// count, the first `>` is passed over, and the next `?>` ends the whole
/// tag; inside a nested tag (depth 2 or more) it never ends.
fn skip_nested_php(b: &[u8], from: usize, depth: usize) -> usize {
    if depth > 1 {
        return b.len();
    }
    let Some(first) = b[from.min(b.len())..].iter().position(|&c| c == b'>') else {
        return b.len();
    };
    find_after(b, from + first + 1, b"?>")
}

fn find_after(b: &[u8], from: usize, end: &[u8]) -> usize {
    b[from.min(b.len())..]
        .windows(end.len())
        .position(|w| w == end)
        .map_or(b.len(), |p| from + p + end.len())
}

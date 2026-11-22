//! Whether libxml accepts a string as a URI reference (RFC 3986), which
//! decides its namespace and entity-system-identifier diagnostics
//! ("is not a valid URI", "is not absolute", "Invalid URI").
//!
//! Upstream: none

/// The parse result libxml's diagnostics need.
pub(crate) struct Uri {
    pub(crate) has_scheme: bool,
    pub(crate) has_fragment: bool,
}

/// `URI-reference = URI / relative-ref`; `None` when neither matches the
/// whole string.
pub(crate) fn parse(s: &str) -> Option<Uri> {
    let b = s.as_bytes();
    if let Some(u) = absolute(b) {
        return Some(u);
    }
    relative(b)
}

fn absolute(b: &[u8]) -> Option<Uri> {
    let mut i = 0;
    if !b.first()?.is_ascii_alphabetic() {
        return None;
    }
    while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'+' | b'-' | b'.')) {
        i += 1;
    }
    if b.get(i) != Some(&b':') {
        return None;
    }
    i += 1;
    let i = hier_part(b, i)?;
    let (i, has_fragment) = query_fragment(b, i)?;
    (i == b.len()).then_some(Uri {
        has_scheme: true,
        has_fragment,
    })
}

fn relative(b: &[u8]) -> Option<Uri> {
    let i = if b.starts_with(b"//") {
        let i = authority(b, 2)?;
        segments(b, i, true)
    } else if b.first() == Some(&b'/') {
        segments(b, 0, true)
    } else {
        // path-noscheme: the first segment has no colon.
        let mut i = 0;
        while i < b.len() && b[i] != b'/' && b[i] != b'?' && b[i] != b'#' {
            if b[i] == b':' {
                return None;
            }
            i = pchar(b, i)?;
        }
        segments(b, i, true)
    };
    let (i, has_fragment) = query_fragment(b, i)?;
    (i == b.len()).then_some(Uri {
        has_scheme: false,
        has_fragment,
    })
}

fn hier_part(b: &[u8], i: usize) -> Option<usize> {
    if b[i..].starts_with(b"//") {
        let i = authority(b, i + 2)?;
        Some(segments(b, i, true))
    } else {
        Some(segments(b, i, true))
    }
}

/// `*( "/" segment )` with leading segment characters allowed.
fn segments(b: &[u8], mut i: usize, _abempty: bool) -> usize {
    loop {
        match b.get(i) {
            Some(b'/') => i += 1,
            Some(_) => match pchar(b, i) {
                Some(n) => i = n,
                None => return i,
            },
            None => return i,
        }
    }
}

fn authority(b: &[u8], start: usize) -> Option<usize> {
    // [ userinfo "@" ] host [ ":" port ]
    let mut i = start;
    let mut j = start;
    while j < b.len() && (unreserved(b[j]) || sub_delim(b[j]) || b[j] == b':' || b[j] == b'%') {
        if b[j] == b'%' {
            pct(b, j)?;
            j += 3;
        } else {
            j += 1;
        }
    }
    if b.get(j) == Some(&b'@') {
        i = j + 1;
    }
    if b.get(i) == Some(&b'[') {
        while i < b.len() && b[i] != b']' {
            i += 1;
        }
        if i == b.len() {
            return None;
        }
        i += 1;
    } else {
        while i < b.len() && (unreserved(b[i]) || sub_delim(b[i]) || b[i] == b'%') {
            if b[i] == b'%' {
                pct(b, i)?;
                i += 3;
            } else {
                i += 1;
            }
        }
    }
    if b.get(i) == Some(&b':') {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    }
    Some(i)
}

fn query_fragment(b: &[u8], mut i: usize) -> Option<(usize, bool)> {
    if b.get(i) == Some(&b'?') {
        i += 1;
        while i < b.len() && b[i] != b'#' {
            if b[i] == b'/' || b[i] == b'?' {
                i += 1;
            } else {
                i = pchar(b, i)?;
            }
        }
    }
    let mut has_fragment = false;
    if b.get(i) == Some(&b'#') {
        has_fragment = true;
        i += 1;
        while i < b.len() {
            if b[i] == b'/' || b[i] == b'?' {
                i += 1;
            } else {
                i = pchar(b, i)?;
            }
        }
    }
    Some((i, has_fragment))
}

/// One `pchar` at `i`; the index after it.
fn pchar(b: &[u8], i: usize) -> Option<usize> {
    let c = *b.get(i)?;
    if unreserved(c) || sub_delim(c) || c == b':' || c == b'@' {
        Some(i + 1)
    } else if c == b'%' {
        pct(b, i)?;
        Some(i + 3)
    } else {
        None
    }
}

fn pct(b: &[u8], i: usize) -> Option<()> {
    let ok = b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
        && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
    ok.then_some(())
}

fn unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~')
}

fn sub_delim(c: u8) -> bool {
    matches!(
        c,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn recorded_namespace_and_system_identifiers() {
        for ok in [
            "urn:x",
            "http://example.com/schema",
            "urn:oasis:names:tc:xliff:document:1.2",
            "relative",
            "u1",
            "file:///etc/debian_version",
            "http://127.0.0.1:9/x",
            "php://filter/read=convert.base64-encode/resource=/etc/debian_version",
        ] {
            assert!(parse(ok).is_some(), "{ok}");
        }
        for bad in [
            "a b",
            "php://filter/read=convert.base64-encode/resource={{ resource }}",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
        assert!(!parse("relative").is_some_and(|u| u.has_scheme));
    }
}

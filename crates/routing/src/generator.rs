//! Ports the existing subset of Symfony Routing Generator/UrlGenerator.php.
//!
//! Upstream: symfony/routing Generator/UrlGenerator.php Tests/Generator/UrlGeneratorTest.php
use symfony_http_foundation::query::QueryValue;

/// Characters Symfony's `UrlGenerator` leaves unencoded after
/// `rawurlencode()` (`$decodedChars`).
const DECODED: [(&str, &str); 11] = [
    ("%252F", "%2F"),
    ("%2F", "/"),
    ("%40", "@"),
    ("%3A", ":"),
    ("%3B", ";"),
    ("%2C", ","),
    ("%3D", "="),
    ("%2B", "+"),
    ("%21", "!"),
    ("%2A", "*"),
    ("%7C", "|"),
];

/// `UrlGenerator::QUERY_FRAGMENT_DECODED`: unlike in a path, `+`, `=`
/// and `|` stay encoded, and `?` can be decoded.
const QUERY_DECODED: [(&str, &str); 9] = [
    ("%252F", "%2F"),
    ("%2F", "/"),
    ("%3F", "?"),
    ("%40", "@"),
    ("%3A", ":"),
    ("%21", "!"),
    ("%3B", ";"),
    ("%2C", ","),
    ("%2A", "*"),
];

/// PHP's `rawurlencode()` (RFC 3986): everything but `A-Za-z0-9-._~`.
pub fn rawurlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn symfony_decode(mut encoded: &str, replacements: &[(&str, &str)]) -> String {
    // PHP's strtr() substitutes once, without decoding its own output.
    // The longer %252F rule must precede the three-byte rules.
    let mut out = String::with_capacity(encoded.len());
    while !encoded.is_empty() {
        if let Some((code, decoded)) = replacements
            .iter()
            .find(|(code, _)| encoded.starts_with(code))
        {
            out.push_str(decoded);
            encoded = &encoded[code.len()..];
        } else if let Some(c) = encoded.chars().next() {
            out.push(c);
            encoded = &encoded[c.len_utf8()..];
        }
    }
    out
}

/// A path parameter, encoded as Symfony does.
pub fn encode_path(segment: &str) -> String {
    symfony_decode(&rawurlencode(segment), &DECODED)
}

/// `UrlGenerator::doGenerate`'s last path step: every whole segment `.` or
/// `..` of the encoded path becomes `%2E` or `%2E%2E`, so a user agent's
/// RFC 3986 dot-segment removal cannot move the URL off the route.
/// Callers apply it to the complete path (without the base URL), after
/// [`encode_path`] has encoded each parameter: a parameter may contain `/`.
/// Each segment is checked on its own, so chained `../../..` are all
/// encoded (GHSA-h5x3-xfc9-m39h fixed an every-other-segment skip).
pub fn encode_dot_segments(url: &str) -> String {
    if !url.contains("/.") {
        return url.to_owned();
    }
    url.split('/')
        .map(|segment| match segment {
            "." => "%2E",
            ".." => "%2E%2E",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `http_build_query($params, '', '&', PHP_QUERY_RFC3986)`, then Symfony's
/// partial decoding (`UrlGenerator::doGenerate`). Nested arrays become
/// `key[sub]=value` with the brackets encoded.
pub fn build_query(params: &[(String, QueryValue)]) -> String {
    let mut pairs = Vec::new();
    for (name, value) in params {
        push_pairs(&mut pairs, &rawurlencode(name), value);
    }
    symfony_decode(&pairs.join("&"), &QUERY_DECODED)
}

fn push_pairs(pairs: &mut Vec<String>, encoded_name: &str, value: &QueryValue) {
    match value {
        QueryValue::Str(text) => pairs.push(format!("{encoded_name}={}", rawurlencode(text))),
        QueryValue::Map(entries) => {
            for (key, nested) in entries {
                let name = format!("{encoded_name}%5B{}%5D", rawurlencode(key));
                push_pairs(pairs, &name, nested);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The typed generator's dot-segment step, on the paths of
    /// UrlGeneratorTest::testEncodingOfRelativePathSegments (ported in full
    /// in tests/url_generator_test.rs). The expected paths drop upstream's
    /// `/app.php` base URL, which is prepended after this step.
    #[test]
    fn encodes_relative_path_segments() {
        assert_eq!(
            encode_dot_segments("/dir/../dir/.."),
            "/dir/%2E%2E/dir/%2E%2E"
        );
        assert_eq!(encode_dot_segments("/dir/./dir/."), "/dir/%2E/dir/%2E");
        assert_eq!(
            encode_dot_segments("/a./.a/a../..a/..."),
            "/a./.a/a../..a/..."
        );
    }

    /// GHSA-h5x3-xfc9-m39h (CVE-2026-48784): chained dot segments in a
    /// parameter that may contain `/` (requirement `.+`), as in
    /// UrlGeneratorTest::testEncodingOfChainedRelativePathSegments (ported
    /// in full in tests/url_generator_test.rs).
    #[test]
    fn encodes_chained_relative_path_segments() {
        let between = |path: &str| encode_dot_segments(&format!("/foo/{}/bar", encode_path(path)));
        assert_eq!(between("../../.."), "/foo/%2E%2E/%2E%2E/%2E%2E/bar");
        assert_eq!(between("././."), "/foo/%2E/%2E/%2E/bar");
        assert_eq!(between("../././.."), "/foo/%2E%2E/%2E/%2E/%2E%2E/bar");

        let last = |path: &str| encode_dot_segments(&format!("/foo/{}", encode_path(path)));
        assert_eq!(last("../../.."), "/foo/%2E%2E/%2E%2E/%2E%2E");
        assert_eq!(last("././."), "/foo/%2E/%2E/%2E");
    }
}

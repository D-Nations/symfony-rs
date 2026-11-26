//! The matchers of `Symfony\Component\HttpFoundation\RequestMatcher`, and
//! what several of them share: `preg_split('/\s*,\s*/', $list)` of their
//! constructor's lists, and a `{...}`-delimited pattern compiled once.
//!
//! Upstream: none

use std::fmt;

use php_pcre::Pcre;

pub mod attributes_request_matcher;
pub mod expression_request_matcher;
pub mod header_request_matcher;
pub mod host_request_matcher;
pub mod ips_request_matcher;
pub mod is_json_request_matcher;
pub mod method_request_matcher;
pub mod path_request_matcher;
pub mod port_request_matcher;
pub mod query_parameter_request_matcher;
pub mod scheme_request_matcher;

/// PCRE's `\s`: space, `\t`, `\n`, `\v`, `\f`, `\r`.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `preg_split('/\s*,\s*/', $text)`: the parts between commas, without
/// the spaces next to each comma. An empty text is one empty part.
pub(crate) fn split_list(text: &[u8]) -> Vec<Vec<u8>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < text.len() {
        if text[i] == b',' {
            // The match starts at the spaces before the comma, but not
            // before the end of the previous match.
            let mut left = i;
            while left > start && is_space(text[left - 1]) {
                left -= 1;
            }
            let mut right = i + 1;
            while right < text.len() && is_space(text[right]) {
                right += 1;
            }
            parts.push(text[start..left].to_vec());
            start = right;
            i = right;
        } else {
            i += 1;
        }
    }
    parts.push(text[start..].to_vec());
    parts
}

/// `array_reduce(array_map($case, (array) $list), fn => array_merge(...,
/// preg_split(...)), [])`.
pub(crate) fn split_all<T: AsRef<[u8]>>(
    list: impl IntoIterator<Item = T>,
    case: fn(&[u8]) -> Vec<u8>,
) -> Vec<Vec<u8>> {
    list.into_iter()
        .flat_map(|item| split_list(&case(item.as_ref())))
        .collect()
}

/// `preg_match('{'.$regexp.'}'.$flags, $subject)` as a bool: a pattern
/// that does not compile, or a match that fails, is `false`, as PHP's
/// `false` with a warning.
pub(crate) struct Pattern {
    source: Vec<u8>,
    compiled: Option<Pcre>,
}

impl fmt::Debug for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Pattern")
            .field(&String::from_utf8_lossy(&self.source))
            .finish()
    }
}

impl Pattern {
    pub(crate) fn new(regexp: &[u8], flags: &[u8]) -> Self {
        let source = [b"{", regexp, b"}", flags].concat();
        let compiled = Pcre::new(&source).ok();
        Self { source, compiled }
    }

    pub(crate) fn is_match(&self, subject: &[u8]) -> bool {
        self.compiled
            .as_ref()
            .is_some_and(|p| p.is_match(subject).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::split_list;

    fn split(text: &str) -> Vec<String> {
        split_list(text.as_bytes())
            .into_iter()
            .map(|p| String::from_utf8_lossy(&p).into_owned())
            .collect()
    }

    #[test]
    fn splits_as_preg_split() {
        assert_eq!(split("a, b ,c"), ["a", "b", "c"]);
        assert_eq!(split(" a , b "), [" a", "b "]);
        assert_eq!(split(""), [""]);
        assert_eq!(split(",,"), ["", "", ""]);
        assert_eq!(split("a\t,\nb"), ["a", "b"]);
        assert_eq!(split("a  x, b"), ["a  x", "b"]);
    }
}

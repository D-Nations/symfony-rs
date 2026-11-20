//! The request's cookies, read the way PHP fills `$_COOKIE`.
//!
//! Written from the PHP manual ("Variables From External Sources": cookie
//! names are no longer URL-decoded) and PHP 8.4.27's recorded answers
//! (`tests/data/cookie_vectors.json`, `scripts/record.sh request-vars`); no
//! PHP source code is used (PLAN.md decision 8). Pairs are separated by
//! `;` only; spaces and tabs before a name are skipped; the value is
//! URL-decoded with `+` kept; names are converted like query names
//! (`query`), so `a[b]` is an array; a name without brackets that is
//! already present keeps its first value; only the first `max_input_vars`
//! non-empty pairs are read.
//!
//! Upstream: none

use super::query::{Decode, Duplicate, MAX_INPUT_VARS, Query, QueryValue, lookup};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cookies(Vec<(String, QueryValue)>);

impl Cookies {
    /// The `Cookie` header values of one request, in order, read as one
    /// list (as a server joining them with `; ` would pass them to PHP).
    pub fn parse<'a>(headers: impl IntoIterator<Item = &'a str>) -> Self {
        let mut cookies = Query::default();
        let pairs = headers
            .into_iter()
            .flat_map(|header| header.as_bytes().split(|&b| b == b';'))
            .map(|pair| {
                let start = pair.iter().position(|&b| b != b' ' && b != b'\t');
                start.map_or(&b""[..], |start| &pair[start..])
            })
            .filter(|pair| !pair.is_empty());
        for pair in pairs.take(MAX_INPUT_VARS) {
            cookies.register_pair(pair, Decode::ValueOnly, Duplicate::KeepFirst);
        }
        Self(cookies.into_entries())
    }

    /// `$request->cookies->get($name)` for a string cookie; `None` when it
    /// is absent or an array.
    pub fn get(&self, name: &str) -> Option<&str> {
        match lookup(&self.0, name) {
            Some(QueryValue::Str(value)) => Some(value),
            _ => None,
        }
    }

    pub fn has(&self, name: &str) -> bool {
        lookup(&self.0, name).is_some()
    }

    /// The string cookies, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().filter_map(|(name, value)| match value {
            QueryValue::Str(value) => Some((name.as_str(), value.as_str())),
            QueryValue::Map(_) => None,
        })
    }

    /// Every cookie, arrays included, as `$request->cookies->all()`.
    pub fn all(&self) -> &[(String, QueryValue)] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_php() {
        let cookies = Cookies::parse([
            "PHPSESSID=abc; REMEMBERME=%3Aam9obl91c2Vy%3A1; a.b=1; PHPSESSID=second",
            "x y=+",
        ]);
        assert_eq!(cookies.get("PHPSESSID"), Some("abc"));
        assert_eq!(cookies.get("REMEMBERME"), Some(":am9obl91c2Vy:1"));
        assert_eq!(cookies.get("a_b"), Some("1"));
        // PHP 8.4.27 keeps `+` in a cookie value (recorded: `cookie-12`); the
        // earlier implementation made it a space.
        assert_eq!(cookies.get("x_y"), Some("+"));
        assert!(!cookies.has("missing"));
    }
}

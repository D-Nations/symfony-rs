//! The request's cookies, read the way PHP fills `$_COOKIE`.
//!
//! Ports PHP's cookie parsing (`php_default_treat_data` with
//! `PARSE_COOKIE`): pairs are separated by `;`, leading spaces are skipped,
//! the value is URL-decoded, `.` and ` ` in a name become `_`, and when a
//! name appears twice the first value wins.
//!
//! Upstream: php-src main/php_variables.c

use super::query::urldecode;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cookies(Vec<(String, String)>);

impl Cookies {
    /// Parses every `Cookie` header of the request.
    pub fn parse<'a>(headers: impl IntoIterator<Item = &'a str>) -> Self {
        let mut cookies: Vec<(String, String)> = Vec::new();
        for header in headers {
            for pair in header.split(';') {
                let pair = pair.trim_start_matches([' ', '\t']);
                let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                if name.is_empty() {
                    continue;
                }
                let name = name.replace(['.', ' '], "_");
                if cookies.iter().any(|(existing, _)| *existing == name) {
                    continue;
                }
                cookies.push((name, urldecode(value)));
            }
        }
        Self(cookies)
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(existing, _)| existing == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
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
        assert_eq!(cookies.get("x_y"), Some(" "));
        assert!(!cookies.has("missing"));
    }
}

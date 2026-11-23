//! The request's query string and URL-encoded form body, read the way PHP
//! fills `$_GET` and `$_POST`, and Symfony's `InputBag::get()`.
//!
//! The original depends on PHP's rules here: `?a.b=1` arrives as `a_b`,
//! `?tag[]=x` makes `tag` an array (and `->get('tag')` a 400 error), and the
//! RSS link copies the whole parsed query. Written from the PHP manual
//! ("Variables From External Sources", "Arrays") and from PHP 8.4.27's
//! recorded answers (`tests/data/query_vectors.json`, recorded by
//! `scripts/record.sh request-vars`); no PHP source code is used (PLAN.md
//! decision 8). `cookies` and `multipart` register their names here too.
//! PHP's naming rules themselves live in `php_value::vars`, shared with
//! `parse_str()`.
//!
//! Upstream: symfony/http-foundation InputBag.php

use php_value::vars;
pub(crate) use php_value::vars::Duplicate;
use php_value::{Key, PhpArray, PhpString, Value, url};

/// A parsed query value: a string, or an array (PHP arrays keep insertion
/// order; keys are strings here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryValue {
    Str(String),
    Map(Vec<(String, QueryValue)>),
}

/// PHP's `max_input_vars` default.
pub(crate) const MAX_INPUT_VARS: usize = php_value::vars::MAX_INPUT_VARS;

/// `InputBag::get()` found an array where a string was expected; Symfony
/// answers 400 Bad Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonScalarValue(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// The variables as PHP stores them (`php_value::vars`).
    vars: Vars,
    /// `vars` as text, filled by the constructors.
    entries: Vec<(String, QueryValue)>,
    /// Top-level names whose value, as sent, was not UTF-8. PHP keeps the
    /// bytes; here they are replaced, so the places where the original
    /// fails on such bytes (`u()` in `findBySearchQuery()`) can ask.
    not_utf8: Vec<String>,
}

/// The variables, compared by their text form so `Query` keeps deriving
/// `PartialEq`.
#[derive(Debug, Clone, Default)]
struct Vars(PhpArray);

impl PartialEq for Vars {
    fn eq(&self, other: &Self) -> bool {
        to_entries(&self.0) == to_entries(&other.0)
    }
}

impl Eq for Vars {}

impl Query {
    /// A query string, as `$_GET`: pairs are separated by `&`, empty
    /// pairs are skipped, and only the first `max_input_vars` other pairs
    /// are read.
    pub fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        let pairs = raw
            .as_bytes()
            .split(|&b| b == b'&')
            .filter(|pair| !pair.is_empty());
        for pair in pairs.take(MAX_INPUT_VARS) {
            query.register_pair(pair, Decode::Plus, Duplicate::Replace);
        }
        query.finish()
    }

    /// An `application/x-www-form-urlencoded` body, as `$_POST`. Unlike a
    /// query string, PHP counts empty pairs too, and reads one pair more:
    /// `max_input_vars + 1` (recorded: `vars-1001`, `vars-1000-empty-pairs`).
    pub fn parse_body(raw: &[u8]) -> Self {
        let mut query = Self::default();
        for pair in raw.split(|&b| b == b'&').take(MAX_INPUT_VARS + 1) {
            if !pair.is_empty() {
                query.register_pair(pair, Decode::Plus, Duplicate::Replace);
            }
        }
        query.finish()
    }

    /// The fields of a `multipart/form-data` body, already limited to
    /// `max_input_vars` by the parser (`multipart`). Names and values are
    /// used as sent, without URL-decoding.
    pub fn from_fields(fields: &[(Vec<u8>, Vec<u8>)]) -> Self {
        let mut query = Self::default();
        for (name, value) in fields {
            query.register(name, value, Duplicate::Replace);
        }
        query.finish()
    }

    /// Every parameter, as `$request->query->all()` returns them.
    pub fn all(&self) -> &[(String, QueryValue)] {
        &self.entries
    }

    /// The parameters as PHP holds them: binary-safe keys and values, int
    /// keys where PHP has them.
    pub fn vars(&self) -> &PhpArray {
        &self.vars.0
    }

    pub fn has(&self, name: &str) -> bool {
        self.value(name).is_some()
    }

    /// `$request->request->all()[$name]`: the value, string or array.
    pub fn value(&self, name: &str) -> Option<&QueryValue> {
        lookup(&self.entries, name)
    }

    /// `$request->query->get($name)`: the string value, `None` if absent,
    /// and an error if it is an array.
    pub fn get(&self, name: &str) -> Result<Option<&str>, NonScalarValue> {
        match self.value(name) {
            None => Ok(None),
            Some(QueryValue::Str(text)) => Ok(Some(text)),
            Some(QueryValue::Map(_)) => Err(NonScalarValue(name.to_owned())),
        }
    }

    /// Whether the value now stored under the top-level `name`, including
    /// any nested values, was sent as UTF-8 (true when absent).
    pub fn is_utf8(&self, name: &str) -> bool {
        !self.not_utf8.iter().any(|n| n == name)
    }

    /// Fills `entries` from `vars`; every constructor ends with it.
    pub(crate) fn finish(mut self) -> Self {
        self.entries = to_entries(&self.vars.0);
        self
    }

    pub(crate) fn into_entries(self) -> Vec<(String, QueryValue)> {
        to_entries(&self.vars.0)
    }

    /// One `name=value` pair: the name ends at the first `=`; a pair
    /// without `=` has an empty value.
    pub(crate) fn register_pair(&mut self, pair: &[u8], decode: Decode, duplicate: Duplicate) {
        let (name, value) = match pair.iter().position(|&b| b == b'=') {
            Some(at) => (&pair[..at], &pair[at + 1..]),
            None => (pair, &b""[..]),
        };
        let (name, value) = match decode {
            Decode::Plus => (url::urldecode(name), url::urldecode(value)),
            Decode::ValueOnly => (name.to_vec(), url::rawurldecode(value)),
        };
        self.register(&name, &value, duplicate);
    }

    /// Stores `value` under `name`, which may be `base[a][b]...`, as PHP
    /// does (`php_value::vars::register`).
    pub(crate) fn register(&mut self, name: &[u8], value: &[u8], duplicate: Duplicate) {
        let done = vars::register(&mut self.vars.0, name, PhpString::from(value), duplicate);
        if !done.stored && !done.replaced {
            return;
        }
        let Some(base) = vars::top_level_key(name).map(|key| key_text(&key)) else {
            return;
        };
        if done.replaced {
            self.not_utf8.retain(|n| *n != base);
        }
        if done.stored && std::str::from_utf8(value).is_err() && !self.not_utf8.contains(&base) {
            self.not_utf8.push(base);
        }
    }
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => s.to_string_lossy().into_owned(),
    }
}

/// An array's entries as text (bytes that are not UTF-8 replaced).
fn to_entries(array: &PhpArray) -> Vec<(String, QueryValue)> {
    array
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Value::Array(children) => QueryValue::Map(to_entries(children)),
                other => QueryValue::Str(
                    other
                        .to_php_string()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ),
            };
            (key_text(key), value)
        })
        .collect()
}

/// How a pair is decoded before it is registered.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decode {
    /// `urldecode()` on name and value (query strings and form bodies).
    Plus,
    /// The name as sent, the value `rawurldecode()`d (cookies: PHP stopped
    /// decoding cookie names in 7.2.34 / 7.3.23 / 7.4.11, per the manual).
    ValueOnly,
}

/// PHP's `urldecode()`: `+` is a space, `%XX` a byte; invalid escapes stay.
/// Bytes that do not form UTF-8 are replaced.
pub fn urldecode(text: &str) -> String {
    String::from_utf8_lossy(&url::urldecode(text.as_bytes())).into_owned()
}

/// PHP's `rawurldecode()`: like `urldecode()`, but `+` stays.
pub fn rawurldecode(text: &str) -> String {
    String::from_utf8_lossy(&url::rawurldecode(text.as_bytes())).into_owned()
}

/// The value stored under `name` in a list of parameters or array entries.
pub fn lookup<'a>(entries: &'a [(String, QueryValue)], name: &str) -> Option<&'a QueryValue> {
    entries
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> QueryValue {
        QueryValue::Str(text.to_owned())
    }

    #[test]
    fn plain_pairs_last_one_wins() {
        let query = Query::parse("tag=lorem&q=a+b%20c&tag=ipsum&empty&=x");
        assert_eq!(query.get("tag"), Ok(Some("ipsum")));
        assert_eq!(query.get("q"), Ok(Some("a b c")));
        assert_eq!(query.get("empty"), Ok(Some("")));
        assert!(query.has("empty"));
        assert_eq!(query.get("missing"), Ok(None));
    }

    #[test]
    fn dots_and_spaces_in_names_become_underscores() {
        let query = Query::parse("a.b=1&c+d=2&%20lead=3");
        assert_eq!(
            query.all(),
            [
                ("a_b".into(), s("1")),
                ("c_d".into(), s("2")),
                ("lead".into(), s("3"))
            ]
        );
    }

    #[test]
    fn brackets_build_arrays() {
        let query = Query::parse("foo[]=1&foo[]=2&bar[x][y]=3&baz[=4");
        assert_eq!(
            query.all(),
            [
                (
                    "foo".into(),
                    QueryValue::Map(vec![("0".into(), s("1")), ("1".into(), s("2"))])
                ),
                (
                    "bar".into(),
                    QueryValue::Map(vec![(
                        "x".into(),
                        QueryValue::Map(vec![("y".into(), s("3"))])
                    )])
                ),
                ("baz_".into(), s("4")),
            ]
        );
        assert_eq!(query.get("foo"), Err(NonScalarValue("foo".into())));
    }

    #[test]
    fn invalid_escapes_are_kept() {
        assert_eq!(urldecode("100%"), "100%");
        assert_eq!(urldecode("%zz%41"), "%zzA");
    }

    #[test]
    fn array_indexes_follow_php_integer_key_rules() {
        for (input, expected) in [
            ("a[-3]=x&a[]=y", "-2"),
            ("a[01]=x&a[]=y", "0"),
            ("a[%2B2]=x&a[]=y", "0"),
            ("a[-0]=x&a[]=y", "0"),
            ("a[9223372036854775808]=x&a[]=y", "0"),
        ] {
            let query = Query::parse(input);
            let Some(QueryValue::Map(entries)) = query.value("a") else {
                panic!("expected an array for {input}");
            };
            assert_eq!(lookup(entries, expected), Some(&s("y")), "{input}");
        }
        let query = Query::parse("a[b][9223372036854775807]=x&a[b][]=y");
        assert_eq!(
            query.value("a"),
            Some(&QueryValue::Map(vec![(
                "b".into(),
                QueryValue::Map(vec![(i64::MAX.to_string(), s("x"))])
            )]))
        );
    }

    #[test]
    fn tracks_values_that_were_not_utf8() {
        // The contract of the earlier implementation, kept for its callers.
        for (raw, utf8) in [
            ("q=%FF", false),
            ("q=%FF&q=a", true),
            ("q=a&q=%FF", false),
            ("q[]=%FF", false),
            ("q[x]=a&q=%FF", false),
            ("q=%FF&q[x]=a", true),
            ("%FF=1", true),
        ] {
            assert_eq!(Query::parse(raw).is_utf8("q"), utf8, "{raw}");
        }
    }

    #[test]
    fn form_bodies_keep_bytes_and_read_one_pair_more() {
        let body = Query::parse_body(b"a=\xff&b=%FF");
        assert!(!body.is_utf8("a") && !body.is_utf8("b"));
        let pairs: Vec<String> = (0..=MAX_INPUT_VARS).map(|i| format!("v{i}=x")).collect();
        assert_eq!(
            Query::parse_body(pairs.join("&").as_bytes()).all().len(),
            1001
        );
        assert_eq!(Query::parse(&pairs.join("&")).all().len(), 1000);
    }
}

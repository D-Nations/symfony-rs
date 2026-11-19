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
//!
//! Upstream: symfony/http-foundation InputBag.php

/// A parsed query value: a string, or an array (PHP arrays keep insertion
/// order; keys are strings here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryValue {
    Str(String),
    Map(Vec<(String, QueryValue)>),
}

/// PHP's `max_input_vars` default.
pub(crate) const MAX_INPUT_VARS: usize = 1000;
/// PHP's `max_input_nesting_level` default: a name with more `[...]`
/// groups is dropped.
const MAX_NESTING: usize = 64;

/// `InputBag::get()` found an array where a string was expected; Symfony
/// answers 400 Bad Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonScalarValue(pub String);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    entries: Vec<(String, QueryValue)>,
    /// Top-level names whose value, as sent, was not UTF-8. PHP keeps the
    /// bytes; here they are replaced, so the places where the original
    /// fails on such bytes (`u()` in `findBySearchQuery()`) can ask.
    not_utf8: Vec<String>,
}

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
        query
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
        query
    }

    /// The fields of a `multipart/form-data` body, already limited to
    /// `max_input_vars` by the parser (`multipart`). Names and values are
    /// used as sent, without URL-decoding.
    pub fn from_fields(fields: &[(Vec<u8>, Vec<u8>)]) -> Self {
        let mut query = Self::default();
        for (name, value) in fields {
            query.register(name, value, Duplicate::Replace);
        }
        query
    }

    /// Every parameter, as `$request->query->all()` returns them.
    pub fn all(&self) -> &[(String, QueryValue)] {
        &self.entries
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

    pub(crate) fn into_entries(self) -> Vec<(String, QueryValue)> {
        self.entries
    }

    /// One `name=value` pair: the name ends at the first `=`; a pair
    /// without `=` has an empty value.
    pub(crate) fn register_pair(&mut self, pair: &[u8], decode: Decode, duplicate: Duplicate) {
        let (name, value) = match pair.iter().position(|&b| b == b'=') {
            Some(at) => (&pair[..at], &pair[at + 1..]),
            None => (pair, &b""[..]),
        };
        let (name, value) = match decode {
            Decode::Plus => (decode_bytes(name, true), decode_bytes(value, true)),
            Decode::ValueOnly => (name.to_vec(), decode_bytes(value, false)),
        };
        self.register(&name, &value, duplicate);
    }

    /// Stores `value` under `name`, which may be `base[a][b]...`.
    pub(crate) fn register(&mut self, name: &[u8], value: &[u8], duplicate: Duplicate) {
        let Some(name) = Name::parse(name) else {
            return;
        };
        let text = String::from_utf8_lossy(value).into_owned();
        let (replaced, stored) = match name.groups.as_slice() {
            [] => {
                let position = self.entries.iter().position(|(key, _)| *key == name.base);
                match position {
                    Some(_) if duplicate == Duplicate::KeepFirst => return,
                    Some(at) => self.entries[at].1 = QueryValue::Str(text),
                    None => self
                        .entries
                        .push((name.base.clone(), QueryValue::Str(text))),
                }
                (true, true)
            }
            // A new or replaced array stays even when the value itself
            // cannot be appended.
            groups => match map_entry(&mut self.entries, &name.base) {
                Some((map, replaced)) => (replaced, store(map, groups, text)),
                None => (false, false),
            },
        };
        if replaced {
            self.not_utf8.retain(|n| *n != name.base);
        }
        if stored && std::str::from_utf8(value).is_err() && !self.not_utf8.contains(&name.base) {
            self.not_utf8.push(name.base);
        }
    }
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

/// What happens when a name without brackets is already present.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Duplicate {
    /// The later value replaces the earlier one in place.
    Replace,
    /// The first value stays (cookies).
    KeepFirst,
}

/// One `[...]` group of a name.
#[derive(Debug, PartialEq, Eq)]
enum Group {
    /// `[]`: the next integer index.
    Append,
    Key(Vec<u8>),
}

/// A registered name: the top-level key and its groups.
#[derive(Debug, PartialEq, Eq)]
struct Name {
    base: String,
    groups: Vec<Group>,
}

impl Name {
    /// `None` when PHP stores nothing for this name.
    ///
    /// Recorded rules: the name ends at a NUL byte; leading spaces are
    /// dropped; an empty name, or one starting with `[`, stores nothing.
    /// Before the first `[`, spaces and dots become `_`. When that `[` has
    /// no `]` after it, it is not an array: every space, dot and `[` of the
    /// name becomes `_`. Otherwise each `[...]` group ends at the first `]`;
    /// anything after a group other than `[` is ignored, as is an unclosed
    /// later group. More than `max_input_nesting_level` groups store
    /// nothing.
    fn parse(name: &[u8]) -> Option<Self> {
        let name = match name.iter().position(|&b| b == 0) {
            Some(nul) => &name[..nul],
            None => name,
        };
        let start = name.iter().position(|&b| b != b' ')?;
        let name = &name[start..];
        let Some(open) = name.iter().position(|&b| b == b'[') else {
            return Some(Self::plain(name, b" ."));
        };
        if open == 0 {
            return None;
        }
        if !name[open + 1..].contains(&b']') {
            return Some(Self::plain(name, b" .["));
        }
        let mut groups = Vec::new();
        let mut rest = &name[open..];
        while let Some(inside) = rest.strip_prefix(b"[") {
            let Some(close) = inside.iter().position(|&b| b == b']') else {
                break;
            };
            groups.push(Group::of(&inside[..close]));
            rest = &inside[close + 1..];
        }
        if groups.len() > MAX_NESTING {
            return None;
        }
        Some(Self {
            base: underscored(&name[..open], b" ."),
            groups,
        })
    }

    fn plain(name: &[u8], converted: &[u8]) -> Self {
        Self {
            base: underscored(name, converted),
            groups: Vec::new(),
        }
    }
}

impl Group {
    /// `[]`, and a group holding exactly one whitespace byte (`[ ]`,
    /// `[\t]`, ...), append; anything else is the key as written, spaces
    /// included (`[ b]`, `[  ]`).
    fn of(inside: &[u8]) -> Self {
        match inside {
            [] => Self::Append,
            [b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c] => Self::Append,
            key => Self::Key(key.to_vec()),
        }
    }
}

fn underscored(name: &[u8], converted: &[u8]) -> String {
    let bytes: Vec<u8> = name
        .iter()
        .map(|&b| if converted.contains(&b) { b'_' } else { b })
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The array stored under `key`, created at the end if absent and
/// replacing a string in place. The flag is true unless an array was
/// already there.
fn map_entry<'a>(
    entries: &'a mut Vec<(String, QueryValue)>,
    key: &str,
) -> Option<(&'a mut Vec<(String, QueryValue)>, bool)> {
    let at = match entries.iter().position(|(k, _)| k == key) {
        Some(at) => at,
        None => {
            entries.push((key.to_owned(), QueryValue::Map(Vec::new())));
            entries.len() - 1
        }
    };
    let slot = &mut entries.get_mut(at)?.1;
    let replaced = !matches!(slot, QueryValue::Map(_));
    if replaced {
        *slot = QueryValue::Map(Vec::new());
    }
    match slot {
        QueryValue::Map(map) => Some((map, replaced)),
        QueryValue::Str(_) => None,
    }
}

/// Stores `value` at `groups` below `map`. False when an append found the
/// next index taken (after `PHP_INT_MAX`), in which case nothing more is
/// stored.
fn store(mut map: &mut Vec<(String, QueryValue)>, groups: &[Group], value: String) -> bool {
    let Some((last, parents)) = groups.split_last() else {
        return false;
    };
    for group in parents {
        let Some(key) = key_for(map, group) else {
            return false;
        };
        let Some((child, _)) = map_entry(map, &key) else {
            return false;
        };
        map = child;
    }
    let Some(key) = key_for(map, last) else {
        return false;
    };
    match map.iter().position(|(k, _)| *k == key) {
        Some(at) => map[at].1 = QueryValue::Str(value),
        None => map.push((key, QueryValue::Str(value))),
    }
    true
}

/// The array key a group stands for. Keys that are decimal integers in
/// PHP's sense become those integers' text; `[]` is the largest integer
/// key plus one (0 when there is none), and `None` past `PHP_INT_MAX`.
fn key_for(map: &[(String, QueryValue)], group: &Group) -> Option<String> {
    match group {
        Group::Key(key) => Some(match int_key(key) {
            Some(n) => n.to_string(),
            None => String::from_utf8_lossy(key).into_owned(),
        }),
        Group::Append => match map.iter().filter_map(|(k, _)| int_key(k.as_bytes())).max() {
            None => Some("0".to_owned()),
            Some(i64::MAX) => None,
            Some(n) => Some((n + 1).to_string()),
        },
    }
}

/// The PHP manual's rule for array keys: a string holding a decimal
/// integer, with no `+`, no leading zero and not `-0`, that fits in an
/// int, is that int.
fn int_key(key: &[u8]) -> Option<i64> {
    let digits = key.strip_prefix(b"-").unwrap_or(key);
    let canonical = match digits {
        [b'0'] => key.len() == 1,
        [b'1'..=b'9', rest @ ..] => rest.iter().all(u8::is_ascii_digit),
        _ => false,
    };
    if !canonical {
        return None;
    }
    std::str::from_utf8(key).ok()?.parse().ok()
}

/// PHP's `urldecode()`: `+` is a space, `%XX` a byte; invalid escapes stay.
/// Bytes that do not form UTF-8 are replaced.
pub fn urldecode(text: &str) -> String {
    String::from_utf8_lossy(&decode_bytes(text.as_bytes(), true)).into_owned()
}

/// PHP's `rawurldecode()`: like `urldecode()`, but `+` stays.
pub fn rawurldecode(text: &str) -> String {
    String::from_utf8_lossy(&decode_bytes(text.as_bytes(), false)).into_owned()
}

/// `%` followed by two hexadecimal digits is that byte; any other `%`
/// stays as it is (recorded: `%zz%41` is `%zzA`, `%%41` is `%A`).
fn decode_bytes(text: &[u8], plus_is_space: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while let Some(&byte) = text.get(at) {
        let escaped = match (byte, text.get(at + 1..at + 3)) {
            (b'%', Some(&[high, low])) => hex(high).zip(hex(low)).map(|(h, l)| h << 4 | l),
            _ => None,
        };
        match escaped {
            Some(decoded) => {
                out.push(decoded);
                at += 3;
            }
            None => {
                out.push(if plus_is_space && byte == b'+' {
                    b' '
                } else {
                    byte
                });
                at += 1;
            }
        }
    }
    out
}

fn hex(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(16)
        .and_then(|d| u8::try_from(d).ok())
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

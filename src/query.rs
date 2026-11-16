//! The request's query string, read the way PHP fills `$_GET`.
//!
//! Ports PHP's query parsing (`php_default_treat_data` and
//! `php_register_variable_ex`) and Symfony's `InputBag::get()`, because the
//! original's behaviour depends on them: `?a.b=1` arrives as `a_b`,
//! `?tag[]=x` makes `tag` an array (and `->get('tag')` a 400 error), and the
//! RSS link copies the whole parsed query.

/// A parsed query value: a string, or an array (PHP arrays keep insertion
/// order; keys are strings here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryValue {
    Str(String),
    Map(Vec<(String, QueryValue)>),
}

/// PHP's `max_input_vars` default: later pairs are ignored.
const MAX_INPUT_VARS: usize = 1000;
/// PHP's `max_input_nesting_level` default.
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
    pub fn parse(raw: &str) -> Self {
        let mut query = Self::default();
        for pair in raw
            .split('&')
            .filter(|pair| !pair.is_empty())
            .take(MAX_INPUT_VARS)
        {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            query.register_bytes(&percent_decode(name, true), &percent_decode(value, true));
        }
        query
    }

    /// The fields of a `multipart/form-data` body, already limited to
    /// `max_input_vars` by the parser (`request::multipart`).
    pub fn from_fields(fields: &[(Vec<u8>, Vec<u8>)]) -> Self {
        let mut query = Self::default();
        for (name, value) in fields {
            query.register_bytes(name, value);
        }
        query
    }

    /// Every parameter, as `$request->query->all()` returns them.
    pub fn all(&self) -> &[(String, QueryValue)] {
        &self.entries
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(key, _)| key == name)
    }

    /// `$request->request->all()[$name]`: the value, string or array.
    pub fn value(&self, name: &str) -> Option<&QueryValue> {
        lookup(&self.entries, name)
    }

    /// `$request->query->get($name)`: the string value, `None` if absent,
    /// and an error if it is an array.
    pub fn get(&self, name: &str) -> Result<Option<&str>, NonScalarValue> {
        match self.entries.iter().find(|(key, _)| key == name) {
            None => Ok(None),
            Some((_, QueryValue::Str(value))) => Ok(Some(value)),
            Some((_, QueryValue::Map(_))) => Err(NonScalarValue(name.to_owned())),
        }
    }

    /// Whether the value last sent under the top-level `name` was UTF-8
    /// (true when absent).
    pub fn is_utf8(&self, name: &str) -> bool {
        !self.not_utf8.iter().any(|key| key == name)
    }

    /// Decodes name and value (invalid UTF-8 replaced) and registers them,
    /// remembering a value that was not UTF-8.
    fn register_bytes(&mut self, name: &[u8], value: &[u8]) {
        let name = String::from_utf8_lossy(name);
        let utf8 = std::str::from_utf8(value).is_ok();
        let value = String::from_utf8_lossy(value).into_owned();
        if let Some(base) = self.register(&name, value) {
            self.not_utf8.retain(|key| *key != base);
            if !utf8 {
                self.not_utf8.push(base);
            }
        }
    }

    /// `php_register_variable_ex`: splits `name[a][b]` into its path and
    /// stores the value there. Returns the top-level name, if stored.
    fn register(&mut self, raw_name: &str, value: String) -> Option<String> {
        let name = raw_name.trim_start_matches(' ');
        let (base, indexes) = split_name(name);
        if base.is_empty() {
            return None;
        }
        insert(&mut self.entries, &base, &indexes, value);
        Some(base)
    }
}

/// PHP's `urldecode()`: `+` is a space, `%XX` a byte; invalid escapes stay.
/// Bytes that do not form UTF-8 are replaced.
pub fn urldecode(text: &str) -> String {
    String::from_utf8_lossy(&percent_decode(text, true)).into_owned()
}

/// PHP's `rawurldecode()`, which Symfony's router applies to the path:
/// like [`urldecode`] but `+` stays a plus sign.
pub fn rawurldecode(text: &str) -> String {
    String::from_utf8_lossy(&percent_decode(text, false)).into_owned()
}

/// The bytes `%XX` escapes stand for, as PHP decodes them.
fn percent_decode(text: &str, plus_is_space: bool) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' if plus_is_space => out.push(b' '),
            b'%' => match bytes.get(i + 1..i + 3) {
                Some(&[high, low]) if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() => {
                    out.push(hex_value(high) << 4 | hex_value(low));
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            byte => out.push(byte),
        }
        i += 1;
    }
    out
}

fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        _ => digit.to_ascii_uppercase().saturating_sub(b'A') + 10,
    }
}

/// Splits `a.b[x][]` into `a_b` and `[Some("x"), None]` (`None` = append).
/// In the base name, `.` and ` ` become `_`; a `[` without a matching `]`
/// also becomes `_` and the rest is kept literally.
fn split_name(name: &str) -> (String, Vec<Option<String>>) {
    let normalize = |text: &str| text.replace(['.', ' '], "_");
    let Some(open) = name.find('[') else {
        return (normalize(name), Vec::new());
    };
    let mut indexes = Vec::new();
    let mut rest = &name[open..];
    while let Some(after_open) = rest.strip_prefix('[') {
        if indexes.len() >= MAX_NESTING {
            break;
        }
        let Some(close) = after_open.find(']') else {
            if indexes.is_empty() {
                // `a[b` : the bracket is not an array index.
                let mut base = normalize(&name[..open]);
                base.push('_');
                base.push_str(&name[open + 1..]);
                return (base, Vec::new());
            }
            break;
        };
        let index = &after_open[..close];
        indexes.push((!index.is_empty()).then(|| index.to_owned()));
        rest = &after_open[close + 1..];
    }
    (normalize(&name[..open]), indexes)
}

/// The value stored under `name` in a list of parameters or array entries.
pub fn lookup<'a>(entries: &'a [(String, QueryValue)], name: &str) -> Option<&'a QueryValue> {
    entries
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

fn next_index(entries: &[(String, QueryValue)]) -> Option<String> {
    let max = entries
        .iter()
        // PHP keeps noncanonical integers (e.g. "01" and "+2") as
        // string keys, which do not affect the next numeric index.
        .filter_map(|(key, _)| key.parse::<i64>().ok().filter(|n| n.to_string() == *key))
        .max();
    // PHP ignores an append when the largest integer key is occupied.
    let next = match max {
        Some(max) => max.checked_add(1)?,
        None => 0,
    };
    Some(next.to_string())
}

fn insert(
    entries: &mut Vec<(String, QueryValue)>,
    key: &str,
    indexes: &[Option<String>],
    value: String,
) {
    let Some((first, rest)) = indexes.split_first() else {
        match entries.iter_mut().find(|(existing, _)| existing == key) {
            Some((_, slot)) => *slot = QueryValue::Str(value),
            None => entries.push((key.to_owned(), QueryValue::Str(value))),
        }
        return;
    };
    let position = match entries.iter().position(|(existing, _)| existing == key) {
        Some(position) => {
            if !matches!(entries[position].1, QueryValue::Map(_)) {
                entries[position].1 = QueryValue::Map(Vec::new());
            }
            position
        }
        None => {
            entries.push((key.to_owned(), QueryValue::Map(Vec::new())));
            entries.len() - 1
        }
    };
    if let QueryValue::Map(children) = &mut entries[position].1 {
        let Some(child_key) = first.clone().or_else(|| next_index(children)) else {
            return;
        };
        insert(children, &child_key, rest, value);
    }
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
}

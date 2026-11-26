//! `session.serialize_handler`: how the session extension turns
//! `$_SESSION` into the text a save handler stores, and back, written from
//! the PHP manual and PHP 8.4.27's answers in the oracle (no PHP source
//! code, PLAN.md decision 8).
//!
//! - `php` (the default): each entry as `key|` and the value's
//!   `serialize()` text. An integer key is skipped with a warning; a key
//!   with `|` makes the whole encoding fail, silently. Decoding reads the
//!   entries one after the other and fails as a whole on any bad one.
//! - `php_serialize`: `serialize($_SESSION)`, and `unserialize()` back,
//!   which may give a value that is not an array.
//!
//! PHP numbers the values of all `php` entries together, so a
//! back-reference (`r:`/`R:`) in one entry may point into another. This
//! port reads and writes the entries one at a time, so it refuses session
//! data with back-references (`Class::NotPorted`) rather than read or
//! write them differently. `php_binary` is refused the same way.
//!
//! Upstream: none

use php_value::{
    Key, PhpString, Value,
    constants::Registry,
    serialize::{Classes, serialize, unserialize},
};

use crate::exception::{Class, Error, Result};

/// A `session.serialize_handler` PHP has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serializer {
    Php,
    PhpSerialize,
    PhpBinary,
}

impl Serializer {
    /// The handler `name` names; `None` for one PHP does not have.
    pub fn from_name(name: &[u8]) -> Option<Self> {
        match name {
            b"php" => Some(Self::Php),
            b"php_serialize" => Some(Self::PhpSerialize),
            b"php_binary" => Some(Self::PhpBinary),
            _ => None,
        }
    }
}

fn not_ported(what: &str) -> Error {
    Error::new(
        Class::NotPorted,
        format!("session data {what} is not ported"),
    )
}

/// The classes `unserialize()` may create here: `stdClass`; any other
/// object is a `__PHP_Incomplete_Class`.
fn classes() -> Classes {
    let mut classes = Classes::new();
    classes.allow("stdClass");
    classes
}

/// PHP's `unserialize()` nests at most this deep (`unserialize_max_depth`).
const MAX_DEPTH: usize = 4096;

/// Finds where one serialized value ends, and whether it has a
/// back-reference.
struct Scan<'a> {
    data: &'a [u8],
    pos: usize,
    references: bool,
}

impl Scan<'_> {
    fn byte(&mut self, b: u8) -> Option<()> {
        (self.data.get(self.pos) == Some(&b)).then(|| self.pos += 1)
    }

    fn until(&mut self, b: u8) -> Option<&[u8]> {
        let rest = self.data.get(self.pos..)?;
        let i = rest.iter().position(|x| *x == b)?;
        self.pos += i + 1;
        Some(&rest[..i])
    }

    fn length(&mut self, end: u8) -> Option<usize> {
        let digits = self.until(end)?;
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(digits).ok()?.parse().ok()
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        (end <= self.data.len()).then(|| self.pos = end)
    }

    /// `"<n bytes>"`.
    fn quoted(&mut self, n: usize) -> Option<()> {
        self.byte(b'"')?;
        self.skip(n)?;
        self.byte(b'"')
    }

    fn values(&mut self, n: usize, depth: usize) -> Option<()> {
        for _ in 0..n.checked_mul(2)? {
            self.value(depth + 1)?;
        }
        self.byte(b'}')
    }

    fn value(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let tag = *self.data.get(self.pos)?;
        self.pos += 1;
        match tag {
            b'N' => self.byte(b';'),
            b'b' | b'i' | b'd' => {
                self.byte(b':')?;
                self.until(b';').map(|_| ())
            }
            b'r' | b'R' => {
                self.references = true;
                self.byte(b':')?;
                self.until(b';').map(|_| ())
            }
            b's' | b'E' => {
                self.byte(b':')?;
                let n = self.length(b':')?;
                self.quoted(n)?;
                self.byte(b';')
            }
            b'S' => {
                self.byte(b':')?;
                let n = self.length(b':')?;
                self.byte(b'"')?;
                for _ in 0..n {
                    let b = *self.data.get(self.pos)?;
                    self.skip(if b == b'\\' { 3 } else { 1 })?;
                }
                self.byte(b'"')?;
                self.byte(b';')
            }
            b'a' => {
                self.byte(b':')?;
                let n = self.length(b':')?;
                self.byte(b'{')?;
                self.values(n, depth)
            }
            b'O' => {
                self.byte(b':')?;
                let n = self.length(b':')?;
                self.quoted(n)?;
                self.byte(b':')?;
                let m = self.length(b':')?;
                self.byte(b'{')?;
                self.values(m, depth)
            }
            b'C' => {
                self.byte(b':')?;
                let n = self.length(b':')?;
                self.quoted(n)?;
                self.byte(b':')?;
                let m = self.length(b':')?;
                self.byte(b'{')?;
                self.skip(m)?;
                self.byte(b'}')
            }
            _ => None,
        }
    }
}

/// Whether `serialize()` text has a back-reference.
fn has_references(text: &[u8]) -> bool {
    let mut scan = Scan {
        data: text,
        pos: 0,
        references: false,
    };
    scan.value(0).is_none() || scan.references
}

/// What encoding gave: the text, or PHP's `false`; and the warnings, each
/// without its function prefix.
#[derive(Debug, Default)]
pub struct Encoded {
    pub text: Option<PhpString>,
    pub warnings: Vec<String>,
}

/// Encodes the entries of an array `$_SESSION`.
pub fn encode<'a>(
    serializer: Serializer,
    entries: impl Iterator<Item = (&'a Key, Value)>,
) -> Result<Encoded> {
    let mut out = Encoded::default();
    match serializer {
        Serializer::PhpBinary => return Err(not_ported("in php_binary")),
        Serializer::PhpSerialize => {
            let mut array = php_value::PhpArray::new();
            for (key, value) in entries {
                array.insert(key.clone(), value);
            }
            out.text = Some(serialize(&Value::Array(array)));
        }
        Serializer::Php => {
            let mut text = Vec::new();
            for (key, value) in entries {
                let name = match key {
                    Key::Int(n) => {
                        out.warnings.push(format!("Skipping numeric key {n}"));
                        continue;
                    }
                    Key::Str(name) => name,
                };
                if name.as_bytes().contains(&b'|') {
                    return Ok(out);
                }
                let value = serialize(&value);
                if has_references(value.as_bytes()) {
                    return Err(not_ported("with back-references between entries"));
                }
                text.extend_from_slice(name.as_bytes());
                text.push(b'|');
                text.extend_from_slice(value.as_bytes());
            }
            out.text = Some(PhpString::from(text));
        }
    }
    Ok(out)
}

/// What decoding gave: for `php`, entries to set in `$_SESSION`; for
/// `php_serialize`, the value `$_SESSION` becomes. `None` where decoding
/// fails.
#[derive(Debug)]
pub enum Decoded {
    Entries(Vec<(Key, Value)>),
    Replace(Value),
}

/// Decodes what a save handler read.
pub fn decode(serializer: Serializer, data: &[u8]) -> Result<Option<Decoded>> {
    match serializer {
        Serializer::PhpBinary => Err(not_ported("in php_binary")),
        Serializer::PhpSerialize => {
            if data.is_empty() {
                return Ok(Some(Decoded::Replace(Value::Array(
                    php_value::PhpArray::new(),
                ))));
            }
            // one value, numbered as PHP numbers it
            Ok(unserialize(data, &classes(), &Registry::default()).map(Decoded::Replace))
        }
        Serializer::Php => {
            let mut entries = Vec::new();
            let mut pos = 0;
            while pos < data.len() {
                let Some(bar) = data[pos..].iter().position(|b| *b == b'|') else {
                    return Ok(None);
                };
                let key = Key::from_bytes(&data[pos..pos + bar]);
                pos += bar + 1;
                let mut scan = Scan {
                    data,
                    pos,
                    references: false,
                };
                if scan.value(0).is_none() {
                    return Ok(None);
                }
                if scan.references {
                    return Err(not_ported("with back-references between entries"));
                }
                let Some(value) =
                    unserialize(&data[pos..scan.pos], &classes(), &Registry::default())
                else {
                    return Ok(None);
                };
                entries.push((key, value));
                pos = scan.pos;
            }
            Ok(Some(Decoded::Entries(entries)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decoded(data: &str) -> Option<Vec<(Key, Value)>> {
        match decode(Serializer::Php, data.as_bytes()).ok()? {
            Some(Decoded::Entries(e)) => Some(e),
            _ => None,
        }
    }

    #[test]
    fn php_entries_are_read_one_after_the_other() {
        let e = decoded("a|i:1;b|s:3:\"x;y\";c|a:1:{i:0;N;}").unwrap_or_default();
        assert_eq!(3, e.len());
        assert_eq!(Key::from_bytes(b"b"), e[1].0);
        assert_eq!(Value::string("x;y"), e[1].1);
    }

    #[test]
    fn a_bad_entry_fails_the_whole_decoding() {
        assert!(decoded("a|i:1;b|garbage").is_none());
        assert!(decoded("garbage").is_none());
        assert_eq!(Some(0), decoded("").map(|e| e.len()));
    }

    #[test]
    fn back_references_are_refused() {
        assert!(decode(Serializer::Php, b"a|a:2:{i:0;i:1;i:1;R:2;}").is_err());
    }
}

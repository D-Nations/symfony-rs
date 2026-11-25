//! `.env.local.php` without running PHP: `bootEnv()` includes the file,
//! which `composer dump-env` writes as `<?php return <array literal>;`.
//! This reads that form only: the array (`array(...)` or `[...]`) of
//! string or integer keys to strings, integers, floats, booleans, null or
//! nested arrays, written with single- or double-quoted strings (no
//! interpolation), with comments and an optional closing `?>`. A file it
//! cannot read that way fails with a `LogicException` naming it, rather
//! than being run.
//!
//! Upstream: none

use php_value::{Key, PhpArray, PhpString, Value, numeric::string_to_float};

use crate::exception::Error;

/// The array the file returns, `None` when it returns something else
/// (PHP's `include` of a file without `return` gives `1`).
pub fn read(path: &str) -> Result<Option<PhpArray>, Error> {
    let data = std::fs::read(path).map_err(|_| unreadable(path, "it cannot be read"))?;
    let mut reader = Reader { s: &data, i: 0 };
    reader.file().map_err(|reason| unreadable(path, reason))
}

fn unreadable(path: &str, reason: &str) -> Error {
    Error::Logic(format!(
        "The \"{path}\" file is not a plain array as written by \"composer dump-env\" ({reason}); this port does not run PHP code."
    ))
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}

type Read<T> = Result<T, &'static str>;

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn rest(&self) -> &[u8] {
        &self.s[self.i..]
    }

    /// Whitespace and comments.
    fn skip(&mut self) {
        loop {
            while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
                self.i += 1;
            }
            if self.rest().starts_with(b"//") || self.rest().starts_with(b"#") {
                while self.peek().is_some_and(|b| b != b'\n') {
                    self.i += 1;
                }
            } else if self.rest().starts_with(b"/*") {
                match self.rest()[2..].windows(2).position(|w| w == b"*/") {
                    Some(end) => self.i += 2 + end + 2,
                    None => self.i = self.s.len(),
                }
            } else {
                return;
            }
        }
    }

    fn eat(&mut self, token: &[u8]) -> bool {
        self.skip();
        if self.rest().len() >= token.len()
            && self.rest()[..token.len()].eq_ignore_ascii_case(token)
        {
            self.i += token.len();
            true
        } else {
            false
        }
    }

    fn file(&mut self) -> Read<Option<PhpArray>> {
        if !self.s.starts_with(b"<?php") {
            return Err("no <?php tag at the start");
        }
        self.i = 5;
        if !self.eat(b"return") {
            return Err("no return statement");
        }
        let value = self.value()?;
        self.eat(b";");
        self.skip();
        if self.eat(b"?>") {
            self.skip();
        }
        if self.i < self.s.len() {
            return Err("code after the returned array");
        }
        Ok(match value {
            Value::Array(array) => Some(array),
            _ => None,
        })
    }

    fn value(&mut self) -> Read<Value> {
        self.skip();
        if self.eat(b"array") {
            if !self.eat(b"(") {
                return Err("array without (");
            }
            return self.entries(b')').map(Value::Array);
        }
        if self.eat(b"[") {
            return self.entries(b']').map(Value::Array);
        }
        self.skip();
        match self.peek() {
            Some(b'\'') => self.single_quoted().map(Value::String),
            Some(b'"') => self.double_quoted().map(Value::String),
            Some(b'-' | b'+' | b'0'..=b'9' | b'.') => self.number(),
            _ => {
                for (word, value) in [
                    (&b"true"[..], Value::Bool(true)),
                    (b"false", Value::Bool(false)),
                    (b"null", Value::Null),
                ] {
                    let end = self.i + word.len();
                    if self.rest().len() >= word.len()
                        && self.rest()[..word.len()].eq_ignore_ascii_case(word)
                        && !self
                            .s
                            .get(end)
                            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    {
                        self.i = end;
                        return Ok(value);
                    }
                }
                Err("an expression that is not a literal")
            }
        }
    }

    fn entries(&mut self, close: u8) -> Read<PhpArray> {
        let mut array = PhpArray::new();
        loop {
            if self.eat(&[close]) {
                return Ok(array);
            }
            let first = self.value()?;
            if self.eat(b"=>") {
                let key = match &first {
                    Value::String(s) => Key::from_bytes(s),
                    Value::Int(i) => Key::Int(*i),
                    _ => return Err("a key that is not a string or an integer"),
                };
                let value = self.value()?;
                array.insert(key, value);
            } else {
                array.push(first).map_err(|_| "too many list items")?;
            }
            if !self.eat(b",") {
                if self.eat(&[close]) {
                    return Ok(array);
                }
                return Err("a malformed array");
            }
        }
    }

    fn number(&mut self) -> Read<Value> {
        let start = self.i;
        if matches!(self.peek(), Some(b'-' | b'+')) {
            self.i += 1;
        }
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'_'))
        {
            self.i += 1;
        }
        let text: Vec<u8> = self.s[start..self.i]
            .iter()
            .copied()
            .filter(|&b| b != b'_')
            .collect();
        let digits = text
            .strip_prefix(b"-")
            .or_else(|| text.strip_prefix(b"+"))
            .unwrap_or(&text);
        if digits.is_empty() {
            return Err("a malformed number");
        }
        if digits.iter().all(u8::is_ascii_digit) {
            let text = std::str::from_utf8(&text).map_err(|_| "a malformed number")?;
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Value::Int(i));
            }
        }
        Ok(Value::Float(string_to_float(&text)))
    }

    fn single_quoted(&mut self) -> Read<PhpString> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return Err("an unterminated string"),
                Some(b'\'') => {
                    self.i += 1;
                    return Ok(PhpString::from(out));
                }
                Some(b'\\') if matches!(self.s.get(self.i + 1), Some(b'\'' | b'\\')) => {
                    out.push(self.s[self.i + 1]);
                    self.i += 2;
                }
                Some(b) => {
                    out.push(b);
                    self.i += 1;
                }
            }
        }
    }

    fn double_quoted(&mut self) -> Read<PhpString> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err("an unterminated string");
            };
            self.i += 1;
            match b {
                b'"' => return Ok(PhpString::from(out)),
                b'$' if self.peek().is_some_and(|c| {
                    c.is_ascii_alphabetic() || c == b'_' || c == b'{' || c >= 0x80
                }) =>
                {
                    return Err("a string with interpolation");
                }
                b'{' if self.peek() == Some(b'$') => return Err("a string with interpolation"),
                b'\\' => self.escape(&mut out),
                b => out.push(b),
            }
        }
    }

    /// The escapes of double-quoted PHP strings.
    fn escape(&mut self, out: &mut Vec<u8>) {
        let Some(c) = self.peek() else {
            out.push(b'\\');
            return;
        };
        let simple = match c {
            b'n' => Some(b'\n'),
            b't' => Some(b'\t'),
            b'r' => Some(b'\r'),
            b'v' => Some(0x0b),
            b'e' => Some(0x1b),
            b'f' => Some(0x0c),
            b'\\' => Some(b'\\'),
            b'$' => Some(b'$'),
            b'"' => Some(b'"'),
            _ => None,
        };
        if let Some(byte) = simple {
            out.push(byte);
            self.i += 1;
            return;
        }
        if (b'0'..=b'7').contains(&c) {
            let digits = self
                .rest()
                .iter()
                .take(3)
                .take_while(|b| (b'0'..=b'7').contains(*b))
                .count();
            let text = std::str::from_utf8(&self.rest()[..digits]).unwrap_or("0");
            out.push(u32::from_str_radix(text, 8).unwrap_or(0) as u8);
            self.i += digits;
            return;
        }
        if c == b'x' {
            let digits = self.rest()[1..]
                .iter()
                .take(2)
                .take_while(|b| b.is_ascii_hexdigit())
                .count();
            if digits > 0 {
                let text = std::str::from_utf8(&self.rest()[1..=digits]).unwrap_or("0");
                out.push(u8::from_str_radix(text, 16).unwrap_or(0));
                self.i += 1 + digits;
                return;
            }
        }
        if c == b'u'
            && self.rest().get(1) == Some(&b'{')
            && let Some(close) = self.rest().iter().position(|&b| b == b'}')
            && let Some(ch) = std::str::from_utf8(&self.rest()[2..close])
                .ok()
                .and_then(|text| u32::from_str_radix(text, 16).ok())
                .and_then(char::from_u32)
        {
            let mut buf = [0; 4];
            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            self.i += close + 1;
            return;
        }
        out.push(b'\\');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Read<Option<PhpArray>> {
        Reader {
            s: source.as_bytes(),
            i: 0,
        }
        .file()
    }

    #[test]
    fn reads_dump_env_output() {
        let array = parse(
            "<?php\n\n// This file was generated by running \"composer dump-env prod\"\n\nreturn array (\n  'APP_ENV' => 'prod',\n  'APP_SECRET' => 'it\\'s',\n);\n",
        )
        .ok()
        .flatten()
        .unwrap_or_default();
        assert_eq!(array.get(&Key::from("APP_ENV")), Some(&Value::from("prod")));
        assert_eq!(
            array.get(&Key::from("APP_SECRET")),
            Some(&Value::from("it's"))
        );
    }

    #[test]
    fn reads_short_arrays_and_double_quotes() {
        let array = parse("<?php return [\"A\" => \"x\\ty\", 'N' => 3, 'B' => true];")
            .ok()
            .flatten()
            .unwrap_or_default();
        assert_eq!(array.get(&Key::from("A")), Some(&Value::from("x\ty")));
        assert_eq!(array.get(&Key::from("N")), Some(&Value::Int(3)));
        assert_eq!(array.get(&Key::from("B")), Some(&Value::Bool(true)));
    }

    #[test]
    fn refuses_code() {
        assert!(parse("<?php return getenv();").is_err());
        assert!(parse("<?php return [\"A\" => \"$x\"];").is_err());
        assert!(parse("<?php echo 1; return [];").is_err());
        assert_eq!(parse("<?php return 'x';"), Ok(None));
    }
}

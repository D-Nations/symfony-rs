//! `json_decode($json, true, $depth, JSON_BIGINT_AS_STRING)` with
//! `JSON_THROW_ON_ERROR` ([`decode_assoc`]), written from the PHP manual
//! and PHP 8.4.27's recorded answers (`tests/data/json_decode.json`,
//! `oracle/recorders/php_value_json_decode.php`):
//!
//! - RFC 8259 JSON, case-sensitive literals, whitespace only space, tab,
//!   LF and CR; objects become arrays with PHP's key rules (`"1"` is the
//!   key 1, `"01"` stays a string); a repeated key keeps its place and
//!   takes the last value;
//! - an integer that does not fit in 64 bits is kept as its text (with
//!   `JSON_BIGINT_AS_STRING`) or read as a float; a number with a
//!   fraction or exponent is a float (`1E400` is INF); `-0` is the int 0,
//!   `-0.0` the float;
//! - opening the `$depth`-th nested array or object fails at once
//!   (`JSON_ERROR_DEPTH`): at depth 512, 511 levels decode;
//! - the scanner's errors come first, in the order of the input: a
//!   control byte outside a string other than the four whitespace bytes,
//!   any control byte (NUL included) inside one, or the end of the input
//!   inside a string is `JSON_ERROR_CTRL_CHAR`; a byte that does not start
//!   valid UTF-8, inside a string or out, is `JSON_ERROR_UTF8`; a `\u`
//!   high surrogate not followed by a `\u` escape of a low one, or a low
//!   surrogate alone, is `JSON_ERROR_UTF16`;
//! - then the parser's: `]` where an object may close or `}` where an
//!   array may close is `JSON_ERROR_STATE_MISMATCH`; any other unexpected
//!   byte or token is `JSON_ERROR_SYNTAX`.
//!
//! Upstream: none

use crate::{Key, PhpArray, PhpString, Value};

/// A `json_decode()` error, as `json_last_error()` and
/// `json_last_error_msg()` report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonError {
    /// `JSON_ERROR_DEPTH`.
    Depth,
    /// `JSON_ERROR_STATE_MISMATCH`: `]` closing an object or `}` an
    /// array, where a closer may come.
    StateMismatch,
    /// `JSON_ERROR_CTRL_CHAR`.
    CtrlChar,
    /// `JSON_ERROR_SYNTAX`.
    Syntax,
    /// `JSON_ERROR_UTF8`.
    Utf8,
    /// `JSON_ERROR_UTF16`.
    Utf16,
}

impl JsonError {
    /// `json_last_error()`, the `\JsonException` code.
    pub const fn code(self) -> i64 {
        match self {
            Self::Depth => 1,
            Self::StateMismatch => 2,
            Self::CtrlChar => 3,
            Self::Syntax => 4,
            Self::Utf8 => 5,
            Self::Utf16 => 10,
        }
    }

    /// `json_last_error_msg()`, the `\JsonException` message.
    pub const fn message(self) -> &'static str {
        match self {
            Self::Depth => "Maximum stack depth exceeded",
            Self::StateMismatch => "State mismatch (invalid or malformed JSON)",
            Self::CtrlChar => "Control character error, possibly incorrectly encoded",
            Self::Syntax => "Syntax error",
            Self::Utf8 => "Malformed UTF-8 characters, possibly incorrectly encoded",
            Self::Utf16 => "Single unpaired UTF-16 surrogate in unicode escape",
        }
    }
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for JsonError {}

#[derive(Debug, PartialEq)]
enum Token {
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Comma,
    String(Vec<u8>),
    Value(Value),
    End,
}

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
    bigint_as_string: bool,
}

/// The length of the valid UTF-8 sequence at the start of `s`, if any.
fn utf8_len(s: &[u8]) -> Option<usize> {
    let len = match s.first()? {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    std::str::from_utf8(s.get(..len)?).ok().map(|_| len)
}

impl Scanner<'_> {
    fn next(&mut self) -> Result<Token, JsonError> {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
        let Some(&c) = self.s.get(self.i) else {
            return Ok(Token::End);
        };
        let token = match c {
            b'{' => Token::LBrace,
            b'}' => Token::RBrace,
            b'[' => Token::LBracket,
            b']' => Token::RBracket,
            b':' => Token::Colon,
            b',' => Token::Comma,
            b'"' => {
                self.i += 1;
                return self.string().map(Token::String);
            }
            b'-' | b'0'..=b'9' => return self.number(),
            b't' | b'f' | b'n' => {
                for (word, value) in [
                    (&b"true"[..], Value::Bool(true)),
                    (b"false", Value::Bool(false)),
                    (b"null", Value::Null),
                ] {
                    if self.s[self.i..].starts_with(word) {
                        self.i += word.len();
                        return Ok(Token::Value(value));
                    }
                }
                return Err(JsonError::Syntax);
            }
            0x00..=0x1F => return Err(JsonError::CtrlChar),
            0x80.. if utf8_len(&self.s[self.i..]).is_none() => return Err(JsonError::Utf8),
            _ => return Err(JsonError::Syntax),
        };
        self.i += 1;
        Ok(token)
    }

    fn digits(&mut self) {
        self.i += self.s[self.i..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
    }

    fn number(&mut self) -> Result<Token, JsonError> {
        let start = self.i;
        if self.s[self.i] == b'-' {
            self.i += 1;
        }
        match self.s.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(JsonError::Syntax),
        }
        let mut float = false;
        if self.s.get(self.i) == Some(&b'.')
            && self.s.get(self.i + 1).is_some_and(u8::is_ascii_digit)
        {
            self.i += 1;
            self.digits();
            float = true;
        }
        if let Some(b'e' | b'E') = self.s.get(self.i) {
            let sign = usize::from(matches!(self.s.get(self.i + 1), Some(b'+' | b'-')));
            if self
                .s
                .get(self.i + 1 + sign)
                .is_some_and(u8::is_ascii_digit)
            {
                self.i += 1 + sign;
                self.digits();
                float = true;
            }
        }
        // the token is ASCII: digits, a sign, `.` and `e`
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| JsonError::Syntax)?;
        let value = if float {
            Value::Float(text.parse().map_err(|_| JsonError::Syntax)?)
        } else if let Ok(i) = text.parse::<i64>() {
            Value::Int(i)
        } else if self.bigint_as_string {
            Value::string(text)
        } else {
            Value::Float(text.parse().map_err(|_| JsonError::Syntax)?)
        };
        Ok(Token::Value(value))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let digits = self.s.get(self.i..self.i + 4).ok_or(JsonError::Syntax)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(JsonError::Syntax);
        }
        self.i += 4;
        let text = std::str::from_utf8(digits).map_err(|_| JsonError::Syntax)?;
        u32::from_str_radix(text, 16).map_err(|_| JsonError::Syntax)
    }

    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), JsonError> {
        let byte = match self.s.get(self.i) {
            Some(b'"') => b'"',
            Some(b'\\') => b'\\',
            Some(b'/') => b'/',
            Some(b'b') => 0x08,
            Some(b'f') => 0x0C,
            Some(b'n') => b'\n',
            Some(b'r') => b'\r',
            Some(b't') => b'\t',
            Some(b'u') => {
                self.i += 1;
                let unit = self.hex4()?;
                let code = match unit {
                    0xD800..=0xDBFF => {
                        if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                            return Err(JsonError::Utf16);
                        }
                        self.i += 2;
                        let low = self.hex4().map_err(|_| JsonError::Utf16)?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(JsonError::Utf16);
                        }
                        0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                    }
                    0xDC00..=0xDFFF => return Err(JsonError::Utf16),
                    unit => unit,
                };
                let c = char::from_u32(code).ok_or(JsonError::Utf16)?;
                let mut buf = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                return Ok(());
            }
            _ => return Err(JsonError::Syntax),
        };
        out.push(byte);
        self.i += 1;
        Ok(())
    }

    fn string(&mut self) -> Result<Vec<u8>, JsonError> {
        let mut out = Vec::new();
        loop {
            let Some(&c) = self.s.get(self.i) else {
                return Err(JsonError::CtrlChar);
            };
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    self.escape(&mut out)?;
                }
                0x00..=0x1F => return Err(JsonError::CtrlChar),
                0x80.. => {
                    let len = utf8_len(&self.s[self.i..]).ok_or(JsonError::Utf8)?;
                    out.extend_from_slice(&self.s[self.i..self.i + len]);
                    self.i += len;
                }
                c => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
    }
}

/// An array or object being read, with an object's key for its next value.
enum Frame {
    List(PhpArray),
    Map(PhpArray, Key),
}

/// `json_decode($json, true, $depth, $bigint_as_string ?
/// JSON_BIGINT_AS_STRING : 0)` with `JSON_THROW_ON_ERROR`: the value, or
/// the error PHP throws. Iterative, so deep input does not grow the stack.
pub fn decode_assoc(json: &[u8], depth: usize, bigint_as_string: bool) -> Result<Value, JsonError> {
    let mut scanner = Scanner {
        s: json,
        i: 0,
        bigint_as_string,
    };
    let mut stack: Vec<Frame> = Vec::new();
    let mut token = scanner.next()?;
    loop {
        // `token` starts a value
        let mut value = match token {
            Token::LBracket | Token::LBrace => {
                if stack.len() + 1 >= depth {
                    return Err(JsonError::Depth);
                }
                let list = token == Token::LBracket;
                match (list, scanner.next()?) {
                    (true, Token::RBracket) | (false, Token::RBrace) => {
                        Value::Array(PhpArray::new())
                    }
                    (true, Token::RBrace) | (false, Token::RBracket) => {
                        return Err(JsonError::StateMismatch);
                    }
                    (true, next) => {
                        stack.push(Frame::List(PhpArray::new()));
                        token = next;
                        continue;
                    }
                    (false, Token::String(key)) => {
                        if scanner.next()? != Token::Colon {
                            return Err(JsonError::Syntax);
                        }
                        stack.push(Frame::Map(PhpArray::new(), Key::from_bytes(&key)));
                        token = scanner.next()?;
                        continue;
                    }
                    (false, _) => return Err(JsonError::Syntax),
                }
            }
            Token::String(s) => Value::String(PhpString::from(s)),
            Token::Value(v) => v,
            _ => return Err(JsonError::Syntax),
        };
        // a value is complete: add it to its container, closing those that
        // end here
        loop {
            match stack.last_mut() {
                None => {
                    return match scanner.next()? {
                        Token::End => Ok(value),
                        _ => Err(JsonError::Syntax),
                    };
                }
                Some(Frame::List(list)) => {
                    let index = i64::try_from(list.len()).map_err(|_| JsonError::Depth)?;
                    list.insert(index, value);
                    match scanner.next()? {
                        Token::Comma => {
                            token = scanner.next()?;
                            break;
                        }
                        Token::RBracket => {}
                        Token::RBrace => return Err(JsonError::StateMismatch),
                        _ => return Err(JsonError::Syntax),
                    }
                }
                Some(Frame::Map(map, key)) => {
                    map.insert(key.clone(), value);
                    match scanner.next()? {
                        Token::Comma => {
                            let Token::String(next) = scanner.next()? else {
                                return Err(JsonError::Syntax);
                            };
                            *key = Key::from_bytes(&next);
                            if scanner.next()? != Token::Colon {
                                return Err(JsonError::Syntax);
                            }
                            token = scanner.next()?;
                            break;
                        }
                        Token::RBrace => {}
                        Token::RBracket => return Err(JsonError::StateMismatch),
                        _ => return Err(JsonError::Syntax),
                    }
                }
            }
            value = match stack.pop() {
                Some(Frame::List(a) | Frame::Map(a, _)) => Value::Array(a),
                None => return Err(JsonError::Syntax),
            };
        }
    }
}

//! Port of Symfony\Component\Yaml\Exception\ParseException.php,
//! DumpException.php and RuntimeException.php.
//!
//! The message is built as PHP builds it: the raw message without its
//! final dot, then ` in "<file>"` (the file name through `json_encode()`
//! with unescaped slashes and Unicode), ` at line N` when the line is not
//! negative, ` (near "<snippet>")` when the snippet is truthy in PHP (not
//! empty and not `"0"`), then the dot again.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Exception/ParseException.php Exception/DumpException.php Exception/RuntimeException.php

use std::fmt;

use php_value::{
    PhpString,
    json::{JsonFlags, encode_string},
};

/// Thrown when an error occurs during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseException {
    raw_message: String,
    parsed_line: i64,
    snippet: Option<PhpString>,
    parsed_file: Option<String>,
    message: PhpString,
}

impl ParseException {
    pub fn new(
        raw_message: impl Into<String>,
        parsed_line: i64,
        snippet: Option<PhpString>,
        parsed_file: Option<String>,
    ) -> Self {
        let mut exception = Self {
            raw_message: raw_message.into(),
            parsed_line,
            snippet,
            parsed_file,
            message: PhpString::default(),
        };
        exception.update_repr();
        exception
    }

    /// A message without line, snippet or file (`-1` line).
    pub fn message_only(raw_message: impl Into<String>) -> Self {
        Self::new(raw_message, -1, None, None)
    }

    /// `getMessage()`. Bytes: the snippet is a slice of the input.
    pub fn get_message(&self) -> &PhpString {
        &self.message
    }

    pub fn raw_message(&self) -> &str {
        &self.raw_message
    }

    pub fn get_snippet(&self) -> Option<&PhpString> {
        self.snippet.as_ref()
    }

    pub fn set_snippet(&mut self, snippet: PhpString) {
        self.snippet = Some(snippet);
        self.update_repr();
    }

    pub fn get_parsed_file(&self) -> Option<&str> {
        self.parsed_file.as_deref()
    }

    pub fn set_parsed_file(&mut self, parsed_file: impl Into<String>) {
        self.parsed_file = Some(parsed_file.into());
        self.update_repr();
    }

    pub fn get_parsed_line(&self) -> i64 {
        self.parsed_line
    }

    pub fn set_parsed_line(&mut self, parsed_line: i64) {
        self.parsed_line = parsed_line;
        self.update_repr();
    }

    fn update_repr(&mut self) {
        let mut message: Vec<u8> = self.raw_message.as_bytes().to_vec();
        let dot = message.last() == Some(&b'.');
        if dot {
            message.pop();
        }
        if let Some(file) = &self.parsed_file {
            let flags = JsonFlags {
                unescaped_slashes: true,
                unescaped_unicode: true,
            };
            // json_encode() of invalid UTF-8 is false, which prints as "".
            let encoded = encode_string(file.as_bytes(), flags).unwrap_or_default();
            message.extend_from_slice(b" in ");
            message.extend_from_slice(encoded.as_bytes());
        }
        if self.parsed_line >= 0 {
            message.extend_from_slice(format!(" at line {}", self.parsed_line).as_bytes());
        }
        if let Some(snippet) = &self.snippet
            && !snippet.is_empty()
            && snippet.as_bytes() != b"0"
        {
            message.extend_from_slice(b" (near \"");
            message.extend_from_slice(snippet);
            message.extend_from_slice(b"\")");
        }
        if dot {
            message.push(b'.');
        }
        self.message = PhpString::from(message);
    }
}

impl fmt::Display for ParseException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseException {}

/// Thrown when an error occurs during dumping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpException {
    pub message: String,
}

impl DumpException {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DumpException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DumpException {}

/// PHP's `\InvalidArgumentException` from the dumper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidArgumentException {
    pub message: String,
}

impl fmt::Display for InvalidArgumentException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InvalidArgumentException {}

/// What `Dumper::dump()` and `Yaml::dump()` throw: PHP's
/// `\InvalidArgumentException` for arguments they refuse, or a
/// `DumpException` from the inline dumper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DumpError {
    InvalidArgument(InvalidArgumentException),
    Dump(DumpException),
}

impl DumpError {
    /// The PHP exception class.
    pub fn class(&self) -> &'static str {
        match self {
            Self::InvalidArgument(_) => "InvalidArgumentException",
            Self::Dump(_) => r"Symfony\Component\Yaml\Exception\DumpException",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument(e) => &e.message,
            Self::Dump(e) => &e.message,
        }
    }
}

impl fmt::Display for DumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for DumpError {}

impl From<DumpException> for DumpError {
    fn from(e: DumpException) -> Self {
        Self::Dump(e)
    }
}

impl From<InvalidArgumentException> for DumpError {
    fn from(e: InvalidArgumentException) -> Self {
        Self::InvalidArgument(e)
    }
}

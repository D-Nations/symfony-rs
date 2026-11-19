//! Port of Symfony\Component\Dotenv\Exception: FormatException (with its
//! FormatExceptionContext), PathException and
//! VariableCircularReferenceException; and the `\LogicException`s
//! Dotenv.php throws itself.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dotenv Exception/ExceptionInterface.php Exception/FormatException.php Exception/FormatExceptionContext.php Exception/PathException.php Exception/VariableCircularReferenceException.php

use std::fmt;

/// Where a syntax error is: the data, the file, the line and the offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatExceptionContext {
    data: Vec<u8>,
    path: String,
    lineno: i64,
    cursor: usize,
}

impl FormatExceptionContext {
    pub fn new(
        data: impl Into<Vec<u8>>,
        path: impl Into<String>,
        lineno: i64,
        cursor: usize,
    ) -> Self {
        Self {
            data: data.into(),
            path: path.into(),
            lineno,
            cursor,
        }
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_lineno(&self) -> i64 {
        self.lineno
    }

    /// Up to 20 bytes before and after the cursor (newlines as `\n`),
    /// then a caret under the cursor.
    pub fn get_details(&self) -> Vec<u8> {
        let escape = |s: &[u8]| -> Vec<u8> {
            let mut out = Vec::with_capacity(s.len());
            for &b in s {
                if b == b'\n' {
                    out.extend_from_slice(b"\\n");
                } else {
                    out.push(b);
                }
            }
            out
        };
        let substr = |start: usize, len: usize| -> &[u8] {
            let start = start.min(self.data.len());
            let end = (start + len).min(self.data.len());
            &self.data[start..end]
        };
        let before = escape(substr(self.cursor.saturating_sub(20), self.cursor.min(20)));
        let after = escape(substr(self.cursor, 20));
        let mut out = b"...".to_vec();
        out.extend_from_slice(&before);
        out.extend_from_slice(&after);
        out.extend_from_slice(b"...\n");
        out.extend(std::iter::repeat_n(b' ', before.len() + 2));
        out.extend_from_slice(format!("^ line {} offset {}", self.lineno, self.cursor).as_bytes());
        out
    }
}

/// A syntax error in an .env file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatException {
    message: Vec<u8>,
    context: FormatExceptionContext,
}

impl FormatException {
    pub const CLASS: &'static str = r"Symfony\Component\Dotenv\Exception\FormatException";

    pub fn new(message: impl AsRef<[u8]>, context: FormatExceptionContext) -> Self {
        let mut full = message.as_ref().to_vec();
        full.extend_from_slice(
            format!(
                " in \"{}\" at line {}.\n",
                context.get_path(),
                context.get_lineno()
            )
            .as_bytes(),
        );
        full.extend(context.get_details());
        Self {
            message: full,
            context,
        }
    }

    /// `getMessage()`: bytes, as the data around the error is.
    pub fn get_message(&self) -> &[u8] {
        &self.message
    }

    pub fn get_context(&self) -> &FormatExceptionContext {
        &self.context
    }
}

/// An .env file that does not exist, cannot be read or is a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathException {
    message: String,
}

impl PathException {
    pub const CLASS: &'static str = r"Symfony\Component\Dotenv\Exception\PathException";

    pub fn new(path: &str) -> Self {
        Self {
            message: format!("Unable to read the \"{path}\" environment file."),
        }
    }

    pub fn get_message(&self) -> &str {
        &self.message
    }
}

/// What the Dotenv methods throw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Format(FormatException),
    Path(PathException),
    /// `VariableCircularReferenceException`.
    VariableCircularReference(String),
    /// PHP's `\LogicException` (commands on Windows; an `.env.local.php`
    /// that is not a plain array, which the port cannot run).
    Logic(String),
}

impl Error {
    /// The PHP exception class.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Format(_) => FormatException::CLASS,
            Self::Path(_) => PathException::CLASS,
            Self::VariableCircularReference(_) => {
                r"Symfony\Component\Dotenv\Exception\VariableCircularReferenceException"
            }
            Self::Logic(_) => "LogicException",
        }
    }

    /// `getMessage()`.
    pub fn message(&self) -> Vec<u8> {
        match self {
            Self::Format(e) => e.get_message().to_vec(),
            Self::Path(e) => e.get_message().as_bytes().to_vec(),
            Self::VariableCircularReference(m) | Self::Logic(m) => m.as_bytes().to_vec(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.message()))
    }
}

impl std::error::Error for Error {}

impl From<FormatException> for Error {
    fn from(e: FormatException) -> Self {
        Self::Format(e)
    }
}

impl From<PathException> for Error {
    fn from(e: PathException) -> Self {
        Self::Path(e)
    }
}

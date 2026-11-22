//! The errors libxml reports while parsing, as PHP's `libxml_get_errors()`
//! returns them (`LibXMLError`: level, code, message, file, line, column),
//! and the inputs this crate refuses instead of approximating.
//!
//! Upstream: none

use std::fmt;

/// `LIBXML_ERR_WARNING`, `LIBXML_ERR_ERROR`, `LIBXML_ERR_FATAL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorLevel {
    Warning,
    Error,
    Fatal,
}

impl ErrorLevel {
    /// The integer PHP exposes as `LibXMLError::$level`.
    pub fn as_int(self) -> u8 {
        match self {
            ErrorLevel::Warning => 1,
            ErrorLevel::Error => 2,
            ErrorLevel::Fatal => 3,
        }
    }
}

/// One entry of `libxml_get_errors()`. `message` is stored trimmed, as
/// every Symfony caller reads it (`trim($error->message)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibxmlError {
    pub level: ErrorLevel,
    pub code: u32,
    pub message: String,
    /// `None` for documents loaded from a string (PHP's `''`).
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
}

/// Why a document was not parsed at all. Nothing in this crate reads a
/// file or the network; documents that would need it, and encodings it
/// does not decode, are refused with the position where the need arose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// `DOMDocument::loadXML('')`, which PHP rejects with a `ValueError`.
    Empty,
    Refused(Refusal),
}

impl LoadError {
    /// PHP's message for [`LoadError::Empty`].
    pub const EMPTY_MESSAGE: &'static str =
        "DOMDocument::loadXML(): Argument #1 ($source) must not be empty";
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub reason: RefusalReason,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefusalReason {
    /// A document type with a `SYSTEM` or `PUBLIC` identifier: libxml
    /// would load the external subset (PHP's `validateOnParse`).
    ExternalSubset { system_id: String },
    /// A reference in content to an external parsed entity, which libxml
    /// loads when validating.
    ExternalEntity { name: String },
    /// A parameter-entity reference to an external entity.
    ExternalParameterEntity { name: String },
    /// An encoding PHP decodes through iconv, or a detected encoding
    /// (UCS-4, EBCDIC) this crate does not decode.
    Encoding { name: String },
    /// A byte sequence that is not valid in the document's UTF-16.
    InvalidUtf16 { offset: usize },
    /// A schema construct outside the set this crate validates (stage
    /// log docs/milestones/m1-c-xml-x2.md); `line` is in the schema.
    UnsupportedSchema { what: String },
    /// An `<xs:import>` whose document the resolver did not supply (libxml
    /// would read it from a file or the network).
    UnresolvedImport { location: String },
    /// An instance construct the validator does not model (`xsi:type`,
    /// entity references); `line` is in the document.
    UnsupportedInstance { what: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Empty => f.write_str(Self::EMPTY_MESSAGE),
            LoadError::Refused(r) => r.fmt(f),
        }
    }
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusalReason::ExternalSubset { system_id } => {
                write!(f, "external DTD subset \"{system_id}\" is not loaded")
            }
            RefusalReason::ExternalEntity { name } => {
                write!(f, "external entity '{name}' is not loaded")
            }
            RefusalReason::ExternalParameterEntity { name } => {
                write!(f, "external parameter entity '%{name}' is not loaded")
            }
            RefusalReason::Encoding { name } => write!(f, "encoding \"{name}\" is not supported"),
            RefusalReason::InvalidUtf16 { offset } => write!(f, "invalid UTF-16 at byte {offset}"),
            RefusalReason::UnsupportedSchema { what } => {
                write!(f, "schema construct not supported: {what}")
            }
            RefusalReason::UnresolvedImport { location } => {
                write!(f, "imported schema \"{location}\" is not loaded")
            }
            RefusalReason::UnsupportedInstance { what } => {
                write!(
                    f,
                    "document construct not supported by schema validation: {what}"
                )
            }
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (line {}, column {})",
            self.reason, self.line, self.column
        )
    }
}

impl std::error::Error for LoadError {}

//! Port of Symfony\Component\Config\Util\XmlUtils.php, on `php-dom`
//! (PLAN.md decision 9): `parse()`, `loadFile()`,
//! `convertDomElementToArray()`, `phpize()` and `getXmlErrors()`.
//!
//! What `DOMDocument` does is `php_dom`'s: `loadXML()` with
//! `validateOnParse` and `schemaValidateSource()`, with libxml 2.9.14's
//! errors. libxml's error buffer is the list each call returns; a
//! validation callable adds to it through the list it is given.
//! `libxml_use_internal_errors()` is `php_dom::libxml_use_internal_errors`,
//! set and restored as upstream does (also where upstream leaves it set:
//! after `loadXML('')`'s ValueError or a callable's `\Error`).
//!
//! Where `php_dom` refuses a document or schema (it would read a file or
//! the network, or uses an encoding or schema construct it does not
//! implement), the refusal is an XmlParsingException carrying its text
//! (PLAN.md decision 14), a stated difference from PHP.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Util/XmlUtils.php

use std::path::{Path, PathBuf};

use php_dom::{
    Document, ErrorLevel, LibxmlError, LoadError, LoadOptions, NodeId, NodeType, Refusal,
    libxml_use_internal_errors, load_xml, schema_validate_source,
};
use php_value::{
    Key, Number, PhpArray, PhpString, Value,
    numeric::{hexdec, is_numeric, string_to_float, string_to_int},
};

use super::exception::{invalid_xml, xml_parsing};
use crate::{
    Class, Error,
    php::{bindec, ctype_digit, decoct, intval_octal, trim, truthy},
};

/// A validation callable: `$schemaOrCallable($dom, $internalErrors)`. It
/// receives the document, the caller's `libxml_use_internal_errors()`
/// setting and libxml's error buffer (the errors of what it validates go
/// there, as `schemaValidateSource()` leaves them in PHP). Its result is
/// tested for truth; an `\Exception` it throws makes the document invalid
/// and becomes the exception's previous one, an `\Error` propagates.
pub type ValidationCallable<'a> =
    &'a mut dyn FnMut(&Document, bool, &mut Vec<LibxmlError>) -> Result<Value, Error>;

/// `$schemaOrCallable` when not `null`.
pub enum Validation<'a> {
    /// An XSD schema file path. (In PHP a string naming a function, such as
    /// `"dir"`, is a callable instead; here the caller chooses.)
    Schema(&'a Path),
    Callable(ValidationCallable<'a>),
}

/// `XmlUtils`: static methods only, as upstream.
pub struct XmlUtils {
    _private: (),
}

const DOCUMENT_TYPES: &[u8] = b"Document types are not allowed.";

impl XmlUtils {
    /// `XmlUtils::parse($content, $schemaOrCallable)`.
    pub fn parse(content: &[u8], validation: Option<Validation<'_>>) -> Result<Document, Error> {
        let internal_errors = libxml_use_internal_errors(Some(true));

        let options = LoadOptions {
            validate_on_parse: true,
        };
        let loaded = match load_xml(content, options) {
            Ok(loaded) => loaded,
            Err(LoadError::Empty) => {
                // loadXML() throws before the setting is restored.
                return Err(Error::new(
                    Class::Other {
                        class: "ValueError".into(),
                        exception: false,
                    },
                    LoadError::EMPTY_MESSAGE,
                ));
            }
            Err(LoadError::Refused(refusal)) => {
                libxml_use_internal_errors(Some(internal_errors));
                return Err(refused(&refusal));
            }
        };
        let Some(dom) = loaded.document else {
            let messages = Self::get_xml_errors(&loaded.errors, internal_errors);
            return Err(xml_parsing(join(&messages), None));
        };

        libxml_use_internal_errors(Some(internal_errors));

        let root = dom.root();
        if dom
            .children(root)
            .iter()
            .any(|&child| dom.node_type(child) == NodeType::DocumentType)
        {
            return Err(xml_parsing(DOCUMENT_TYPES, None));
        }

        if let Some(validation) = validation {
            let internal_errors = libxml_use_internal_errors(Some(true));
            let mut errors = Vec::new();
            let mut previous = None;
            let valid = match validation {
                Validation::Callable(callable) => {
                    match callable(&dom, internal_errors, &mut errors) {
                        Ok(result) => truthy(&result),
                        Err(e) if e.class().is_exception() => {
                            previous = Some(e);
                            false
                        }
                        Err(e) => return Err(e),
                    }
                }
                Validation::Schema(path) if path.is_file() => {
                    let source = match read_file(path) {
                        Ok(source) => source,
                        Err(e) => {
                            libxml_use_internal_errors(Some(internal_errors));
                            return Err(e);
                        }
                    };
                    let cwd = std::env::current_dir().ok();
                    let document_file = cwd.as_deref().map(document_url);
                    let mut resolve = |location: &str| resolve_import(location, cwd.as_deref());
                    match schema_validate_source(
                        &dom,
                        &source,
                        &mut resolve,
                        document_file.as_deref(),
                    ) {
                        Ok(result) => {
                            errors.extend(result.errors);
                            result.valid
                        }
                        Err(refusal) => {
                            libxml_use_internal_errors(Some(internal_errors));
                            return Err(refused(&refusal));
                        }
                    }
                }
                Validation::Schema(path) => {
                    libxml_use_internal_errors(Some(internal_errors));
                    let mut message = b"Invalid XSD file: \"".to_vec();
                    message.extend_from_slice(&path_bytes(path));
                    message.extend_from_slice(b"\".");
                    return Err(xml_parsing(message, None));
                }
            };

            if !valid {
                let messages = Self::get_xml_errors(&errors, internal_errors);
                if messages.is_empty() {
                    return Err(invalid_xml("The XML is not valid.", previous));
                }
                return Err(xml_parsing(join(&messages), previous));
            }
        }

        libxml_use_internal_errors(Some(internal_errors));

        Ok(dom)
    }

    /// `XmlUtils::loadFile($file, $schemaOrCallable)`.
    pub fn load_file(file: &Path, validation: Option<Validation<'_>>) -> Result<Document, Error> {
        Self::load_file_named(file, &path_bytes(file), validation)
    }

    /// `XmlUtils::loadFile($file, $schemaOrCallable)` where `$file` is
    /// `name` and the file system finds it at `file` (a `file://` URL,
    /// which PHP's functions open as the path it names).
    pub fn load_file_named(
        file: &Path,
        name: &[u8],
        validation: Option<Validation<'_>>,
    ) -> Result<Document, Error> {
        let message = |before: &str, after: &str| {
            let mut m = before.as_bytes().to_vec();
            m.extend_from_slice(name);
            m.extend_from_slice(after.as_bytes());
            m
        };
        if !file.is_file() {
            return Err(Error::new(
                Class::InvalidArgument,
                message("Resource \"", "\" is not a file."),
            ));
        }
        if std::fs::File::open(file).is_err() {
            return Err(Error::new(
                Class::InvalidArgument,
                message("File \"", "\" is not readable."),
            ));
        }

        let content = read_file_named(file, name)?;

        if trim(&content).is_empty() {
            return Err(Error::new(
                Class::InvalidArgument,
                message("File \"", "\" does not contain valid XML, it is empty."),
            ));
        }

        match Self::parse(&content, validation) {
            Err(e) if *e.class() == Class::InvalidXml => Err(xml_parsing(
                message("The XML file \"", "\" is not valid."),
                e.get_previous().cloned(),
            )),
            other => other,
        }
    }

    /// `XmlUtils::convertDomElementToArray($element, $checkPrefix)`:
    /// attributes and child elements become keys, text the value (under
    /// `"value"` when there are keys), repeated keys a list; `null` for an
    /// empty element. A child that is neither text, a comment nor an
    /// element (a processing instruction, an entity reference) reaches the
    /// `\DOMElement` parameter: PHP's TypeError.
    pub fn convert_dom_element_to_array(
        dom: &Document,
        element: NodeId,
        check_prefix: bool,
    ) -> Result<Value, Error> {
        if dom.node_type(element) != NodeType::Element {
            return Err(Error::new(
                Class::TypeError,
                format!(
                    "Symfony\\Component\\Config\\Util\\XmlUtils::convertDomElementToArray(): Argument #1 ($element) must be of type DOMElement, {} given",
                    dom_class(dom.node_type(element))
                ),
            ));
        }
        let prefix = dom.prefix(element).unwrap_or("");
        let mut empty = true;
        let mut config = PhpArray::new();
        for attribute in dom.attributes(element) {
            let attribute_prefix = attribute.prefix.as_deref().unwrap_or("");
            if check_prefix && !(attribute_prefix.is_empty() || attribute_prefix == prefix) {
                continue;
            }
            config.insert(
                Key::from_bytes(attribute.local_name.as_bytes()),
                Self::phpize(attribute.value.as_bytes()),
            );
            empty = false;
        }

        let mut node_value: Option<&[u8]> = None;
        for &node in dom.children(element) {
            match dom.node_type(node) {
                NodeType::Text | NodeType::CData => {
                    let text = trim(dom.node_value(node).unwrap_or("").as_bytes());
                    if !text.is_empty() {
                        node_value = Some(text);
                        empty = false;
                    }
                }
                // `$prefix != $node->prefix`: a non-element's prefix is
                // null, equal to ''.
                _ if check_prefix && prefix != dom.prefix(node).unwrap_or("") => {}
                NodeType::Comment => {}
                _ => {
                    let value = Self::convert_dom_element_to_array(dom, node, check_prefix)?;
                    let key = Key::from_bytes(dom.local_name(node).unwrap_or("").as_bytes());
                    match config.get(&key) {
                        Some(existing) if !matches!(existing, Value::Null) => {
                            let is_list = match existing {
                                Value::Array(a) => matches!(a.first(), Some((Key::Int(_), _))),
                                _ => false,
                            };
                            if !is_list {
                                let mut list = PhpArray::new();
                                push(&mut list, existing.clone())?;
                                config.insert(key.clone(), Value::Array(list));
                            }
                            if let Some(Value::Array(list)) = config.get_mut(&key) {
                                push(list, value)?;
                            }
                        }
                        _ => config.insert(key, value),
                    }
                    empty = false;
                }
            }
        }

        let mut result = Value::Array(config);
        if let Some(text) = node_value {
            let value = Self::phpize(text);
            match &mut result {
                Value::Array(config) if !config.is_empty() => config.insert("value", value),
                _ => result = value,
            }
        }

        Ok(if empty { Value::Null } else { result })
    }

    /// `XmlUtils::phpize($value)`: an XML value as a PHP value (`null`,
    /// booleans, decimal, octal, binary and hexadecimal ints, floats), or
    /// the string itself.
    pub fn phpize(value: &[u8]) -> Value {
        let lowercase = value.to_ascii_lowercase();

        if lowercase == b"null" {
            return Value::Null;
        }
        if ctype_digit(value) || (value.len() > 1 && value[0] == b'-' && ctype_digit(&value[1..])) {
            let cast = string_to_int(value);
            if is_octal(value) {
                return Value::Int(intval_octal(value));
            }
            return if cast.to_string().as_bytes() == value {
                Value::Int(cast)
            } else {
                Value::String(PhpString::from(value))
            };
        }
        if lowercase == b"true" {
            return Value::Bool(true);
        }
        if lowercase == b"false" {
            return Value::Bool(false);
        }
        if value.len() > 1
            && value.starts_with(b"0b")
            && matches_to_end(&value[2..], |b| matches!(b, b'0' | b'1'))
        {
            return number(bindec(value));
        }
        if is_numeric(value) {
            return if value.starts_with(b"0x") {
                number(hexdec(value))
            } else {
                Value::Float(string_to_float(value))
            };
        }
        if lowercase.starts_with(b"0x")
            && value.len() > 2
            && value[2..] != *b"\n"
            && matches_to_end(&value[2..], |b| b.is_ascii_hexdigit())
        {
            return number(hexdec(value));
        }
        if is_decimal(value) {
            return Value::Float(string_to_float(value));
        }

        Value::String(PhpString::from(value))
    }

    /// `XmlUtils::getXmlErrors($internalErrors)`: libxml's errors as
    /// `[LEVEL code] message (in file - line l, column c)`; restores the
    /// internal-errors setting.
    pub fn get_xml_errors(errors: &[LibxmlError], internal_errors: bool) -> Vec<Vec<u8>> {
        let formatted = errors
            .iter()
            .map(|error| {
                let file = match error.file.as_deref() {
                    None | Some("") | Some("0") => "n/a",
                    Some(file) => file,
                };
                format!(
                    "[{} {}] {} (in {} - line {}, column {})",
                    if error.level == ErrorLevel::Warning {
                        "WARNING"
                    } else {
                        "ERROR"
                    },
                    error.code,
                    error
                        .message
                        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B')),
                    file,
                    error.line,
                    error.column
                )
                .into_bytes()
            })
            .collect();
        libxml_use_internal_errors(Some(internal_errors));
        formatted
    }
}

/// `isOctal($str)`.
fn is_octal(s: &[u8]) -> bool {
    let digits = s.strip_prefix(b"-").unwrap_or(s);
    let mut expected = b"0".to_vec();
    expected.extend_from_slice(decoct(intval_octal(digits)).as_bytes());
    digits == expected
}

/// `preg_match('/^…*$/')` over the rest of a subject: every byte matches,
/// except that `$` also matches before a final newline.
fn matches_to_end(rest: &[u8], class: impl Fn(u8) -> bool) -> bool {
    let rest = rest.strip_suffix(b"\n").unwrap_or(rest);
    rest.iter().all(|&b| class(b))
}

/// `preg_match('/^[+-]?[0-9]+(\.[0-9]+)?$/', $value)`.
fn is_decimal(value: &[u8]) -> bool {
    let s = value.strip_suffix(b"\n").unwrap_or(value);
    let s = s
        .strip_prefix(b"+")
        .or_else(|| s.strip_prefix(b"-"))
        .unwrap_or(s);
    let (int, fraction) = match s.iter().position(|&b| b == b'.') {
        Some(dot) => (&s[..dot], Some(&s[dot + 1..])),
        None => (s, None),
    };
    ctype_digit(int) && fraction.is_none_or(ctype_digit)
}

fn number(n: Number) -> Value {
    match n {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

fn push(list: &mut PhpArray, value: Value) -> Result<(), Error> {
    list.push(value).map(|_| ()).map_err(|_| {
        Error::new(
            Class::Error,
            "Cannot add element to the array as the next element is already occupied",
        )
    })
}

fn join(messages: &[Vec<u8>]) -> Vec<u8> {
    messages.join(&b'\n')
}

/// A php-dom refusal, as PLAN.md decision 14 reports it.
fn refused(refusal: &Refusal) -> Error {
    xml_parsing(refusal.to_string(), None)
}

/// PHP's class for a node of this type.
fn dom_class(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Element => "DOMElement",
        NodeType::Text => "DOMText",
        NodeType::CData => "DOMCdataSection",
        NodeType::EntityReference => "DOMEntityReference",
        NodeType::ProcessingInstruction => "DOMProcessingInstruction",
        NodeType::Comment => "DOMComment",
        NodeType::Document => "DOMDocument",
        NodeType::DocumentType => "DOMDocumentType",
    }
}

/// `(new Filesystem())->readFile($filename)`: the content, or
/// Filesystem's IOException.
fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    read_file_named(path, &path_bytes(path))
}

fn read_file_named(path: &Path, name: &[u8]) -> Result<Vec<u8>, Error> {
    let io = |reason: String| {
        let mut message = b"Failed to read file \"".to_vec();
        message.extend_from_slice(name);
        message.extend_from_slice(b"\": ");
        message.extend_from_slice(reason.as_bytes());
        Error::new(
            Class::Other {
                class: "Symfony\\Component\\Filesystem\\Exception\\IOException".into(),
                exception: true,
            },
            message,
        )
    };
    if path.is_dir() {
        return Err(io("File is a directory.".into()));
    }
    std::fs::read(path).map_err(|e| {
        let reason = match e.kind() {
            std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
            std::io::ErrorKind::NotFound => "No such file or directory".to_string(),
            _ => e.to_string(),
        };
        io(format!(
            "file_get_contents({}): Failed to open stream: {reason}",
            String::from_utf8_lossy(name)
        ))
    })
}

/// The document URL libxml gives a document loaded from a string, which
/// its schema errors name: the working directory and `/`.
fn document_url(cwd: &Path) -> String {
    let mut url = String::from_utf8_lossy(&path_bytes(cwd)).into_owned();
    url.push('/');
    url
}

/// Where libxml finds an `<xs:import>`'s `schemaLocation` for a schema
/// read from memory (recorded): a `file://` URL or a path, `%XX` escapes
/// decoded, a relative path against the working directory. A location
/// with another scheme (libxml would use the network), with a space (libxml
/// skips it) or that cannot be read is not supplied, which php-dom refuses.
fn resolve_import(location: &str, cwd: Option<&Path>) -> Option<Vec<u8>> {
    if location.contains(' ') {
        return None;
    }
    let path = match location.strip_prefix("file://") {
        Some(rest) => {
            if !rest.starts_with('/') {
                return None;
            }
            let decoded = percent_decode(rest)?;
            let decoded = if cfg!(windows) && decoded.get(2..3) == Some(":") {
                decoded[1..].to_string()
            } else {
                decoded
            };
            PathBuf::from(decoded)
        }
        None => {
            if has_scheme(location) {
                return None;
            }
            let path = PathBuf::from(percent_decode(location)?);
            if path.is_absolute() {
                path
            } else {
                cwd?.join(path)
            }
        }
    };
    std::fs::read(path).ok()
}

/// An RFC 3986 scheme followed by `:` (not a Windows drive letter).
fn has_scheme(location: &str) -> bool {
    match location.find(':') {
        Some(colon) if colon > 1 => {
            let scheme = &location[..colon];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        _ => false,
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A path as the bytes PHP has for it.
fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(not(unix))]
    {
        path.to_string_lossy().into_owned().into_bytes()
    }
}

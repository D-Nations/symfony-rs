//! Port of Symfony\Component\Translation\Util\XliffUtils.php, on
//! `php-dom` (PLAN.md decision 9).
//!
//! `validateSchema()` validates against the XLIFF 1.2 transitional, 2.0 or
//! 2.2 schema that symfony/translation ships (copied to `resources/`),
//! with the `xml.xsd` import rewritten to a `file:` URL as upstream does.
//! php-dom reads imports only through a resolver: the one here supplies the
//! bundled `xml.xsd` for that URL and nothing else. Upstream's `phar://`
//! branches (a temporary copy of `xml.xsd`) have no counterpart, since the
//! schemas are part of the binary.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Util/XliffUtils.php

use php_dom::{Document, ErrorLevel, Refusal, libxml_use_internal_errors, schema_validate_source};

use crate::exception::{Class, Error};

const XLIFF_12: &[u8] = include_bytes!("../../resources/schemas/xliff-core-1.2-transitional.xsd");
const XLIFF_20: &[u8] = include_bytes!("../../resources/schemas/xliff-core-2.0.xsd");
const XLIFF_22: &[u8] = include_bytes!("../../resources/schemas/xliff-core-2.2.xsd");
const XML_XSD: &[u8] = include_bytes!("../../resources/schemas/xml.xsd");

/// `__DIR__.'/../Resources/schemas/xml.xsd'`: where this crate keeps it.
const XML_XSD_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/schemas/xml.xsd");

/// One entry of `validateSchema()`'s list (from `libxml_get_errors()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlError {
    /// `'WARNING'` or `'ERROR'` (fatal errors are `'ERROR'` too).
    pub level: &'static str,
    pub code: u32,
    /// Trimmed.
    pub message: String,
    /// The document's URL, or `'n/a'`.
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// What a validation that php-dom could not run gives instead of errors:
/// a schema or document construct it does not model (decision 14's
/// counterpart for schemas), with its location.
pub type ValidationRefusal = Refusal;

/// `XliffUtils`: static methods only, as upstream.
pub struct XliffUtils {
    _private: (),
}

impl XliffUtils {
    /// `getVersionNumber($dom)`: the `version` attribute of the first
    /// `xliff` element that has one (an element named `xliff` with any
    /// namespace, an attribute `version` with any prefix, the first one), else `'1.2'`.
    /// Upstream's fallback to the `xmlns` attribute never applies: in PHP's
    /// DOM a namespace declaration is not an attribute.
    pub fn get_version_number(dom: &Document) -> String {
        for xliff in dom.get_elements_by_tag_name(dom.root(), "xliff") {
            if let Some(version) = named_item(dom.attributes(xliff), "version") {
                return version.to_owned();
            }
        }
        "1.2".to_owned()
    }

    /// `validateSchema($dom)`: libxml's errors, or none when the document
    /// is valid. `Err(Class::InvalidArgument)` for a version without a
    /// schema; a refusal when php-dom cannot validate the document.
    pub fn validate_schema(
        dom: &Document,
    ) -> Result<Result<Vec<XmlError>, ValidationRefusal>, Error> {
        let version = Self::get_version_number(dom);
        let internal_errors = libxml_use_internal_errors(Some(true));

        let schema = match get_schema(&version) {
            Ok(schema) => schema,
            Err(e) => {
                // upstream throws with the setting still on
                return Err(e);
            }
        };
        let new_path = Self::get_file_url(XML_XSD_PATH.as_bytes());
        let mut resolve = |location: &str| -> Option<Vec<u8>> {
            (location.as_bytes() == new_path.as_slice()).then(|| XML_XSD.to_vec())
        };
        let document_file = std::env::current_dir().ok().map(|cwd| {
            let mut url = cwd.to_string_lossy().into_owned();
            url.push('/');
            url
        });

        let result =
            match schema_validate_source(dom, &schema, &mut resolve, document_file.as_deref()) {
                Ok(result) => result,
                Err(refusal) => {
                    libxml_use_internal_errors(Some(internal_errors));
                    return Ok(Err(refusal));
                }
            };

        libxml_use_internal_errors(Some(internal_errors));
        if !result.valid {
            return Ok(Ok(result
                .errors
                .into_iter()
                .map(|e| XmlError {
                    level: if e.level == ErrorLevel::Warning {
                        "WARNING"
                    } else {
                        "ERROR"
                    },
                    code: e.code,
                    message: e.message.trim().to_owned(),
                    file: e
                        .file
                        .filter(|f| !f.is_empty() && f != "0")
                        .unwrap_or_else(|| "n/a".to_owned()),
                    line: e.line,
                    column: e.column,
                })
                .collect()));
        }

        // $dom->normalizeDocument(): an identity on what php-dom parses
        // (php-dom README).
        Ok(Ok(Vec::new()))
    }

    /// `getErrorsAsString($xmlErrors)`. Upstream compares the level
    /// (`'WARNING'` or `'ERROR'`) with the integer `LIBXML_ERR_WARNING`,
    /// which never matches: every line says `ERROR`.
    pub fn get_errors_as_string(errors: &[XmlError]) -> String {
        let mut out = String::new();
        for error in errors {
            out.push_str(&format!(
                "[ERROR {}] {} (in {} - line {}, column {})\n",
                error.code, error.message, error.file, error.line, error.column
            ));
        }
        out
    }

    /// `getFileUrl($path)` (private upstream; `XliffUtilsTest` reaches it by
    /// reflection): `file:///` and the path's `/`-separated parts through
    /// `rawurlencode()`.
    pub fn get_file_url(path: &[u8]) -> Vec<u8> {
        let mut url = b"file:///".to_vec();
        let normalized;
        let mut parts: Vec<&[u8]> = if cfg!(windows) {
            normalized = path
                .iter()
                .map(|&b| if b == b'\\' { b'/' } else { b })
                .collect::<Vec<u8>>();
            normalized.split(|&b| b == b'/').collect()
        } else {
            path.split(|&b| b == b'/').collect()
        };
        if cfg!(windows) && !parts.is_empty() {
            url.extend_from_slice(parts.remove(0));
            url.push(b'/');
        }
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                url.push(b'/');
            }
            rawurlencode(part, &mut url);
        }
        url
    }
}

/// `$attributes->getNamedItem($name)`: the first attribute, in document
/// order, whose local name is `name`, whatever its prefix (recorded).
fn named_item<'a>(attributes: &'a [php_dom::Attribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|a| a.local_name == name)
        .map(|a| a.value.as_str())
}

/// `getSchema($xliffVersion)`.
fn get_schema(version: &str) -> Result<Vec<u8>, Error> {
    let (source, xml_uri): (&[u8], &[u8]) = match version {
        "1.2" => (XLIFF_12, b"http://www.w3.org/2001/xml.xsd"),
        // XLIFF 2.1 documents are validated against the 2.0 schema, as
        // upstream does.
        "2.0" | "2.1" => (XLIFF_20, b"informativeCopiesOf3rdPartySchemas/w3c/xml.xsd"),
        "2.2" => (XLIFF_22, b"informativeCopiesOf3rdPartySchemas/w3c/xml.xsd"),
        _ => {
            return Err(Error::new(
                Class::InvalidArgument,
                format!("No support implemented for loading XLIFF version \"{version}\"."),
            ));
        }
    };
    let new_path = XliffUtils::get_file_url(XML_XSD_PATH.as_bytes());
    Ok(str_replace(source, xml_uri, &new_path))
}

fn str_replace(haystack: &[u8], search: &[u8], replace: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(search) {
            out.extend_from_slice(replace);
            i += search.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

/// `rawurlencode()`: everything but `A-Z a-z 0-9 - _ . ~` as `%XX`.
fn rawurlencode(bytes: &[u8], out: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b);
        } else {
            out.extend_from_slice(&[b'%', HEX[usize::from(b >> 4)], HEX[usize::from(b & 15)]]);
        }
    }
}

//! Port of Symfony\Component\HttpFoundation\JsonResponse.php: a response
//! whose content is a JSON document, encoded with `json_encode()` and
//! `getEncodingOptions()` (by default `JSON_HEX_TAG | JSON_HEX_APOS |
//! JSON_HEX_AMP | JSON_HEX_QUOT`, so it can be embedded in HTML), or given
//! as JSON text; with a JSONP callback, `/**/callback(json);` as
//! `text/javascript`.
//!
//! A [`JsonResponse`] is a [`Response`] (it dereferences to one), as the
//! PHP class extends it. Its headers are given as PHP's array, a list of
//! name and values pairs, as `Response`'s constructor reads them.
//! `json_encode()` and `json_decode()` are php-value's
//! (`php_value::json`). `setData()`'s rethrow of the previous exception
//! of a plain `\Exception` whose message starts with `Failed calling `
//! (a wrapper older PHP put around a `jsonSerialize()` exception) gives
//! the same exception on PHP 8.4, so what `jsonSerialize()` throws is
//! passed on as it is ([`JsonResponseError::Thrown`]).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation JsonResponse.php

use std::{
    ops::{Deref, DerefMut},
    sync::OnceLock,
};

use php_pcre::Pcre;
use php_value::{
    PhpString, Thrown, Value,
    json::{EncodeFailure, JSON_THROW_ON_ERROR, JsonError, decode_object, encode},
    numeric::float_to_string,
    object::ArrayObject,
};

use crate::{
    exception::{Class, Error},
    header_bag::{HeaderValues, ResponseHeaderBag},
    response::Response,
};

/// `JsonResponse::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\JsonResponse";

/// `json_encode()`'s default depth, which `setData()` uses.
const DEPTH: usize = 512;

/// The JSONP callback pattern, partially taken from
/// <https://geekality.net/2011/08/03/valid-javascript-identifier/> and
/// from <https://github.com/willdurand/JsonpCallbackValidator>
/// (`JsonpCallbackValidator` is released under the MIT License, (c) William
/// Durand), as PHP reads upstream's single-quoted literal.
const CALLBACK_PATTERN: &[u8] = br#"/^[$_\p{L}][$_\p{L}\p{Mn}\p{Mc}\p{Nd}\p{Pc}\x{200C}\x{200D}]*(?:\[(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\d+)\])*?$/u"#;

/// The words a callback part may not be.
const RESERVED: [&str; 45] = [
    "break",
    "do",
    "instanceof",
    "typeof",
    "case",
    "else",
    "new",
    "var",
    "catch",
    "finally",
    "return",
    "void",
    "continue",
    "for",
    "switch",
    "while",
    "debugger",
    "function",
    "this",
    "with",
    "default",
    "if",
    "throw",
    "delete",
    "in",
    "try",
    "class",
    "enum",
    "extends",
    "super",
    "const",
    "export",
    "import",
    "implements",
    "let",
    "private",
    "public",
    "yield",
    "interface",
    "package",
    "protected",
    "static",
    "null",
    "true",
    "false",
];

/// What a `JsonResponse` call threw.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonResponseError {
    /// An exception of the classes this crate throws: `\TypeError`,
    /// `\InvalidArgumentException`, `\JsonException`, or `Response`'s.
    Exception(Error),
    /// What a `jsonSerialize()` threw, passed on.
    Thrown(Thrown),
    /// `json_encode()`'s `\JsonException` (`JSON_THROW_ON_ERROR`) whose
    /// previous exception is what a `jsonSerialize()` threw.
    JsonWithPrevious { exception: Error, previous: Thrown },
}

impl From<Error> for JsonResponseError {
    fn from(error: Error) -> Self {
        Self::Exception(error)
    }
}

impl std::fmt::Display for JsonResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exception(e) | Self::JsonWithPrevious { exception: e, .. } => {
                write!(f, "{e}")
            }
            Self::Thrown(t) => write!(f, "{t}"),
        }
    }
}

impl std::error::Error for JsonResponseError {}

pub type Result<T> = std::result::Result<T, JsonResponseError>;

/// `Symfony\Component\HttpFoundation\JsonResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonResponse {
    response: Response,
    /// `$this->data`: the JSON text.
    data: PhpString,
    callback: Option<PhpString>,
    encoding_options: i64,
}

impl Deref for JsonResponse {
    type Target = Response;

    fn deref(&self) -> &Response {
        &self.response
    }
}

impl DerefMut for JsonResponse {
    fn deref_mut(&mut self) -> &mut Response {
        &mut self.response
    }
}

impl From<JsonResponse> for Response {
    fn from(response: JsonResponse) -> Self {
        response.response
    }
}

pub(crate) fn json_exception(error: JsonError) -> Error {
    Error::new(Class::PhpJson, error.message()).with_code(error.code())
}

fn callback_pattern() -> std::result::Result<&'static Pcre, Error> {
    static PATTERN: OnceLock<std::result::Result<Pcre, String>> = OnceLock::new();
    PATTERN
        .get_or_init(|| Pcre::new(CALLBACK_PATTERN).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| Error::new(Class::NotPorted, e.clone()))
}

impl JsonResponse {
    /// `getContent()`: always a string, as the class is not a
    /// `StreamedResponse`.
    pub fn get_content(&self) -> &PhpString {
        self.response.content()
    }

    /// `JsonResponse::DEFAULT_ENCODING_OPTIONS`: `JSON_HEX_TAG |
    /// JSON_HEX_APOS | JSON_HEX_AMP | JSON_HEX_QUOT`.
    pub const DEFAULT_ENCODING_OPTIONS: i64 = 15;

    /// `new JsonResponse($data, $status, $headers, $json)`: `data` is
    /// encoded, or with `json` taken as JSON text (a string, a number or
    /// a `\Stringable`); null is an empty `ArrayObject` (`{}`).
    ///
    /// Fails with `Response`'s `\InvalidArgumentException` for a status
    /// code that is not valid, a `\TypeError` for JSON text of another
    /// type, and as [`set_data`](Self::set_data) does.
    pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        data: Value,
        status: i64,
        headers: impl IntoIterator<Item = (K, V)>,
        json: bool,
    ) -> Result<Self> {
        let headers = ResponseHeaderBag::new(headers)?;
        let mut response = Self {
            response: Response::new(Some(""), status, headers)?,
            data: PhpString::default(),
            callback: None,
            encoding_options: Self::DEFAULT_ENCODING_OPTIONS,
        };

        if json {
            let text = match &data {
                Value::String(s) => s.clone(),
                Value::Int(i) => PhpString::from(i.to_string()),
                Value::Float(f) => PhpString::from(float_to_string(*f)),
                Value::Object(o) if o.is_stringable() => o.to_php_string().unwrap_or_default(),
                other => {
                    return Err(Error::new(
                        Class::PhpTypeError,
                        format!(
                            "\"{CLASS}::__construct\": If $json is set to true, argument $data must be a string or object implementing __toString(), \"{}\" given.",
                            other.debug_type()
                        ),
                    )
                    .into());
                }
            };
            response.set_json(text);
            return Ok(response);
        }

        let data = match data {
            Value::Null => Value::Object(std::sync::Arc::new(ArrayObject::default())),
            data => data,
        };
        response.set_data(&data)?;
        Ok(response)
    }

    /// `JsonResponse::fromJsonString($data, $status, $headers)`.
    pub fn from_json_string<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        data: impl Into<PhpString>,
        status: i64,
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self> {
        Self::new(Value::String(data.into()), status, headers, true)
    }

    /// `setCallback($callback)`: the JSONP callback, or none. Each part
    /// between dots must be an identifier, optionally followed by `[...]`
    /// accessors, and not a reserved word.
    ///
    /// Fails with `\InvalidArgumentException` when the name is not valid.
    pub fn set_callback(&mut self, callback: Option<&[u8]>) -> Result<&mut Self> {
        if let Some(callback) = callback {
            let pattern = callback_pattern()?;
            for part in callback.split(|&b| b == b'.') {
                // a subject that is not UTF-8 makes preg_match() false
                let matches = pattern.is_match(part).unwrap_or(false);
                if !matches || RESERVED.iter().any(|word| word.as_bytes() == part) {
                    return Err(Error::new(
                        Class::PhpInvalidArgument,
                        "The callback name is not valid.",
                    )
                    .into());
                }
            }
        }

        self.callback = callback.map(PhpString::from);

        Ok(self.update())
    }

    /// `setJson($json)`: JSON text to send as it is.
    pub fn set_json(&mut self, json: impl Into<PhpString>) -> &mut Self {
        self.data = json.into();

        self.update()
    }

    /// `setData($data)`: `json_encode($data, $this->encodingOptions)`.
    ///
    /// Fails with what `jsonSerialize()` threw, with `json_encode()`'s
    /// `\JsonException` under `JSON_THROW_ON_ERROR`, and otherwise with an
    /// `\InvalidArgumentException` whose message is `json_last_error_msg()`.
    pub fn set_data(&mut self, data: &Value) -> Result<&mut Self> {
        let encoded = match encode(data, self.encoding_options, DEPTH) {
            Ok(encoded) => encoded,
            Err(EncodeFailure::Thrown(thrown)) => return Err(JsonResponseError::Thrown(thrown)),
            Err(EncodeFailure::Json {
                error,
                previous: None,
            }) => return Err(json_exception(error).into()),
            Err(EncodeFailure::Json {
                error,
                previous: Some(previous),
            }) => {
                return Err(JsonResponseError::JsonWithPrevious {
                    exception: json_exception(error),
                    previous,
                });
            }
        };
        // false as a string parameter is ''
        let json = PhpString::from(encoded.json.unwrap_or_default());

        if JSON_THROW_ON_ERROR & self.encoding_options != 0 {
            return Ok(self.set_json(json));
        }

        if let Some(error) = encoded.error {
            return Err(Error::new(Class::PhpInvalidArgument, error.message()).into());
        }

        Ok(self.set_json(json))
    }

    /// `getEncodingOptions()`.
    pub fn get_encoding_options(&self) -> i64 {
        self.encoding_options
    }

    /// `setEncodingOptions($encodingOptions)`: the data is decoded
    /// (`json_decode()`, objects as `stdClass`; null when it is not
    /// valid JSON) and encoded again.
    ///
    /// Fails as [`set_data`](Self::set_data) does.
    pub fn set_encoding_options(&mut self, encoding_options: i64) -> Result<&mut Self> {
        self.encoding_options = encoding_options;

        let data = decode_object(self.data.as_bytes(), DEPTH, false).unwrap_or(Value::Null);
        self.set_data(&data)
    }

    /// The protected `update()`: the content and Content-Type for the data
    /// and callback.
    fn update(&mut self) -> &mut Self {
        // Content-Type is not Set-Cookie, so setting it cannot fail.
        if let Some(callback) = &self.callback {
            // Not using application/javascript for compatibility reasons with older browsers.
            let _ = self
                .response
                .headers_mut()
                .set(b"Content-Type", "text/javascript", true);

            let content = [
                b"/**/",
                callback.as_bytes(),
                b"(",
                self.data.as_bytes(),
                b");",
            ]
            .concat();
            // A plain response takes any content.
            let _ = self.response.set_content(Some(PhpString::from(content)));
            return self;
        }

        // Only set the header when there is none or when it equals 'text/javascript' (from a previous update with callback)
        // in order to not overwrite a custom definition.
        let headers = self.response.headers();
        if !headers.has("Content-Type")
            || headers
                .get("Content-Type", None)
                .is_some_and(|v| v.as_bytes() == b"text/javascript")
        {
            let _ = self
                .response
                .headers_mut()
                .set(b"Content-Type", "application/json", true);
        }

        // A plain response takes any content.
        let _ = self.response.set_content(Some(self.data.clone()));
        self
    }
}

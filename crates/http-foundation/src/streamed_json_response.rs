//! Port of Symfony\Component\HttpFoundation\StreamedJsonResponse.php: a
//! `StreamedResponse` that writes a JSON document whose generators (and
//! other objects) are streamed as they are read, by default as
//! `application/json`.
//!
//! The data is a PHP value: an array, or a `\Traversable` object.
//! Objects are php-value's [`PhpObject`]s; a [`Generator`] is read lazily,
//! so its pairs are written as it yields them, and what it echoes lands
//! between them. `\Traversable` and `\JsonSerializable` are what
//! [`PhpObject::instance_of`] answers; another `\Traversable` is read
//! with [`PhpObject::traverse`]. `json_encode()` is php-value's
//! (`php_value::json`), at its default depth.
//!
//! The output is PHP's, quirks included: the structure around the objects
//! is encoded with each object replaced by the placeholder
//! `"__symfony_json__"` and split there, and the objects are streamed
//! between the parts. A placeholder already in the structure, as a value
//! or as the key of a value that is not an array, is written back as
//! itself. As the key of an array it is not counted, so the parts no
//! longer line up, as in PHP. An iterable is a JSON list when its first
//! key is `0`, otherwise an object, whatever its later keys.
//!
//! Not reproduced: a key that is an array is written as `"Array"`
//! without PHP's warning; the `\TypeError` for data that is not iterable
//! has no "called in" suffix.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation StreamedJsonResponse.php

use std::sync::Arc;

use php_value::{
    PhpArray, PhpObject, PhpString, Value,
    json::{EncodeFailure, JSON_NUMERIC_CHECK, JSON_THROW_ON_ERROR, encode},
};

use crate::{
    exception::{Class, Error, Result},
    generator::Generator,
    header_bag::{HeaderValues, ResponseHeaderBag},
    json_response::{JsonResponseError, json_exception},
    response::Response,
    streamed_response::{CallbackOrChunks, Subclass, Throwable, echo, new_subclass},
};

/// `StreamedJsonResponse::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\StreamedJsonResponse";

/// `self::PLACEHOLDER`.
const PLACEHOLDER: &str = "__symfony_json__";

/// `json_encode()`'s default depth.
const DEPTH: usize = 512;

/// `new StreamedJsonResponse($data, $status, $headers,
/// $encodingOptions)`. `headers` is PHP's array; `Content-Type` is
/// `application/json` unless the headers give a truthy one.
///
/// Fails with `\TypeError` when the data is neither an array nor
/// `\Traversable`, and with `\InvalidArgumentException` when the status
/// code is not valid.
pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
    data: Value,
    status: i64,
    headers: impl IntoIterator<Item = (K, V)>,
    encoding_options: i64,
) -> Result<Response> {
    if !is_iterable(&data) {
        return Err(Error::new(
            Class::PhpTypeError,
            format!(
                "{CLASS}::__construct(): Argument #1 ($data) must be of type Traversable|array, {} given",
                data.debug_type()
            ),
        ));
    }
    let data = Arc::new(data);
    let callback = Arc::new(move || stream(&data, encoding_options));
    let mut response = new_subclass(
        CallbackOrChunks::Callback(callback),
        status,
        ResponseHeaderBag::new(headers)?,
        Subclass::Json,
    )?;
    let content_type = response.headers().get("Content-Type", None);
    if !content_type.is_some_and(|t| Value::String(t).is_truthy()) {
        response
            .headers_mut()
            .set(b"Content-Type", "application/json", true)?;
    }
    Ok(response)
}

/// `is_iterable($data)`.
fn is_iterable(data: &Value) -> bool {
    match data {
        Value::Array(_) => true,
        Value::Object(object) => object.instance_of("Traversable"),
        _ => false,
    }
}

/// `stream()`, the callback.
fn stream(data: &Value, encoding_options: i64) -> std::result::Result<(), Throwable> {
    let json_options = JSON_THROW_ON_ERROR | encoding_options;
    let key_options = json_options & !JSON_NUMERIC_CHECK;
    stream_data(data, json_options, key_options)
}

fn stream_data(
    data: &Value,
    json_options: i64,
    key_options: i64,
) -> std::result::Result<(), Throwable> {
    match data {
        Value::Array(array) => stream_array(array, json_options, key_options),
        Value::Object(object)
            if object.instance_of("Traversable") && !object.instance_of("JsonSerializable") =>
        {
            stream_iterable(object.as_ref(), json_options, key_options)
        }
        other => {
            echo(json_encode(other, json_options)?);
            Ok(())
        }
    }
}

/// `array_walk_recursive()`'s pass: each object leaf replaced by the
/// placeholder, and what will be streamed in the placeholders' places
/// collected in order.
fn replace_objects(array: &PhpArray, streamed: &mut Vec<Value>) -> PhpArray {
    let mut out = PhpArray::new();
    for (key, item) in array.iter() {
        let item = match item {
            Value::Array(inner) => Value::Array(replace_objects(inner, streamed)),
            leaf => {
                if matches!(key, php_value::Key::Str(k) if k.as_bytes() == PLACEHOLDER.as_bytes()) {
                    streamed.push(Value::string(PLACEHOLDER));
                }
                match leaf {
                    Value::Object(_) => {
                        streamed.push(leaf.clone());
                        Value::string(PLACEHOLDER)
                    }
                    Value::String(s) if s.as_bytes() == PLACEHOLDER.as_bytes() => {
                        streamed.push(leaf.clone());
                        leaf.clone()
                    }
                    _ => leaf.clone(),
                }
            }
        };
        out.insert(key.clone(), item);
    }
    out
}

fn stream_array(
    array: &PhpArray,
    json_options: i64,
    key_options: i64,
) -> std::result::Result<(), Throwable> {
    let mut streamed = Vec::new();
    let structure = replace_objects(array, &mut streamed);
    let json = json_encode(&Value::Array(structure), json_options)?;
    let parts = explode(json.as_bytes(), format!("\"{PLACEHOLDER}\"").as_bytes());

    for (index, item) in streamed.iter().enumerate() {
        // A part PHP lacks is null, which echoes nothing.
        echo(parts.get(index).copied().unwrap_or_default());
        stream_data(item, json_options, key_options)?;
    }
    echo(parts.last().copied().unwrap_or_default());
    Ok(())
}

fn stream_iterable(
    iterable: &dyn PhpObject,
    json_options: i64,
    key_options: i64,
) -> std::result::Result<(), Throwable> {
    let mut is_first_item = true;
    let mut start_tag = b'[';
    let mut item = |key: Value, value: Value| -> std::result::Result<(), Throwable> {
        if is_first_item {
            is_first_item = false;
            // A list or a map, by the first key alone.
            if !matches!(key, Value::Int(0)) {
                start_tag = b'{';
            }
            echo(PhpString::from(vec![start_tag]));
        } else {
            echo(",");
        }
        if start_tag == b'{' {
            let key = Value::String(key_string(&key)?);
            let mut written = json_encode(&key, key_options)?.into_bytes();
            written.push(b':');
            echo(written);
        }
        stream_data(&value, json_options, key_options)
    };

    if let Some(generator) = iterable.downcast_ref::<Generator>() {
        generator.for_each(&mut item, |thrown| -> Throwable { Arc::new(thrown) })?;
    } else {
        let pairs = iterable
            .traverse()
            .unwrap_or_else(|| Ok(Vec::new()))
            .map_err(|thrown| -> Throwable { Arc::new(thrown) })?;
        for (key, value) in pairs {
            item(key, value)?;
        }
    }

    if is_first_item {
        // The iterable was empty.
        echo("[");
    }
    echo(if start_tag == b'[' { "]" } else { "}" });
    Ok(())
}

/// `(string) $key`.
fn key_string(key: &Value) -> std::result::Result<PhpString, Throwable> {
    if let Some(string) = key.to_php_string_or_stringable() {
        return Ok(string);
    }
    match key {
        Value::Array(_) => Ok(PhpString::from("Array")),
        Value::Object(object) => Err(Arc::new(Error::new(
            Class::PhpError,
            format!(
                "Object of class {} could not be converted to string",
                object.class()
            ),
        ))),
        _ => Ok(PhpString::default()),
    }
}

/// `json_encode($value, $options)` with `JSON_THROW_ON_ERROR` among the
/// options: `false` (only with `JSON_PARTIAL_OUTPUT_ON_ERROR`, which then
/// takes precedence) echoes nothing.
fn json_encode(value: &Value, options: i64) -> std::result::Result<String, Throwable> {
    match encode(value, options, DEPTH) {
        Ok(encoded) => Ok(encoded.json.unwrap_or_default()),
        Err(EncodeFailure::Thrown(thrown)) => Err(Arc::new(thrown)),
        Err(EncodeFailure::Json {
            error,
            previous: None,
        }) => Err(Arc::new(json_exception(error))),
        Err(EncodeFailure::Json {
            error,
            previous: Some(previous),
        }) => Err(Arc::new(JsonResponseError::JsonWithPrevious {
            exception: json_exception(error),
            previous,
        })),
    }
}

/// `explode($separator, $string)`.
fn explode<'a>(string: &'a [u8], separator: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = Vec::new();
    let mut rest = string;
    while let Some(at) = rest
        .windows(separator.len())
        .position(|window| window == separator)
    {
        parts.push(&rest[..at]);
        rest = &rest[at + separator.len()..];
    }
    parts.push(rest);
    parts
}

#[cfg(test)]
mod tests {
    use super::explode;

    #[test]
    fn explode_splits_as_php_does() {
        assert_eq!(explode(b"a,b,,c", b","), [&b"a"[..], b"b", b"", b"c"]);
        assert_eq!(explode(b"", b","), [&b""[..]]);
        assert_eq!(explode(b"\"x\"\"x\"", b"\"x\""), [&b""[..], b"", b""]);
    }
}

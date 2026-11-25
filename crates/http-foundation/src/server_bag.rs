//! Port of Symfony\Component\HttpFoundation\ServerBag.php: the request
//! headers held in `$_SERVER`, with the `Authorization` header rebuilt
//! from or split into `PHP_AUTH_*`.
//!
//! The values are PHP values, as `$_SERVER` may hold anything. Where PHP
//! converts a value to a string, an array is `"Array"` (PHP warns), and an
//! array or object handed to `stripos()` is PHP's `TypeError`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation ServerBag.php

use std::ops::{Deref, DerefMut};

use php_value::{Key, PhpArray, PhpString, Value, base64};

use crate::{
    exception::{Class, Error, Result},
    parameter_bag::ParameterBag,
};

/// `ServerBag`: a `ParameterBag` over `$_SERVER`.
#[derive(Debug, Clone)]
pub struct ServerBag(ParameterBag);

impl Deref for ServerBag {
    type Target = ParameterBag;

    fn deref(&self) -> &ParameterBag {
        &self.0
    }
}

impl DerefMut for ServerBag {
    fn deref_mut(&mut self) -> &mut ParameterBag {
        &mut self.0
    }
}

/// `isset($array[$key])`.
fn isset(array: &PhpArray, key: &str) -> bool {
    array
        .get(&Key::from(key))
        .is_some_and(|value| !value.is_null())
}

/// `empty($value)`.
fn is_empty(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null | Value::Bool(false)) => true,
        Some(Value::Int(i)) => *i == 0,
        Some(Value::Float(f)) => *f == 0.0,
        Some(Value::String(s)) => s.is_empty() || s.as_bytes() == b"0",
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(_) | Value::Bool(true)) => false,
    }
}

/// `(string) $value` in a concatenation.
fn concat_string(value: &Value) -> Result<PhpString> {
    match value {
        Value::Array(_) => Ok(PhpString::from("Array")),
        Value::Object(object) => object.to_php_string().ok_or_else(|| {
            Error::new(
                Class::PhpError,
                format!(
                    "Object of class {} could not be converted to string",
                    object.class()
                ),
            )
        }),
        other => Ok(other.to_php_string().unwrap_or_default()),
    }
}

/// `0 === stripos($haystack, $prefix)` for an ASCII prefix.
fn starts_with_ignore_case(haystack: &Value, prefix: &[u8]) -> Result<bool> {
    let text = match haystack {
        Value::Array(_) | Value::Object(_) if haystack.to_php_string_or_stringable().is_none() => {
            return Err(Error::new(
                Class::PhpTypeError,
                format!(
                    "stripos(): Argument #1 ($haystack) must be of type string, {} given",
                    haystack.debug_type()
                ),
            ));
        }
        other => other.to_php_string_or_stringable().unwrap_or_default(),
    };
    Ok(text
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix)))
}

impl ServerBag {
    pub fn new(parameters: PhpArray) -> Self {
        Self(ParameterBag::new(parameters))
    }

    /// `getHeaders()`: the HTTP headers, named as in `$_SERVER` without
    /// `HTTP_`. A Digest `Authorization` header is also stored as
    /// `PHP_AUTH_DIGEST` in the bag, as PHP does.
    pub fn get_headers(&mut self) -> Result<PhpArray> {
        let parameters = self.0.all().clone();
        let mut headers = PhpArray::new();
        for (key, value) in parameters.iter() {
            let name = key.to_value().to_php_string().unwrap_or_default();
            if let Some(rest) = name.as_bytes().strip_prefix(b"HTTP_") {
                headers.insert(Key::from_bytes(rest), value.clone());
            } else if matches!(
                key.as_str().map(PhpString::as_bytes),
                Some(b"CONTENT_TYPE" | b"CONTENT_LENGTH" | b"CONTENT_MD5")
            ) && !matches!(value, Value::String(s) if s.is_empty())
            {
                headers.insert(key.clone(), value.clone());
            }
        }

        if isset(&parameters, "PHP_AUTH_USER") {
            let user = parameters
                .get(&Key::from("PHP_AUTH_USER"))
                .cloned()
                .unwrap_or_default();
            let password = parameters
                .get(&Key::from("PHP_AUTH_PW"))
                .filter(|value| !value.is_null())
                .cloned()
                .unwrap_or_else(|| Value::from(""));
            headers.insert(Key::from("PHP_AUTH_USER"), user);
            headers.insert(Key::from("PHP_AUTH_PW"), password);
        } else {
            let authorization = ["HTTP_AUTHORIZATION", "REDIRECT_HTTP_AUTHORIZATION"]
                .into_iter()
                .find(|key| isset(&parameters, key))
                .and_then(|key| parameters.get(&Key::from(key)).cloned());
            if let Some(header) = authorization {
                if starts_with_ignore_case(&header, b"basic ")? {
                    // Decode AUTHORIZATION header into PHP_AUTH_USER and
                    // PHP_AUTH_PW when authorization header is basic
                    let text = header.to_php_string_or_stringable().unwrap_or_default();
                    let decoded = base64::decode(text.as_bytes().get(6..).unwrap_or_default());
                    if let Some(colon) = decoded.iter().position(|&b| b == b':') {
                        headers.insert(
                            Key::from("PHP_AUTH_USER"),
                            Value::from(PhpString::from(&decoded[..colon])),
                        );
                        headers.insert(
                            Key::from("PHP_AUTH_PW"),
                            Value::from(PhpString::from(&decoded[colon + 1..])),
                        );
                    }
                } else if is_empty(parameters.get(&Key::from("PHP_AUTH_DIGEST")))
                    && starts_with_ignore_case(&header, b"digest ")?
                {
                    // In some circumstances PHP_AUTH_DIGEST needs to be set
                    headers.insert(Key::from("PHP_AUTH_DIGEST"), header.clone());
                    self.0.set("PHP_AUTH_DIGEST", header);
                } else if starts_with_ignore_case(&header, b"bearer ")? {
                    // There is no PHP_AUTH_BEARER in PHP predefined
                    // variables, so it is kept as AUTHORIZATION.
                    headers.insert(Key::from("AUTHORIZATION"), header);
                }
            }
        }

        if isset(&headers, "AUTHORIZATION") {
            return Ok(headers);
        }

        // PHP_AUTH_USER/PHP_AUTH_PW
        if isset(&headers, "PHP_AUTH_USER") {
            let mut credentials = concat_string(
                headers
                    .get(&Key::from("PHP_AUTH_USER"))
                    .unwrap_or(&Value::Null),
            )?
            .as_bytes()
            .to_vec();
            credentials.push(b':');
            let password = headers
                .get(&Key::from("PHP_AUTH_PW"))
                .filter(|value| !value.is_null())
                .cloned()
                .unwrap_or_else(|| Value::from(""));
            credentials.extend_from_slice(concat_string(&password)?.as_bytes());
            let authorization = format!("Basic {}", base64::encode(&credentials));
            headers.insert(
                Key::from("AUTHORIZATION"),
                Value::from(authorization.as_str()),
            );
        } else if isset(&headers, "PHP_AUTH_DIGEST") {
            let digest = headers
                .get(&Key::from("PHP_AUTH_DIGEST"))
                .cloned()
                .unwrap_or_default();
            headers.insert(Key::from("AUTHORIZATION"), digest);
        }

        Ok(headers)
    }
}

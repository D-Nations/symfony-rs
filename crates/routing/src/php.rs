//! The PHP engine behaviour Route, `CompiledRoute` and `RouteCompiler` rely
//! on: truthiness, `trim()`, case mapping, and the coercion of a value
//! assigned to a typed property in `__unserialize()`. Written from PHP's
//! documentation and answers recorded in the oracle; no PHP source.
//!
//! Upstream: none

use php_value::{Key, PhpArray, PhpString, Value};

use crate::exception::{Class, Error};

/// `(bool) $value`.
pub(crate) fn truthy(value: &Value) -> bool {
    value.is_truthy()
}

/// `isset($array[$key])`.
pub(crate) fn isset(array: &PhpArray, key: &[u8]) -> bool {
    array
        .get(&Key::from_bytes(key))
        .is_some_and(|v| !v.is_null())
}

/// `$array[$key] ?? null`.
pub(crate) fn get<'a>(array: &'a PhpArray, key: &[u8]) -> Option<&'a Value> {
    array.get(&Key::from_bytes(key)).filter(|v| !v.is_null())
}

/// A key as the string PHP gives it in a string context.
pub(crate) fn key_bytes(key: &Key) -> Vec<u8> {
    key.bytes().into_owned()
}

/// `trim($s)` with the default characters.
pub(crate) use php_value::string::trim;

/// `trim($s, $c)` / `ltrim($s, $c)` for one character.
pub(crate) fn trim_char(s: &[u8], c: u8, right: bool) -> &[u8] {
    let start = s.iter().position(|&x| x != c).unwrap_or(s.len());
    let end = if right {
        s.iter().rposition(|&x| x != c).map_or(start, |e| e + 1)
    } else {
        s.len()
    };
    &s[start..end]
}

/// `(string) $value` as a `string` parameter of a function in a file
/// without `strict_types` receives it: scalars are converted, `\Stringable`
/// objects too; null, arrays and other objects are a `\TypeError`.
pub(crate) fn string_argument(
    value: &Value,
    function: &str,
    position: usize,
    name: &str,
) -> Result<PhpString, Error> {
    match value {
        Value::Null | Value::Array(_) => Err(argument_type_error(value, function, position, name)),
        other => other
            .to_php_string_or_stringable()
            .ok_or_else(|| argument_type_error(value, function, position, name)),
    }
}

/// PHP's message for a parameter type mismatch, without the
/// ", called in %s on line %d" that names the calling PHP file.
fn argument_type_error(value: &Value, function: &str, position: usize, name: &str) -> Error {
    Error::new(
        Class::PhpTypeError,
        format!(
            "{function}(): Argument #{position} (${name}) must be of type string, {} given",
            value.debug_type()
        ),
    )
}

fn property_type_error(value: &Value, class: &str, property: &str, ty: &str) -> Error {
    // a bool is named by its value here (recorded from PHP 8.4.27)
    let given = match value {
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        other => other.debug_type(),
    };
    Error::new(
        Class::PhpTypeError,
        format!("Cannot assign {given} to property {class}::${property} of type {ty}"),
    )
}

/// `$this->$property = $value` for a `string` (or `?string`) property, in
/// coercive typing mode: scalars become strings, a missing value is null.
pub(crate) fn assign_string(
    data: &PhpArray,
    key: &str,
    class: &str,
    property: &str,
    nullable: bool,
) -> Result<Option<PhpString>, Error> {
    let value = data.get(&Key::from(key)).cloned().unwrap_or_default();
    match &value {
        Value::Null if nullable => Ok(None),
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => {
            Ok(value.to_php_string())
        }
        _ => Err(property_type_error(
            &value,
            class,
            property,
            if nullable { "?string" } else { "string" },
        )),
    }
}

/// `$this->$property = $value` for an `array` property.
pub(crate) fn assign_array(
    data: &PhpArray,
    key: &str,
    class: &str,
    property: &str,
) -> Result<PhpArray, Error> {
    match data.get(&Key::from(key)).cloned().unwrap_or_default() {
        Value::Array(a) => Ok(a),
        other => Err(property_type_error(&other, class, property, "array")),
    }
}

/// `$this->$property = $value` for a property of an object type.
pub(crate) fn object_type_error(value: &Value, class: &str, property: &str, ty: &str) -> Error {
    property_type_error(value, class, property, ty)
}

/// `($data[$key] ?? null) instanceof \Stringable`.
pub(crate) fn is_stringable(data: &PhpArray, key: &str) -> bool {
    matches!(data.get(&Key::from(key)), Some(Value::Object(o)) if o.is_stringable())
}

/// `serialize()` throws for a `Closure` anywhere in the value.
pub(crate) fn check_serializable(value: &Value) -> Result<(), Error> {
    match value {
        Value::Object(o) if o.class() == "Closure" => Err(Error::new(
            Class::PhpException,
            "Serialization of 'Closure' is not allowed",
        )),
        Value::Array(a) => a.values().try_for_each(check_serializable),
        _ => Ok(()),
    }
}

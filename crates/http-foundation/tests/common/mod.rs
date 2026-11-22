//! What ParameterBagTest and InputBagTest share: the FooEnum fixture
//! (Tests/Fixtures/FooEnum.php), ParameterBagTest's two data providers,
//! and `strtoupper(...)`.
//!
//! Upstream: none
#![allow(dead_code)]

use php_value::{
    PhpArray, PhpString, Value,
    constants::Registry,
    filter::{
        FILTER_CALLBACK, FILTER_FORCE_ARRAY, FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_ARRAY,
        FILTER_VALIDATE_BOOLEAN, FILTER_VALIDATE_INT,
    },
    object::{Closure, EnumCase, StdClass},
    php_array,
};

pub const FOO_ENUM: &str = "Symfony\\Component\\HttpFoundation\\Tests\\Fixtures\\FooEnum";

/// `enum FooEnum: int { case Bar = 1; }`.
pub fn foo_enum_registry() -> Registry {
    let mut registry = Registry::empty();
    registry.define_enum(
        FOO_ENUM,
        vec![EnumCase::new(FOO_ENUM, "Bar", Some(Value::Int(1)))],
    );
    registry
}

/// `FooEnum::Bar`.
pub fn foo_bar() -> Value {
    Value::Object(EnumCase::new(FOO_ENUM, "Bar", Some(Value::Int(1))).object())
}

/// `strtoupper(...)`: ASCII letters only, as PHP 8's.
pub fn strtoupper(args: &[Value]) -> Value {
    let text = args
        .first()
        .and_then(Value::to_php_string)
        .unwrap_or_default();
    Value::String(PhpString::from(text.as_bytes().to_ascii_uppercase()))
}

/// `(int) $value`.
fn to_int(value: &Value) -> Value {
    Value::Int(php_value::numeric::string_to_int(
        value.to_php_string().unwrap_or_default().as_bytes(),
    ))
}

fn options(flags: i64, callback: Value) -> Value {
    let mut options = PhpArray::new();
    options.insert("flags", Value::Int(flags));
    options.insert("options", callback);
    Value::Array(options)
}

fn object() -> Value {
    StdClass::object(PhpArray::new())
}

fn nulls(keys: &[&str], valid: Value) -> PhpArray {
    let mut array = PhpArray::new();
    array.insert(keys[0], valid);
    array.insert(keys[1], Value::Null);
    array
}

pub type Case = (&'static str, Value, i64, Value, PhpArray);

/// `ParameterBagTest::provideInvalidFilterArrayValues()`.
pub fn provide_invalid_filter_array_values() -> Vec<Case> {
    let flat = || Value::Array(php_array!["valid" => "12", "invalid" => "bad"]);
    let nested =
        || Value::Array(php_array!["values" => php_array!["valid" => "12", "invalid" => "bad"]]);
    let flat_expected = || nulls(&["valid", "invalid"], Value::Int(12));
    let nested_expected = || php_array!["values" => flat_expected()];
    let callback = || {
        Closure::object(|args| {
            let value = args.first().cloned().unwrap_or_default();
            if value == Value::from("bad") {
                Value::Null
            } else {
                to_int(&value)
            }
        })
    };
    let throwing = || {
        Closure::object(|_| {
            panic!("Non-stringable objects must not reach the callback.");
        })
    };
    vec![
        (
            "flat integers",
            flat(),
            FILTER_VALIDATE_INT,
            Value::Int(FILTER_REQUIRE_ARRAY),
            flat_expected(),
        ),
        (
            "nested integers",
            nested(),
            FILTER_VALIDATE_INT,
            Value::Int(FILTER_REQUIRE_ARRAY),
            nested_expected(),
        ),
        (
            "forced scalar integer",
            Value::from("bad"),
            FILTER_VALIDATE_INT,
            Value::Int(FILTER_FORCE_ARRAY),
            PhpArray::list([Value::Null]),
        ),
        (
            "flat callback",
            flat(),
            FILTER_CALLBACK,
            options(FILTER_REQUIRE_ARRAY, callback()),
            flat_expected(),
        ),
        (
            "nested callback",
            nested(),
            FILTER_CALLBACK,
            options(FILTER_REQUIRE_ARRAY, callback()),
            nested_expected(),
        ),
        (
            "forced scalar callback",
            Value::from("bad"),
            FILTER_CALLBACK,
            options(FILTER_FORCE_ARRAY, callback()),
            PhpArray::list([Value::Null]),
        ),
        (
            "nested object",
            Value::Array(php_array!["values" => PhpArray::list([object()])]),
            FILTER_VALIDATE_INT,
            Value::Int(FILTER_REQUIRE_ARRAY),
            php_array!["values" => PhpArray::list([Value::Null])],
        ),
        (
            "nested object with callback",
            Value::Array(php_array!["values" => PhpArray::list([object()])]),
            FILTER_CALLBACK,
            options(FILTER_REQUIRE_ARRAY, throwing()),
            php_array!["values" => PhpArray::list([Value::Null])],
        ),
    ]
}

/// `ParameterBagTest::provideValidFilterArrayValues()`.
pub fn provide_valid_filter_array_values() -> Vec<Case> {
    let optional = || {
        let mut array = PhpArray::new();
        array.insert("optional", Value::Null);
        array
    };
    let returning_array = move || Closure::object(move |_| Value::Array(optional()));
    let default = |flags: i64| {
        Value::Array(php_array!["flags" => flags, "options" => php_array!["default" => optional()]])
    };
    let list = |v: Value| Value::Array(PhpArray::list([v]));
    vec![
        (
            "false boolean",
            Value::Array(php_array!["false", "true"]),
            FILTER_VALIDATE_BOOLEAN,
            Value::Int(FILTER_REQUIRE_ARRAY),
            php_array![false, true],
        ),
        (
            "nested false boolean",
            list(list(Value::from("false"))),
            FILTER_VALIDATE_BOOLEAN,
            Value::Int(FILTER_REQUIRE_ARRAY),
            PhpArray::list([list(Value::Bool(false))]),
        ),
        (
            "forced scalar false boolean",
            Value::from("false"),
            FILTER_VALIDATE_BOOLEAN,
            Value::Int(FILTER_FORCE_ARRAY),
            php_array![false],
        ),
        (
            "empty array",
            Value::Array(PhpArray::new()),
            FILTER_VALIDATE_INT,
            Value::Int(FILTER_REQUIRE_ARRAY),
            PhpArray::new(),
        ),
        (
            "false callback",
            list(Value::from("value")),
            FILTER_CALLBACK,
            options(
                FILTER_REQUIRE_ARRAY,
                Closure::object(|_| Value::Bool(false)),
            ),
            php_array![false],
        ),
        (
            "callback returning array",
            list(Value::from("value")),
            FILTER_CALLBACK,
            options(FILTER_REQUIRE_ARRAY, returning_array()),
            PhpArray::list([Value::Array(optional())]),
        ),
        (
            "nested callback returning array",
            list(list(Value::from("value"))),
            FILTER_CALLBACK,
            options(FILTER_REQUIRE_ARRAY, returning_array()),
            PhpArray::list([list(Value::Array(optional()))]),
        ),
        (
            "forced scalar callback returning array",
            Value::from("value"),
            FILTER_CALLBACK,
            options(FILTER_FORCE_ARRAY, returning_array()),
            PhpArray::list([Value::Array(optional())]),
        ),
        (
            "array default",
            list(Value::from("bad")),
            FILTER_VALIDATE_INT,
            default(FILTER_REQUIRE_ARRAY),
            PhpArray::list([Value::Array(optional())]),
        ),
        (
            "nested array default",
            list(list(Value::from("bad"))),
            FILTER_VALIDATE_INT,
            default(FILTER_REQUIRE_ARRAY),
            PhpArray::list([list(Value::Array(optional()))]),
        ),
        (
            "forced scalar array default",
            Value::from("bad"),
            FILTER_VALIDATE_INT,
            default(FILTER_FORCE_ARRAY),
            PhpArray::list([Value::Array(optional())]),
        ),
    ]
}

/// testFilterArrayWithNullOnFailure's `$options['flags'] |=
/// \FILTER_NULL_ON_FAILURE`, after making `$options` an array.
pub fn with_null_on_failure(options: Value) -> Value {
    let mut array = match options {
        Value::Array(array) => array,
        flags => {
            let mut array = PhpArray::new();
            array.insert("flags", flags);
            array
        }
    };
    let flags = match array.get(&php_value::Key::from("flags")) {
        Some(Value::Int(flags)) => *flags,
        _ => 0,
    };
    array.insert("flags", Value::Int(flags | FILTER_NULL_ON_FAILURE));
    Value::Array(array)
}

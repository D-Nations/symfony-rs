//! Port of Symfony\Component\HttpKernel\Controller\ArgumentResolver\RequestAttributeValueResolver.php,
//! with RequestValueResolver.php, SessionValueResolver.php,
//! DefaultValueResolver.php and VariadicValueResolver.php: the default
//! value resolvers.
//!
//! `RequestAttributeValueResolver` coerces a route parameter for an
//! `int` or `float` argument as PHP passes a value to a typed parameter
//! in coercive mode (a zero-padded `"06"` is 6; an overflow, a
//! leading-numeric or a non-numeric string, an array or null is a
//! `TypeError`, here a 404). PHP's deprecation for a fractional float
//! given to `int` is not reproduced; the value is truncated, as PHP does.
//!
//! `RequestValueResolver` gives the request for an argument typed
//! `Request` (as a [`crate::controller::RequestObject`]); subclasses of
//! `Request` do not exist in this port. Sessions are not ported, so no
//! request has one and `SessionValueResolver` never resolves.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Controller/ArgumentResolver/RequestAttributeValueResolver.php Controller/ArgumentResolver/RequestValueResolver.php Controller/ArgumentResolver/SessionValueResolver.php Controller/ArgumentResolver/DefaultValueResolver.php Controller/ArgumentResolver/VariadicValueResolver.php

use php_value::{
    Number, PhpString, Value,
    filter::{FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_SCALAR, FILTER_VALIDATE_BOOL, filter_var},
    numeric::{int_argument, numeric_value},
    php_array,
};
use symfony_http_foundation::request_stack::RequestHandle;

use crate::{
    argument_resolver::ValueResolverInterface,
    controller::{REQUEST_CLASS, request_value},
    controller_metadata::ArgumentMetadata,
    exception::{HttpArgs, HttpException, PhpClass, PhpException, Throwable},
};

macro_rules! resolver {
    ($name:ident, $class:literal) => {
        #[doc = concat!("`", $class, "`.")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            /// The class name.
            pub const CLASS: &'static str = concat!(
                "Symfony\\Component\\HttpKernel\\Controller\\ArgumentResolver\\",
                $class
            );
        }
    };
}

resolver!(
    RequestAttributeValueResolver,
    "RequestAttributeValueResolver"
);
resolver!(RequestValueResolver, "RequestValueResolver");
resolver!(SessionValueResolver, "SessionValueResolver");
resolver!(DefaultValueResolver, "DefaultValueResolver");
resolver!(VariadicValueResolver, "VariadicValueResolver");

fn invalid(name: &str) -> Throwable {
    HttpException::not_found(HttpArgs::message(format!(
        "The value for the \"{name}\" route parameter is invalid."
    )))
    .throwable()
}

/// A value passed to an `int` parameter in coercive mode; `None` where
/// PHP throws a `TypeError`.
fn int_parameter(value: &Value) -> Option<i64> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0; // 2^63
    match value {
        Value::Int(i) => Some(*i),
        Value::Bool(b) => Some(i64::from(*b)),
        #[allow(
            clippy::cast_possible_truncation,
            reason = "in range, truncated as PHP does"
        )]
        Value::Float(f) if f.is_finite() && (-LIMIT..LIMIT).contains(f) => Some(f.trunc() as i64),
        Value::String(s) => int_argument(s),
        Value::Float(_) | Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// A value passed to a `float` parameter in coercive mode.
fn float_parameter(value: &Value) -> Option<f64> {
    match value {
        Value::Float(f) => Some(*f),
        #[allow(clippy::cast_precision_loss, reason = "PHP converts an int to float")]
        Value::Int(i) => Some(*i as f64),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        #[allow(clippy::cast_precision_loss, reason = "PHP converts an int to float")]
        Value::String(s) => match numeric_value(s)? {
            Number::Int(i) => Some(i as f64),
            Number::Float(f) => Some(f),
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

impl ValueResolverInterface for RequestAttributeValueResolver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn resolve(
        &self,
        request: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        if argument.is_variadic() {
            return Ok(Vec::new());
        }

        let name = argument.get_name();
        let Some(value) = request.with(|r| {
            r.attributes()
                .has(name)
                .then(|| r.attributes().get(name, Value::Null))
        }) else {
            return Ok(Vec::new());
        };

        if matches!(value, Value::Null) && argument.is_nullable() {
            return Ok(vec![Value::Null]);
        }

        // Skip when no type declaration or complex types; fall back to
        // other resolvers/defaults
        let Some(type_name) = argument.get_type() else {
            return Ok(vec![value]);
        };
        if type_name.contains('|') || type_name.contains('&') {
            return Ok(vec![value]);
        }

        let value = match type_name {
            "string" => {
                if !value.is_scalar_or_stringable() {
                    return Err(invalid(name));
                }
                Value::String(
                    value
                        .to_php_string_or_stringable()
                        .unwrap_or_else(PhpString::default),
                )
            }
            "bool" => {
                let options = php_array![
                    "flags" => FILTER_NULL_ON_FAILURE | FILTER_REQUIRE_SCALAR,
                ];
                match filter_var(&value, FILTER_VALIDATE_BOOL, &Value::Array(options)) {
                    Ok(Value::Null) => return Err(invalid(name)),
                    Ok(value) => value,
                    Err(e) => {
                        return Err(PhpException::new(PhpClass::Error, e.to_string()).throwable());
                    }
                }
            }
            // Coerce the value as PHP would when passing it to the typed
            // controller argument, so a zero-padded "06" still resolves to
            // 6 while a value that would raise a TypeError (overflow,
            // "abc") becomes a 404.
            "int" => Value::Int(int_parameter(&value).ok_or_else(|| invalid(name))?),
            "float" => Value::Float(float_parameter(&value).ok_or_else(|| invalid(name))?),
            _ => value,
        };

        Ok(vec![value])
    }
}

impl ValueResolverInterface for RequestValueResolver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn resolve(
        &self,
        request: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        if request.with(|r| r.attributes().has(argument.get_name())) {
            return Ok(Vec::new());
        }

        if argument.get_type() == Some(REQUEST_CLASS) {
            return Ok(vec![request_value(request)]);
        }

        if let Some(type_name) = argument.get_type().filter(|t| t.ends_with("\\Request")) {
            return Err(PhpException::new(
                PhpClass::NearMissValueResolverException,
                format!(
                    "Looks like you required a Request object with the wrong class name \"{type_name}\". Did you mean to use \"{REQUEST_CLASS}\" instead?"
                ),
            )
            .throwable());
        }

        Ok(Vec::new())
    }
}

impl ValueResolverInterface for SessionValueResolver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    /// `!$request->hasSession()`: always, as sessions are not ported.
    fn resolve(&self, _: &RequestHandle, _: &ArgumentMetadata) -> Result<Vec<Value>, Throwable> {
        Ok(Vec::new())
    }
}

impl ValueResolverInterface for DefaultValueResolver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn resolve(
        &self,
        _: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        if argument.has_default_value() {
            return Ok(vec![argument.get_default_value()?]);
        }

        if argument.get_type().is_some() && argument.is_nullable() && !argument.is_variadic() {
            return Ok(vec![Value::Null]);
        }

        Ok(Vec::new())
    }
}

impl ValueResolverInterface for VariadicValueResolver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn resolve(
        &self,
        request: &RequestHandle,
        argument: &ArgumentMetadata,
    ) -> Result<Vec<Value>, Throwable> {
        let name = argument.get_name();
        if !argument.is_variadic() || !request.with(|r| r.attributes().has(name)) {
            return Ok(Vec::new());
        }

        let values = request.with(|r| r.attributes().get(name, Value::Null));

        let Value::Array(values) = values else {
            return Err(PhpException::new(
                PhpClass::InvalidArgumentException,
                format!(
                    "The action argument \"...${name}\" is required to be an array, the request attribute \"{name}\" contains a type of \"{}\" instead.",
                    values.debug_type()
                ),
            )
            .throwable());
        };

        Ok(values.values().cloned().collect())
    }
}

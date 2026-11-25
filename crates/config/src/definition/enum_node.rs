//! Port of Symfony\Component\Config\Definition\EnumNode.php: a `ScalarNode`
//! whose value must be one of a list (compared with `===`), or a case of
//! a PHP enum, given or cast from its backing value.
//!
//! PHP's class table is a [`Registry`] here: the enum named by
//! `$enumFqcn` must be registered in it. `BackedEnum::from()` is written
//! from recorded behaviour (tests/data/php.json): an int-backed enum takes
//! an int, or a numeric string (whitespace allowed) whose value is an int
//! in range, a fraction being truncated with an `E_DEPRECATED` that is not
//! reproduced; any other string is a `TypeError`. A string-backed enum
//! takes a string, or an int as its decimal digits. Whether an enum is
//! backed is read from its cases, so a backed enum without cases counts as
//! a unit enum.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/EnumNode.php

use php_value::{
    Object, PhpObject, Value, constants::Registry, json::JsonFlags, numeric::is_numeric,
    numeric::string_to_float, object::EnumCase,
};

use crate::{
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, ScalarKind, undefined_method},
        exception::{Class, Error},
        scalar_node::{self, scalar},
        variable_node::{self, VariableData},
    },
    php::{self, Msg},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\EnumNode";

/// `EnumNode`'s properties.
#[derive(Clone)]
pub(crate) struct EnumData {
    pub(crate) values: Vec<Value>,
    pub(crate) enum_fqcn: Option<String>,
    /// The enum's cases, when `enum_fqcn` is set.
    pub(crate) cases: Vec<Object>,
}

impl EnumData {
    /// `is_subclass_of($this->enumFqcn, \BackedEnum::class)`.
    fn backed(&self) -> bool {
        self.enum_fqcn.is_some()
            && self
                .cases
                .iter()
                .any(|c| case(c).is_some_and(|c| c.value().is_some()))
    }
}

fn case(object: &Object) -> Option<&EnumCase> {
    object.downcast_ref::<EnumCase>()
}

fn as_case(value: &Value) -> Option<&EnumCase> {
    value.as_object().and_then(case)
}

/// `new EnumNode($name, $parent, $values, $pathSeparator, $enumFqcn)`.
pub struct EnumNode;

impl EnumNode {
    /// An `EnumNode` of `values`.
    pub fn new(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        values: Vec<Value>,
    ) -> Result<Node, Error> {
        Self::with_options(
            name,
            parent,
            values,
            DEFAULT_PATH_SEPARATOR.as_bytes(),
            None,
            &Registry::empty(),
        )
    }

    /// An `EnumNode` of the cases of `enum_fqcn`, looked up in `enums`.
    pub fn of_enum(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        enum_fqcn: &str,
        enums: &Registry,
    ) -> Result<Node, Error> {
        Self::with_options(
            name,
            parent,
            Vec::new(),
            DEFAULT_PATH_SEPARATOR.as_bytes(),
            Some(enum_fqcn),
            enums,
        )
    }

    pub fn with_options(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        values: Vec<Value>,
        separator: &[u8],
        enum_fqcn: Option<&str>,
        enums: &Registry,
    ) -> Result<Node, Error> {
        let fqcn_truthy = enum_fqcn.is_some_and(|f| php::truthy(&Value::from(f)));
        if values.is_empty() && !fqcn_truthy {
            return Err(Error::new(
                Class::InvalidArgument,
                "$values must contain at least one element.",
            ));
        }
        if !values.is_empty() && fqcn_truthy {
            return Err(Error::new(
                Class::InvalidArgument,
                "$values or $enumFqcn cannot be both set.",
            ));
        }
        let mut values = values;
        let mut cases = Vec::new();
        if let Some(fqcn) = enum_fqcn {
            let Some(found) = enums.cases(fqcn) else {
                return Err(Error::new(
                    Class::InvalidArgument,
                    format!("The \"{fqcn}\" enum does not exist."),
                ));
            };
            cases = found.to_vec();
            values = cases.iter().cloned().map(Value::Object).collect();
        }
        let mut enum_class: Option<String> = None;
        for value in &values {
            match value {
                Value::Null
                | Value::Bool(_)
                | Value::Int(_)
                | Value::Float(_)
                | Value::String(_) => continue,
                _ => {}
            }
            let Some(c) = as_case(value) else {
                return Err(Error::new(
                    Class::InvalidArgument,
                    format!(
                        "\"{CLASS}\" only supports scalar, enum, or null values, \"{}\" given.",
                        value.debug_type()
                    ),
                ));
            };
            let class = c.class().to_owned();
            let first = enum_class.get_or_insert_with(|| class.clone());
            if *first != class {
                return Err(Error::new(
                    Class::InvalidArgument,
                    format!(
                        "\"{CLASS}\" only supports one type of enum, \"{first}\" and \"{class}\" passed."
                    ),
                ));
            }
        }
        scalar(
            name.as_ref(),
            parent,
            separator,
            ScalarKind::Enum(Box::new(EnumData {
                values,
                enum_fqcn: enum_fqcn.map(str::to_owned),
                cases,
            })),
        )
    }
}

impl Node {
    fn enum_data(&self, method: &str) -> Result<EnumData, Error> {
        match self.kind() {
            Kind::Variable {
                scalar: Some(ScalarKind::Enum(e)),
                ..
            } => Ok(*e),
            _ => Err(undefined_method(&self.class(), method)),
        }
    }

    /// `getValues()`.
    pub fn get_values(&self) -> Result<Vec<Value>, Error> {
        Ok(self.enum_data("getValues")?.values)
    }

    /// `getEnumFqcn()`.
    pub fn get_enum_fqcn(&self) -> Result<Option<String>, Error> {
        Ok(self.enum_data("getEnumFqcn")?.enum_fqcn)
    }

    /// `getPermissibleValues($separator, $trim)`.
    pub fn get_permissible_values(&self, separator: &[u8], trim: bool) -> Result<Vec<u8>, Error> {
        Ok(permissible_values(
            &self.enum_data("getPermissibleValues")?,
            separator,
            trim,
        ))
    }
}

fn permissible_values(data: &EnumData, separator: &[u8], trim: bool) -> Vec<u8> {
    let unescaped = JsonFlags {
        unescaped_slashes: true,
        unescaped_unicode: true,
    };
    if let (true, Some(fqcn)) = (data.backed(), &data.enum_fqcn) {
        if !trim {
            return Msg::new("value-of<\\")
                .s(fqcn)
                .s(">")
                .b(separator)
                .s("\\")
                .s(fqcn)
                .into_bytes();
        }
        return php::implode(
            separator,
            data.cases.iter().map(|c| {
                case(c)
                    .and_then(EnumCase::value)
                    .and_then(|v| php::json_encode_scalar(v, unescaped, false))
                    .unwrap_or_default()
            }),
        );
    }
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for value in &data.values {
        let text = match as_case(value) {
            Some(c) => {
                let exported = php::var_export_enum(c, c.class());
                if trim {
                    exported.trim_start_matches('\\').as_bytes().to_vec()
                } else {
                    exported.into_bytes()
                }
            }
            None => php::json_encode_scalar(value, unescaped, true).unwrap_or_default(),
        };
        // array_unique(): the first of equal strings stays.
        if !seen.contains(&text) {
            seen.push(text);
        }
    }
    php::implode(separator, seen)
}

/// `EnumNode`'s `validateType()`: enum cases, or what `ScalarNode` accepts.
pub(crate) fn validate_type(node: &Node, value: &Value) -> Result<(), Error> {
    if as_case(value).is_some() {
        return Ok(());
    }
    scalar_node::validate_type(node, value)
}

/// `$enumFqcn::from($value)` for an int or a string, or `None` where PHP
/// throws a `TypeError` or `ValueError`.
fn backed_from(data: &EnumData, value: &Value) -> Option<Value> {
    let int_backed = data
        .cases
        .iter()
        .find_map(|c| case(c).and_then(EnumCase::value))
        .is_some_and(|v| matches!(v, Value::Int(_)));
    let wanted = if int_backed {
        match value {
            Value::Int(i) => Value::Int(*i),
            Value::String(s) => Value::Int(int_from_string(s)?),
            _ => return None,
        }
    } else {
        match value {
            Value::String(s) => Value::String(s.clone()),
            Value::Int(i) => Value::from(i.to_string()),
            _ => return None,
        }
    };
    data.cases
        .iter()
        .find(|c| {
            case(c)
                .and_then(EnumCase::value)
                .is_some_and(|v| v.identical(&wanted))
        })
        .map(|c| Value::Object(c.clone()))
}

/// A string passed to an `int` parameter: a numeric string whose value is
/// an int in range (a fraction is dropped).
fn int_from_string(s: &[u8]) -> Option<i64> {
    if !is_numeric(s) {
        return None;
    }
    let trimmed = s.trim_ascii();
    let digits = trimmed.strip_prefix(b"+").unwrap_or(trimmed);
    if let Some(i) = std::str::from_utf8(digits)
        .ok()
        .and_then(|t| t.parse::<i64>().ok())
    {
        return Some(i);
    }
    let f = string_to_float(s);
    // 2^63: the first float above the int range.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    (f.is_finite() && (-LIMIT..LIMIT).contains(&f)).then(|| f.trunc() as i64)
}

fn invalid_value(node: &Node, data: &EnumData, value: &Value) -> Error {
    let display = match value {
        Value::Int(i) => i.to_string().into_bytes(),
        Value::String(s) => Msg::new("\"").b(s).s("\"").into_bytes(),
        other => format!("of type \"{}\"", other.debug_type()).into_bytes(),
    };
    let mut message = Msg::new("The value ")
        .b(&display)
        .s(" is not allowed for path \"")
        .b(&node.get_path())
        .s("\". Permissible values: ")
        .b(&permissible_values(data, b", ", true))
        .s(".")
        .into_bytes();
    if let Some(fqcn) = &data.enum_fqcn {
        // substr_replace($message, ..., -1, 0): before the final ".".
        let at = message.len() - 1;
        let insert = format!(" (cases of the \"{fqcn}\" enum)");
        message.splice(at..at, insert.into_bytes());
    }
    node.config_error(Class::InvalidConfiguration, message, false)
}

/// `EnumNode`'s `finalizeValue()`.
pub(crate) fn finalize_value(
    node: &Node,
    variable: &VariableData,
    data: &EnumData,
    value: Value,
) -> Result<Value, Error> {
    let value = variable_node::finalize_value(node, variable, value)?;
    let Some(fqcn) = &data.enum_fqcn else {
        if !data.values.iter().any(|v| v.identical(&value)) {
            return Err(invalid_value(node, data, &value));
        }
        return Ok(value);
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    if let Some(c) = as_case(&value)
        && c.class() == fqcn
    {
        return Ok(value);
    }
    if !data.backed() {
        return Err(invalid_value(node, data, &value));
    }
    if let Some(c) = as_case(&value) {
        return Err(Error::new(
            Class::InvalidConfiguration,
            format!(
                "The value should be part of the \"{fqcn}\" enum, got a value from the \"{}\" enum.",
                c.class()
            ),
        ));
    }
    if !matches!(value, Value::String(_) | Value::Int(_)) {
        return Err(Error::new(
            Class::InvalidConfiguration,
            format!(
                "Only strings and integers can be cast to a case of the \"{fqcn}\" enum, got value of type \"{}\".",
                value.debug_type()
            ),
        ));
    }
    backed_from(data, &value).ok_or_else(|| invalid_value(node, data, &value))
}

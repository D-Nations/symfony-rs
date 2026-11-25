//! Port of Doctrine\DBAL\ExpandArrayParameters.php and its exceptions,
//! `ArrayParameters\Exception.php`,
//! `ArrayParameters\Exception\MissingNamedParameter.php` and
//! `MissingPositionalParameter.php`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/ExpandArrayParameters.php src/ArrayParameters/Exception.php src/ArrayParameters/Exception/MissingNamedParameter.php src/ArrayParameters/Exception/MissingPositionalParameter.php

use std::collections::BTreeMap;
use std::fmt;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::WrapperParameterType;
use crate::sql::Visitor;

/// `Doctrine\DBAL\ArrayParameters\Exception`: a parameter that the SQL
/// names has no bound value. Both cases are `\LogicException`s in PHP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayParametersException {
    /// `MissingNamedParameter`, with the name (without the colon).
    MissingNamedParameter(PhpString),
    /// `MissingPositionalParameter`, with the zero-based index.
    MissingPositionalParameter(i64),
    /// PHP's `\TypeError` from `count()`: a value whose type is an
    /// `ArrayParameterType` is not an array, with PHP's name for its type
    /// (`true`, `string`, a class). Objects are refused too; PHP refuses a
    /// `\Countable` one one step later, with another message (see
    /// README.md, Differences).
    NotCountable(String),
}

impl ArrayParametersException {
    /// The PHP class of the exception.
    pub fn class(&self) -> &'static str {
        match self {
            Self::MissingNamedParameter(_) => {
                "Doctrine\\DBAL\\ArrayParameters\\Exception\\MissingNamedParameter"
            }
            Self::MissingPositionalParameter(_) => {
                "Doctrine\\DBAL\\ArrayParameters\\Exception\\MissingPositionalParameter"
            }
            Self::NotCountable(_) => "TypeError",
        }
    }

    /// The exception message, as PHP builds it.
    pub fn message(&self) -> PhpString {
        PhpString::from(self.to_string().into_bytes())
    }
}

impl fmt::Display for ArrayParametersException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingNamedParameter(name) => write!(
                f,
                "Named parameter \"{}\" does not have a bound value.",
                String::from_utf8_lossy(name.as_bytes())
            ),
            Self::MissingPositionalParameter(index) => write!(
                f,
                "Positional parameter at index {index} does not have a bound value."
            ),
            Self::NotCountable(type_name) => write!(
                f,
                "count(): Argument #1 ($value) must be of type Countable|array, {type_name} given"
            ),
        }
    }
}

impl std::error::Error for ArrayParametersException {}

/// The SQL parser visitor that expands list parameters (`IN (?)` with
/// `[1, 2, 3]` and `ArrayParameterType::INTEGER` becomes `IN (?, ?, ?)`)
/// and turns named parameters into positional ones.
pub struct ExpandArrayParameters<'a> {
    parameters: &'a PhpArray,
    types: &'a BTreeMap<Key, WrapperParameterType>,
    original_parameter_index: i64,
    converted_sql: Vec<u8>,
    converted_parameters: Vec<Value>,
    converted_types: BTreeMap<usize, WrapperParameterType>,
}

impl<'a> ExpandArrayParameters<'a> {
    /// `$parameters` is a list or a map by name; `$types` is keyed in the
    /// same way. A missing type binds the value as it is.
    pub fn new(parameters: &'a PhpArray, types: &'a BTreeMap<Key, WrapperParameterType>) -> Self {
        Self {
            parameters,
            types,
            original_parameter_index: 0,
            converted_sql: Vec::new(),
            converted_parameters: Vec::new(),
            converted_types: BTreeMap::new(),
        }
    }

    pub fn get_sql(&self) -> PhpString {
        PhpString::from(self.converted_sql.clone())
    }

    pub fn get_parameters(&self) -> &[Value] {
        &self.converted_parameters
    }

    /// The types by position in `get_parameters()`. Untyped parameters
    /// have no entry, as in PHP.
    pub fn get_types(&self) -> &BTreeMap<usize, WrapperParameterType> {
        &self.converted_types
    }

    fn accept_parameter(
        &mut self,
        key: &Key,
        value: &Value,
    ) -> Result<(), ArrayParametersException> {
        // `isset($this->types[$key])`: no entry, as a type is never null.
        let Some(r#type) = self.types.get(key) else {
            self.converted_sql.push(b'?');
            self.converted_parameters.push(value.clone());
            return Ok(());
        };

        let WrapperParameterType::Array(array_type) = r#type else {
            let r#type = r#type.clone();
            self.append_typed_parameter(std::slice::from_ref(value), &r#type);
            return Ok(());
        };
        let array_type = *array_type;

        let Value::Array(values) = value else {
            return Err(ArrayParametersException::NotCountable(value.value_name()));
        };
        if values.is_empty() {
            self.converted_sql.extend_from_slice(b"NULL");
            return Ok(());
        }

        let values: Vec<Value> = values.values().cloned().collect();
        self.append_typed_parameter(
            &values,
            &WrapperParameterType::Parameter(array_type.to_element_parameter_type()),
        );
        Ok(())
    }

    fn append_typed_parameter(&mut self, values: &[Value], r#type: &WrapperParameterType) {
        let placeholders = vec!["?"; values.len()].join(", ");
        self.converted_sql
            .extend_from_slice(placeholders.as_bytes());

        for value in values {
            self.converted_types
                .insert(self.converted_parameters.len(), r#type.clone());
            self.converted_parameters.push(value.clone());
        }
    }
}

impl Visitor for ExpandArrayParameters<'_> {
    type Error = ArrayParametersException;

    fn accept_positional_parameter(&mut self, _sql: &[u8]) -> Result<(), Self::Error> {
        let index = self.original_parameter_index;
        let key = Key::Int(index);

        let Some(value) = self.parameters.get(&key) else {
            return Err(ArrayParametersException::MissingPositionalParameter(index));
        };

        self.accept_parameter(&key, value)?;

        self.original_parameter_index += 1;
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        let name = sql.get(1..).unwrap_or_default();
        let key = Key::from_bytes(name);

        let Some(value) = self.parameters.get(&key) else {
            return Err(ArrayParametersException::MissingNamedParameter(
                PhpString::from(name.to_vec()),
            ));
        };

        self.accept_parameter(&key, value)
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.converted_sql.extend_from_slice(sql);
        Ok(())
    }
}

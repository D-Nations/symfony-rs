//! Port of Doctrine\DBAL\Query.php: an SQL query together with its bound
//! parameters, as a `DriverException` keeps it (`getQuery()`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query.php

use std::collections::BTreeMap;

use php_value::{Key, PhpArray};

use crate::WrapperParameterType;

/// The types of the parameters of a statement, keyed as the parameters
/// are (positions or names): `array<WrapperParameterType>`.
pub type ParameterTypes = BTreeMap<Key, WrapperParameterType>;

/// An SQL query together with its bound parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    sql: String,
    params: PhpArray,
    types: ParameterTypes,
}

// `Value`'s `===` is an equivalence on the values a query holds; `Eq` lets
// `Exception` keep deriving it.
impl Eq for Query {}

impl Query {
    pub fn new(sql: impl Into<String>, params: PhpArray, types: ParameterTypes) -> Self {
        Self {
            sql: sql.into(),
            params,
            types,
        }
    }

    /// `getSQL()`.
    pub fn get_sql(&self) -> &str {
        &self.sql
    }

    /// `getParams()`.
    pub fn get_params(&self) -> &PhpArray {
        &self.params
    }

    /// `getTypes()`.
    pub fn get_types(&self) -> &ParameterTypes {
        &self.types
    }
}

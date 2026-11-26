//! Port of Doctrine\DBAL\Schema\DefaultExpression.php and
//! DefaultExpression\CurrentDate.php, CurrentTime.php and
//! CurrentTimestamp.php.
//!
//! A default expression stands in a column's `default` as a PHP object:
//! [`DefaultExpressionObject`] wraps one into a `php_value::Value`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/DefaultExpression.php src/Schema/DefaultExpression/CurrentDate.php
//! Upstream: doctrine/dbal src/Schema/DefaultExpression/CurrentTime.php
//! Upstream: doctrine/dbal src/Schema/DefaultExpression/CurrentTimestamp.php

use std::fmt;
use std::sync::Arc;

use php_value::{PhpArray, PhpObject, Value};

use crate::platforms::AbstractPlatform;

/// Represents a default expression supported by all database platforms.
pub trait DefaultExpression: Send + Sync + fmt::Debug {
    /// `$expression::class`.
    fn class(&self) -> &'static str;

    /// Returns the SQL representation of the default expression for the
    /// given platform.
    fn to_sql(&self, platform: &dyn AbstractPlatform) -> String;
}

/// A default expression as a PHP object (`Value::Object`).
#[derive(Debug, Clone)]
pub struct DefaultExpressionObject(pub Arc<dyn DefaultExpression>);

impl DefaultExpressionObject {
    /// The expression as a PHP value.
    pub fn value(expression: impl DefaultExpression + 'static) -> Value {
        Value::Object(Arc::new(Self(Arc::new(expression))))
    }

    /// The expression a value holds (`$default instanceof
    /// DefaultExpression`).
    pub fn of(value: &Value) -> Option<&dyn DefaultExpression> {
        let object = value.as_object()?;
        let this = object.downcast_ref::<Self>()?;
        Some(this.0.as_ref())
    }
}

impl PhpObject for DefaultExpressionObject {
    fn class(&self) -> &str {
        self.0.class()
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// Represents the current date expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurrentDate;

impl CurrentDate {
    /// `new CurrentDate()` as a PHP value.
    pub fn value() -> Value {
        DefaultExpressionObject::value(Self)
    }
}

impl DefaultExpression for CurrentDate {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Schema\\DefaultExpression\\CurrentDate"
    }

    fn to_sql(&self, platform: &dyn AbstractPlatform) -> String {
        platform.get_current_date_sql()
    }
}

/// Represents the current time expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurrentTime;

impl CurrentTime {
    /// `new CurrentTime()` as a PHP value.
    pub fn value() -> Value {
        DefaultExpressionObject::value(Self)
    }
}

impl DefaultExpression for CurrentTime {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Schema\\DefaultExpression\\CurrentTime"
    }

    fn to_sql(&self, platform: &dyn AbstractPlatform) -> String {
        platform.get_current_time_sql()
    }
}

/// Represents the current timestamp expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurrentTimestamp;

impl CurrentTimestamp {
    /// `new CurrentTimestamp()` as a PHP value.
    pub fn value() -> Value {
        DefaultExpressionObject::value(Self)
    }
}

impl DefaultExpression for CurrentTimestamp {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Schema\\DefaultExpression\\CurrentTimestamp"
    }

    fn to_sql(&self, platform: &dyn AbstractPlatform) -> String {
        platform.get_current_timestamp_sql()
    }
}

//! Port of Doctrine\DBAL\Query\Expression\ExpressionBuilder.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/Expression/ExpressionBuilder.php

use crate::ConnectionStandIn;
use crate::Exception;

use super::{CompositeExpression, ExpressionPart};

/// `ExpressionBuilder` class is responsible to dynamically create SQL query
/// parts.
pub struct ExpressionBuilder<'c, C: ?Sized> {
    connection: &'c C,
}

impl<C: ?Sized> Clone for ExpressionBuilder<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ?Sized> Copy for ExpressionBuilder<'_, C> {}

impl<C: ?Sized> std::fmt::Debug for ExpressionBuilder<'_, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpressionBuilder").finish_non_exhaustive()
    }
}

impl<'c, C: ?Sized> ExpressionBuilder<'c, C> {
    pub const EQ: &'static str = "=";
    pub const NEQ: &'static str = "<>";
    pub const LT: &'static str = "<";
    pub const LTE: &'static str = "<=";
    pub const GT: &'static str = ">";
    pub const GTE: &'static str = ">=";

    /// Initializes a new `ExpressionBuilder` on the DBAL connection.
    pub fn new(connection: &'c C) -> Self {
        Self { connection }
    }

    /// Creates a conjunction of the given expressions.
    pub fn and(
        &self,
        expression: impl Into<ExpressionPart>,
        expressions: impl IntoIterator<Item = ExpressionPart>,
    ) -> CompositeExpression {
        CompositeExpression::and(expression, expressions)
    }

    /// Creates a disjunction of the given expressions.
    pub fn or(
        &self,
        expression: impl Into<ExpressionPart>,
        expressions: impl IntoIterator<Item = ExpressionPart>,
    ) -> CompositeExpression {
        CompositeExpression::or(expression, expressions)
    }

    /// Creates a comparison expression: `$x $operator $y`.
    pub fn comparison(&self, x: &str, operator: &str, y: &str) -> String {
        format!("{x} {operator} {y}")
    }

    /// Creates an equality comparison expression (`u.id = ?`).
    pub fn eq(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::EQ, y)
    }

    /// Creates a non equality comparison expression (`u.id <> 1`).
    pub fn neq(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::NEQ, y)
    }

    /// Creates a lower-than comparison expression (`u.id < ?`).
    pub fn lt(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::LT, y)
    }

    /// Creates a lower-than-equal comparison expression (`u.id <= ?`).
    pub fn lte(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::LTE, y)
    }

    /// Creates a greater-than comparison expression (`u.id > ?`).
    pub fn gt(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::GT, y)
    }

    /// Creates a greater-than-equal comparison expression (`u.id >= ?`).
    pub fn gte(&self, x: &str, y: &str) -> String {
        self.comparison(x, Self::GTE, y)
    }

    /// Creates an IS NULL expression with the given arguments.
    pub fn is_null(&self, x: &str) -> String {
        format!("{x} IS NULL")
    }

    /// Creates an IS NOT NULL expression with the given arguments.
    pub fn is_not_null(&self, x: &str) -> String {
        format!("{x} IS NOT NULL")
    }

    /// Creates a LIKE comparison expression.
    pub fn like(&self, expression: &str, pattern: &str, escape_char: Option<&str>) -> String {
        self.comparison(expression, "LIKE", pattern) + &escape(escape_char)
    }

    /// Creates a NOT LIKE comparison expression.
    pub fn not_like(&self, expression: &str, pattern: &str, escape_char: Option<&str>) -> String {
        self.comparison(expression, "NOT LIKE", pattern) + &escape(escape_char)
    }

    /// Creates an IN () comparison expression. PHP takes a string or an
    /// array of strings for `$y`; `(array) $y` makes a string a list of
    /// one, so a slice covers both.
    pub fn r#in<S: AsRef<str>>(&self, x: &str, y: &[S]) -> String {
        self.comparison(x, "IN", &format!("({})", implode(y)))
    }

    /// Creates a NOT IN () comparison expression (see [`Self::r#in`]).
    pub fn not_in<S: AsRef<str>>(&self, x: &str, y: &[S]) -> String {
        self.comparison(x, "NOT IN", &format!("({})", implode(y)))
    }
}

impl<C: ConnectionStandIn + ?Sized> ExpressionBuilder<'_, C> {
    /// Creates an SQL literal expression from the string.
    ///
    /// The usage of this method is discouraged. Use prepared statements
    /// or `AbstractPlatform::quoteStringLiteral()` instead.
    pub fn literal(&self, input: &str) -> Result<String, Exception> {
        self.connection.quote(input)
    }
}

fn escape(escape_char: Option<&str>) -> String {
    escape_char.map_or_else(String::new, |c| format!(" ESCAPE {c}"))
}

fn implode<S: AsRef<str>>(items: &[S]) -> String {
    items
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}

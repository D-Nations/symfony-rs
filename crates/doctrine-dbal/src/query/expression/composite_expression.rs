//! Port of Doctrine\DBAL\Query\Expression\CompositeExpression.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/Expression/CompositeExpression.php

use std::fmt;

/// The type of a composite expression (`TYPE_AND`, `TYPE_OR`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositeType {
    And,
    Or,
}

impl CompositeType {
    /// The SQL keyword (`CompositeExpression::TYPE_AND` is `'AND'`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::And => CompositeExpression::TYPE_AND,
            Self::Or => CompositeExpression::TYPE_OR,
        }
    }
}

impl fmt::Display for CompositeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One part of a composite expression, or a predicate given to
/// `QueryBuilder::where()` and friends: PHP's `self|string`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionPart {
    Sql(String),
    Composite(CompositeExpression),
}

impl fmt::Display for ExpressionPart {
    /// `(string) $part`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(sql) => f.write_str(sql),
            Self::Composite(composite) => composite.fmt(f),
        }
    }
}

impl From<&str> for ExpressionPart {
    fn from(value: &str) -> Self {
        Self::Sql(value.to_owned())
    }
}

impl From<String> for ExpressionPart {
    fn from(value: String) -> Self {
        Self::Sql(value)
    }
}

impl From<&String> for ExpressionPart {
    fn from(value: &String) -> Self {
        Self::Sql(value.clone())
    }
}

impl From<CompositeExpression> for ExpressionPart {
    fn from(value: CompositeExpression) -> Self {
        Self::Composite(value)
    }
}

/// Composite expression is responsible to build a group of similar
/// expression.
///
/// This class is immutable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeExpression {
    r#type: CompositeType,
    /// Each expression part of the composite expression (never empty).
    parts: Vec<ExpressionPart>,
}

impl CompositeExpression {
    /// Constant that represents an AND composite expression.
    pub const TYPE_AND: &'static str = "AND";
    /// Constant that represents an OR composite expression.
    pub const TYPE_OR: &'static str = "OR";

    /// `@internal` Use the `and()` / `or()` factory methods.
    pub fn new(
        r#type: CompositeType,
        part: impl Into<ExpressionPart>,
        parts: impl IntoIterator<Item = ExpressionPart>,
    ) -> Self {
        let mut all = vec![part.into()];
        all.extend(parts);
        Self { r#type, parts: all }
    }

    pub fn and(
        part: impl Into<ExpressionPart>,
        parts: impl IntoIterator<Item = ExpressionPart>,
    ) -> Self {
        Self::new(CompositeType::And, part, parts)
    }

    pub fn or(
        part: impl Into<ExpressionPart>,
        parts: impl IntoIterator<Item = ExpressionPart>,
    ) -> Self {
        Self::new(CompositeType::Or, part, parts)
    }

    /// Returns a new `CompositeExpression` with the given parts added.
    #[must_use]
    pub fn with(
        &self,
        part: impl Into<ExpressionPart>,
        parts: impl IntoIterator<Item = ExpressionPart>,
    ) -> Self {
        let mut that = self.clone();
        that.parts.push(part.into());
        that.parts.extend(parts);
        that
    }

    /// Retrieves the amount of expressions on composite expression.
    pub fn count(&self) -> usize {
        self.parts.len()
    }

    /// Returns the type of this composite expression (AND/OR).
    pub fn get_type(&self) -> CompositeType {
        self.r#type
    }

    /// The parts, in order.
    pub fn parts(&self) -> &[ExpressionPart] {
        &self.parts
    }
}

impl fmt::Display for CompositeExpression {
    /// Retrieves the string representation of this composite expression
    /// (`__toString()`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let [only] = self.parts.as_slice() {
            return only.fmt(f);
        }

        f.write_str("(")?;
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                write!(f, ") {} (", self.r#type)?;
            }
            part.fmt(f)?;
        }
        f.write_str(")")
    }
}

//! Port of Symfony\Component\ExpressionLanguage\Expression.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Expression.php

use std::{fmt, sync::Arc};

use php_value::PhpString;

use crate::parsed_expression::ParsedExpression;

/// An expression's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    expression: PhpString,
}

impl Expression {
    pub fn new(expression: impl Into<PhpString>) -> Self {
        Self {
            expression: expression.into(),
        }
    }

    /// `(string) $expression`.
    pub fn as_php_string(&self) -> &PhpString {
        &self.expression
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.expression.to_string_lossy())
    }
}

/// What `ExpressionLanguage`'s methods take (`Expression|string`): a
/// source, an `Expression`, or a `ParsedExpression`, which is not parsed
/// again.
#[derive(Debug, Clone)]
pub enum ExpressionInput {
    Source(PhpString),
    Parsed(Arc<ParsedExpression>),
}

impl ExpressionInput {
    /// `(string) $expression`.
    pub fn source(&self) -> &PhpString {
        match self {
            Self::Source(s) => s,
            Self::Parsed(p) => p.expression().as_php_string(),
        }
    }
}

impl From<&str> for ExpressionInput {
    fn from(s: &str) -> Self {
        Self::Source(PhpString::from(s))
    }
}

impl From<String> for ExpressionInput {
    fn from(s: String) -> Self {
        Self::Source(PhpString::from(s))
    }
}

impl From<&[u8]> for ExpressionInput {
    fn from(s: &[u8]) -> Self {
        Self::Source(PhpString::from(s))
    }
}

impl From<PhpString> for ExpressionInput {
    fn from(s: PhpString) -> Self {
        Self::Source(s)
    }
}

impl From<&Expression> for ExpressionInput {
    fn from(e: &Expression) -> Self {
        Self::Source(e.expression.clone())
    }
}

impl From<Expression> for ExpressionInput {
    fn from(e: Expression) -> Self {
        Self::Source(e.expression)
    }
}

impl From<Arc<ParsedExpression>> for ExpressionInput {
    fn from(p: Arc<ParsedExpression>) -> Self {
        Self::Parsed(p)
    }
}

impl From<&Arc<ParsedExpression>> for ExpressionInput {
    fn from(p: &Arc<ParsedExpression>) -> Self {
        Self::Parsed(Arc::clone(p))
    }
}

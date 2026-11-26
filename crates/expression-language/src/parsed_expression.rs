//! Port of Symfony\Component\ExpressionLanguage\ParsedExpression.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language ParsedExpression.php

use std::fmt;

use php_value::PhpString;

use crate::{expression::Expression, node::Node};

/// An expression and its nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedExpression {
    expression: Expression,
    nodes: Node,
}

impl ParsedExpression {
    pub fn new(expression: impl Into<PhpString>, nodes: Node) -> Self {
        Self {
            expression: Expression::new(expression),
            nodes,
        }
    }

    /// The expression (`ParsedExpression` extends `Expression`).
    pub fn expression(&self) -> &Expression {
        &self.expression
    }

    /// `getNodes()`.
    pub fn nodes(&self) -> &Node {
        &self.nodes
    }
}

impl fmt::Display for ParsedExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expression.fmt(f)
    }
}

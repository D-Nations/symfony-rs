//! Port of Symfony\Component\ExpressionLanguage\Node\ConditionalNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/ConditionalNode.php

use php_value::{PhpArray, Value};

use super::Node;
use crate::{compiler::Compiler, expression_function::Functions, syntax_error::Error};

/// `expr1 ? expr2 : expr3`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalNode {
    pub expr1: Box<Node>,
    pub expr2: Box<Node>,
    pub expr3: Box<Node>,
}

impl ConditionalNode {
    pub fn new(expr1: impl Into<Node>, expr2: impl Into<Node>, expr3: impl Into<Node>) -> Self {
        Self {
            expr1: Box::new(expr1.into()),
            expr2: Box::new(expr2.into()),
            expr3: Box::new(expr3.into()),
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        compiler
            .raw("((")
            .compile(&self.expr1)?
            .raw(") ? (")
            .compile(&self.expr2)?
            .raw(") : (")
            .compile(&self.expr3)?
            .raw("))");
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        if self.expr1.evaluate(functions, values)?.is_truthy() {
            return self.expr2.evaluate(functions, values);
        }
        self.expr3.evaluate(functions, values)
    }
}

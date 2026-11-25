//! Port of Symfony\Component\ExpressionLanguage\Node\NullCoalesceNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/NullCoalesceNode.php

use php_value::{PhpArray, Value};

use super::Node;
use crate::{compiler::Compiler, expression_function::Functions, syntax_error::Error};

/// `expr1 ?? expr2`.
#[derive(Debug, Clone, PartialEq)]
pub struct NullCoalesceNode {
    pub expr1: Box<Node>,
    pub expr2: Box<Node>,
}

impl NullCoalesceNode {
    pub fn new(expr1: impl Into<Node>, expr2: impl Into<Node>) -> Self {
        Self {
            expr1: Box::new(expr1.into()),
            expr2: Box::new(expr2.into()),
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        compiler
            .raw("((")
            .compile(&self.expr1)?
            .raw(") ?? (")
            .compile(&self.expr2)?
            .raw("))");
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        mark_null_coalesce(&self.expr1);
        match self.expr1.evaluate(functions, values)? {
            Value::Null => self.expr2.evaluate(functions, values),
            value => Ok(value),
        }
    }
}

/// `addNullCoalesceAttributeToGetAttrNodes()`: the `GetAttrNode`s at the
/// top of `node`, and their `GetAttrNode` children, recursively.
fn mark_null_coalesce(node: &Node) {
    let Node::GetAttr(get_attr) = node else {
        return;
    };
    get_attr.is_null_coalesce.set(true);
    for child in node.children() {
        mark_null_coalesce(child);
    }
}

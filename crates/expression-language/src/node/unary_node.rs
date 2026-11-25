//! Port of Symfony\Component\ExpressionLanguage\Node\UnaryNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/UnaryNode.php

use php_value::{PhpArray, Value};

use super::Node;
use crate::{
    compiler::Compiler,
    expression_function::Functions,
    php,
    syntax_error::{Class, Error},
};

/// `not x`, `!x`, `-x`, `+x`, `~x`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryNode {
    pub operator: String,
    pub node: Box<Node>,
}

impl UnaryNode {
    pub fn new(operator: impl Into<String>, node: impl Into<Node>) -> Self {
        Self {
            operator: operator.into(),
            node: Box::new(node.into()),
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        let php_operator = match self.operator.as_str() {
            "!" | "not" => "!",
            "+" => "+",
            "-" => "-",
            "~" => "~",
            other => {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "Symfony\\Component\\ExpressionLanguage\\Compiler::raw(): Argument #1 ($string) must be of type string, null given (undefined operator \"{other}\")"
                    ),
                ));
            }
        };
        compiler
            .raw("(")
            .raw(php_operator)
            .compile(&self.node)?
            .raw(")");
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let value = self.node.evaluate(functions, values)?;
        match self.operator.as_str() {
            "not" | "!" => Ok(Value::Bool(!value.is_truthy())),
            "+" => php::plus(&value),
            "-" => php::negate(&value),
            "~" => php::bitwise_not(&value),
            _ => Ok(value),
        }
    }
}

//! Port of Symfony\Component\ExpressionLanguage\Node\FunctionNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/FunctionNode.php

use php_value::{PhpArray, PhpString, Value};

use super::Node;
use crate::{
    compiler::Compiler,
    expression_function::Functions,
    syntax_error::{Class, Error},
};

/// A call of a registered function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionNode {
    pub name: PhpString,
    /// A `Node` holding the arguments.
    pub arguments: Box<Node>,
}

impl FunctionNode {
    pub fn new(name: impl Into<PhpString>, arguments: impl Into<Node>) -> Self {
        Self {
            name: name.into(),
            arguments: Box::new(arguments.into()),
        }
    }

    fn argument_nodes(&self) -> Vec<&Node> {
        self.arguments.children()
    }

    fn not_callable() -> Error {
        Error::new(Class::Error, "Value of type null is not callable")
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        let mut arguments = Vec::new();
        for node in self.argument_nodes() {
            arguments.push(compiler.subcompile(node)?);
        }
        let function = compiler
            .function(self.name.as_bytes())
            .cloned()
            .ok_or_else(Self::not_callable)?;
        let source = function.compile(&arguments)?;
        compiler.raw_bytes(source.as_bytes());
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let mut arguments = Vec::new();
        for node in self.argument_nodes() {
            arguments.push(node.evaluate(functions, values)?);
        }
        let function = functions
            .get(self.name.as_bytes())
            .ok_or_else(Self::not_callable)?;
        function.evaluate(values, &arguments)
    }

    pub(crate) fn dump_into(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        out.extend_from_slice(self.name.as_bytes());
        out.push(b'(');
        for (i, node) in self.argument_nodes().into_iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(b", ");
            }
            node.dump_into(out)?;
        }
        out.push(b')');
        Ok(())
    }
}

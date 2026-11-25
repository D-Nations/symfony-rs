//! Port of Symfony\Component\ExpressionLanguage\Compiler.php: builds the
//! PHP source an expression compiles to.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Compiler.php

use php_value::{PhpString, Value, numeric::var_export_float};

use crate::{
    expression_function::{ExpressionFunction, Functions},
    node::Node,
    php,
    syntax_error::Error,
};

/// Compiles nodes to PHP source.
#[derive(Debug, Clone, Default)]
pub struct Compiler {
    functions: Functions,
    source: Vec<u8>,
}

impl Compiler {
    pub fn new(functions: Functions) -> Self {
        Self {
            functions,
            source: Vec::new(),
        }
    }

    /// `getFunction($name)`.
    pub fn function(&self, name: &[u8]) -> Option<&ExpressionFunction> {
        self.functions.get(name)
    }

    /// `getSource()`.
    pub fn source(&self) -> PhpString {
        PhpString::from(self.source.as_slice())
    }

    /// `reset()`.
    pub fn reset(&mut self) -> &mut Self {
        self.source.clear();
        self
    }

    /// `compile($node)`.
    pub fn compile(&mut self, node: &Node) -> Result<&mut Self, Error> {
        node.compile(self)?;
        Ok(self)
    }

    /// `subcompile($node)`: the node's source, apart from the rest.
    pub fn subcompile(&mut self, node: &Node) -> Result<PhpString, Error> {
        let current = std::mem::take(&mut self.source);
        let result = node.compile(self);
        let source = std::mem::replace(&mut self.source, current);
        result?;
        Ok(PhpString::from(source))
    }

    /// `raw($string)`.
    pub fn raw(&mut self, string: &str) -> &mut Self {
        self.source.extend_from_slice(string.as_bytes());
        self
    }

    /// `raw($string)` for bytes.
    pub fn raw_bytes(&mut self, string: &[u8]) -> &mut Self {
        self.source.extend_from_slice(string);
        self
    }

    /// `string($value)`: a double-quoted PHP string.
    pub fn string(&mut self, value: &[u8]) -> &mut Self {
        self.source.push(b'"');
        self.source
            .extend_from_slice(&php::addcslashes(value, b"\0\t\"$\\"));
        self.source.push(b'"');
        self
    }

    /// `repr($value)`: a PHP literal for the value.
    pub fn repr(&mut self, value: &Value) -> Result<&mut Self, Error> {
        match value {
            Value::Int(i) => {
                let text = int_export(*i);
                self.raw(&text);
            }
            Value::Float(f) => {
                let text = var_export_float(*f);
                self.raw(&text);
            }
            Value::Null => {
                self.raw("null");
            }
            Value::Bool(b) => {
                self.raw(if *b { "true" } else { "false" });
            }
            Value::Array(array) => {
                self.raw("[");
                let mut first = true;
                for (key, value) in array.iter() {
                    if !first {
                        self.raw(", ");
                    }
                    first = false;
                    self.repr(&key.to_value())?;
                    self.raw(" => ");
                    self.repr(value)?;
                }
                self.raw("]");
            }
            Value::String(s) => {
                self.string(s.as_bytes());
            }
            Value::Object(o) => {
                let text = o.to_php_string().ok_or_else(|| {
                    Error::type_error(format!(
                        "Symfony\\Component\\ExpressionLanguage\\Compiler::string(): Argument #1 ($value) must be of type string, {} given",
                        value.debug_type()
                    ))
                })?;
                self.string(text.as_bytes());
            }
        }
        Ok(self)
    }
}

/// `var_export($int, true)`: `PHP_INT_MIN` is written as an expression.
pub(crate) fn int_export(i: i64) -> String {
    if i == i64::MIN {
        "-9223372036854775807-1".to_owned()
    } else {
        i.to_string()
    }
}

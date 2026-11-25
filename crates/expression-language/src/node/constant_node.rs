//! Port of Symfony\Component\ExpressionLanguage\Node\ConstantNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/ConstantNode.php

use php_value::{Value, numeric::is_numeric};

use super::{dump_string, is_hash};
use crate::{compiler::Compiler, php, syntax_error::Error};

/// A constant value; also a property or method name (`is_identifier`),
/// which may be reached null-safely (`?.`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConstantNode {
    pub value: Value,
    pub is_identifier: bool,
    pub is_null_safe: bool,
}

impl ConstantNode {
    /// `new ConstantNode($value)`.
    pub fn new(value: impl Into<Value>) -> Self {
        Self::with(value, false, false)
    }

    /// `new ConstantNode($value, $isIdentifier, $isNullSafe)`.
    pub fn with(value: impl Into<Value>, is_identifier: bool, is_null_safe: bool) -> Self {
        Self {
            value: value.into(),
            is_identifier,
            is_null_safe,
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        compiler.repr(&self.value)?;
        Ok(())
    }

    pub(crate) fn dump_into(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        let value = &self.value;
        if self.is_identifier {
            out.extend_from_slice(php::to_string(value)?.as_bytes());
            return Ok(());
        }
        match value {
            Value::Bool(true) => out.extend_from_slice(b"true"),
            Value::Bool(false) => out.extend_from_slice(b"false"),
            Value::Null => out.extend_from_slice(b"null"),
            Value::Int(_) | Value::Float(_) => {
                out.extend_from_slice(php::to_string(value)?.as_bytes());
            }
            Value::String(s) if is_numeric(s.as_bytes()) => out.extend_from_slice(s.as_bytes()),
            Value::String(s) => dump_string(s.as_bytes(), out),
            Value::Object(_) => {
                return Err(Error::type_error(format!(
                    "Symfony\\Component\\ExpressionLanguage\\Node\\Node::dumpString(): Argument #1 ($value) must be of type string, {} given",
                    value.debug_type()
                )));
            }
            Value::Array(array) => {
                let hash = is_hash(array);
                out.push(if hash { b'{' } else { b'[' });
                for (i, (k, v)) in array.iter().enumerate() {
                    if i > 0 {
                        out.extend_from_slice(b", ");
                    }
                    if hash {
                        Self::new(k.to_value()).dump_into(out)?;
                        out.extend_from_slice(b": ");
                    }
                    Self::new(v.clone()).dump_into(out)?;
                }
                out.push(if hash { b'}' } else { b']' });
            }
        }
        Ok(())
    }
}

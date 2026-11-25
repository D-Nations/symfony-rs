//! Port of Symfony\Component\ExpressionLanguage\Node\NameNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/NameNode.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::compiler::Compiler;

/// A variable.
#[derive(Debug, Clone, PartialEq)]
pub struct NameNode {
    pub name: PhpString,
}

impl NameNode {
    pub fn new(name: impl Into<PhpString>) -> Self {
        Self { name: name.into() }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) {
        compiler.raw("$").raw_bytes(self.name.as_bytes());
    }

    /// `$values[$name]`; an undefined name is `null` (PHP warns).
    pub(crate) fn evaluate(&self, values: &PhpArray) -> Value {
        values
            .get(&Key::from_bytes(self.name.as_bytes()))
            .cloned()
            .unwrap_or(Value::Null)
    }
}

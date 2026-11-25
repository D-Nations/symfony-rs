//! Port of Symfony\Component\ExpressionLanguage\Node\NullCoalescedNameNode.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/NullCoalescedNameNode.php

use php_value::PhpString;

use crate::compiler::Compiler;

/// An unknown variable on the left of `??`: always `null`.
#[derive(Debug, Clone, PartialEq)]
pub struct NullCoalescedNameNode {
    pub name: PhpString,
}

impl NullCoalescedNameNode {
    pub fn new(name: impl Into<PhpString>) -> Self {
        Self { name: name.into() }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) {
        compiler
            .raw("$")
            .raw_bytes(self.name.as_bytes())
            .raw(" ?? null");
    }
}

//! Port of Symfony\Component\Config\Definition\Builder\NormalizationBuilder.php:
//! the rules run before a value is normalized, the types they declare
//! and the XML singular-to-plural remappings. Upstream reaches it through
//! the protected `normalization()`; `beforeNormalization()` adds an
//! expression and `fixXmlConfig()` a remapping.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/NormalizationBuilder.php

use php_value::PhpString;

use crate::definition::{
    base_node::Closure,
    builder::expr_builder::{Expr, ExprRef, ExprType},
};

/// NormalizationBuilder's properties.
#[derive(Clone, Default)]
pub(crate) struct NormalizationBuilder {
    pub(crate) before: Vec<Expr>,
    /// What `getNode()` read from the expressions' `allowedTypes`; `None`
    /// where PHP reads a closure's (undefined) property as null.
    pub(crate) declared_types: Vec<Option<ExprType>>,
    pub(crate) remappings: Vec<(PhpString, PhpString)>,
}

impl NormalizationBuilder {
    /// `remap($key, $plural)`: the plural defaults to the key and "s".
    pub(crate) fn remap(&mut self, key: &[u8], plural: Option<&[u8]>) {
        let plural = match plural {
            Some(p) => p.to_vec(),
            None => [key, b"s"].concat(),
        };
        self.remappings
            .push((PhpString::from(key), PhpString::from(plural)));
    }

    /// `before()`: a new expression.
    pub(crate) fn before(&mut self) -> ExprRef {
        let expr = ExprRef::default();
        self.before.push(Expr::Builder(expr.clone()));
        expr
    }

    /// `before($closure)`.
    #[allow(
        dead_code,
        reason = "upstream's protected API; no caller in Config yet"
    )]
    pub(crate) fn before_closure(&mut self, closure: Closure) {
        self.before.push(Expr::Closure(closure));
    }
}

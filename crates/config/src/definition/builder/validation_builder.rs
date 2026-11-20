//! Port of Symfony\Component\Config\Definition\Builder\ValidationBuilder.php:
//! the rules run when a value is finalized. Upstream reaches it through
//! the protected `validation()`; `validate()` adds an expression.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/ValidationBuilder.php

use crate::definition::{
    base_node::Closure,
    builder::expr_builder::{Expr, ExprRef},
};

/// ValidationBuilder's properties.
#[derive(Clone, Default)]
pub(crate) struct ValidationBuilder {
    pub(crate) rules: Vec<Expr>,
}

impl ValidationBuilder {
    /// `rule()`: a new expression.
    pub(crate) fn rule(&mut self) -> ExprRef {
        let expr = ExprRef::default();
        self.rules.push(Expr::Builder(expr.clone()));
        expr
    }

    /// `rule($closure)`.
    #[allow(
        dead_code,
        reason = "upstream's protected API; no caller in Config yet"
    )]
    pub(crate) fn rule_closure(&mut self, closure: Closure) {
        self.rules.push(Expr::Closure(closure));
    }
}

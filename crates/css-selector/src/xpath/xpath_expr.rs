//! Port of Symfony\Component\CssSelector\XPath\XPathExpr.php.
//!
//! Upstream: symfony/css-selector XPath/XPathExpr.php

use super::Translator;
use crate::php::{cat, truthy};

/// An XPath expression being built: path, element and condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPathExpr {
    path: Vec<u8>,
    element: Vec<u8>,
    condition: Vec<u8>,
}

impl Default for XPathExpr {
    fn default() -> Self {
        Self::new(b"", b"*", b"", false)
    }
}

impl XPathExpr {
    pub fn new(path: &[u8], element: &[u8], condition: &[u8], star_prefix: bool) -> Self {
        let mut expr = Self {
            path: path.to_vec(),
            element: element.to_vec(),
            condition: condition.to_vec(),
        };
        if star_prefix {
            expr.add_star_prefix();
        }
        expr
    }

    pub fn get_element(&self) -> &[u8] {
        &self.element
    }

    /// ANDs (or `operator`s) a condition onto a true one; replaces a
    /// false one (`""`, `"0"`), as upstream's truthiness test does.
    pub fn add_condition(&mut self, condition: &[u8], operator: &[u8]) -> &mut Self {
        self.condition = if truthy(&self.condition) {
            cat(&[
                b"(",
                &self.condition,
                b") ",
                operator,
                b" (",
                condition,
                b")",
            ])
        } else {
            condition.to_vec()
        };
        self
    }

    /// [`Self::add_condition`] with `and`.
    pub fn and(&mut self, condition: &[u8]) -> &mut Self {
        self.add_condition(condition, b"and")
    }

    pub fn get_condition(&self) -> &[u8] {
        &self.condition
    }

    pub fn add_name_test(&mut self) -> &mut Self {
        if self.element != b"*" {
            let test = cat(&[b"name() = ", &Translator::get_xpath_literal(&self.element)]);
            self.and(&test);
            self.element = b"*".to_vec();
        }
        self
    }

    pub fn add_star_prefix(&mut self) -> &mut Self {
        self.path.extend_from_slice(b"*/");
        self
    }

    /// Joins another expression with a combiner. With
    /// `has_inner_conditions`, `expr`'s condition becomes a `[...]`
    /// predicate on its element (for relative selectors in `:has()`).
    pub fn join(
        &mut self,
        combiner: &[u8],
        expr: &XPathExpr,
        closing_combiner: Option<&[u8]>,
        has_inner_conditions: bool,
    ) -> &mut Self {
        let mut path = cat(&[&self.to_php_string(), combiner]);
        if expr.path != b"*/" {
            path.extend_from_slice(&expr.path);
        }
        self.path = path;

        if has_inner_conditions {
            self.element.clone_from(&expr.element);
            if truthy(&expr.condition) {
                self.element = cat(&[&self.element, b"[", &expr.condition, b"]"]);
            }
            if let Some(closing) = closing_combiner.filter(|c| truthy(c)) {
                self.element.extend_from_slice(closing);
            }
        } else {
            self.element = cat(&[&expr.element, closing_combiner.unwrap_or_default()]);
            self.condition.clone_from(&expr.condition);
        }
        self
    }

    /// `__toString()`.
    pub fn to_php_string(&self) -> Vec<u8> {
        if self.condition.is_empty() {
            cat(&[&self.path, &self.element])
        } else {
            cat(&[&self.path, &self.element, b"[", &self.condition, b"]"])
        }
    }
}

//! Port of Symfony\Component\CssSelector\XPath\Extension\ExtensionInterface.php
//! and AbstractExtension.php: an extension contributes translators by
//! name, and the translator keeps the last one registered for each name.
//!
//! Upstream: symfony/css-selector XPath/Extension/ExtensionInterface.php XPath/Extension/AbstractExtension.php

mod attribute_matching;
mod combination;
mod function;
mod html;
mod node;
mod pseudo_class;
mod relation;

use std::any::Any;
use std::rc::Rc;

pub use attribute_matching::AttributeMatchingExtension;
pub use combination::CombinationExtension;
pub use function::FunctionExtension;
pub use html::HtmlExtension;
pub use node::NodeExtension;
pub use pseudo_class::PseudoClassExtension;
pub use relation::RelationExtension;

use super::{Translator, XPathExpr};
use crate::exception::Exception;
use crate::node::{FunctionNode, Node};

/// Receives the node and the translator.
pub type NodeTranslator = Rc<dyn Fn(&Node, &Translator) -> Result<XPathExpr, Exception>>;
/// Receives the left and the right expression.
pub type CombinationTranslator = Rc<dyn Fn(XPathExpr, XPathExpr) -> Result<XPathExpr, Exception>>;
pub type FunctionTranslator = Rc<dyn Fn(XPathExpr, &FunctionNode) -> Result<XPathExpr, Exception>>;
pub type PseudoClassTranslator = Rc<dyn Fn(XPathExpr) -> Result<XPathExpr, Exception>>;
/// Receives the expression, the attribute expression and the value.
pub type AttributeMatchingTranslator =
    Rc<dyn Fn(XPathExpr, &[u8], Option<&[u8]>) -> Result<XPathExpr, Exception>>;

/// `ExtensionInterface`, with `AbstractExtension`'s empty defaults.
pub trait ExtensionInterface: Any {
    fn get_node_translators(&self) -> Vec<(Vec<u8>, NodeTranslator)> {
        Vec::new()
    }

    fn get_combination_translators(&self) -> Vec<(Vec<u8>, CombinationTranslator)> {
        Vec::new()
    }

    fn get_function_translators(&self) -> Vec<(Vec<u8>, FunctionTranslator)> {
        Vec::new()
    }

    fn get_pseudo_class_translators(&self) -> Vec<(Vec<u8>, PseudoClassTranslator)> {
        Vec::new()
    }

    fn get_attribute_matching_translators(&self) -> Vec<(Vec<u8>, AttributeMatchingTranslator)> {
        Vec::new()
    }

    /// Combination translators inside `:has()`.
    fn get_relative_combination_translators(&self) -> Vec<(Vec<u8>, CombinationTranslator)> {
        Vec::new()
    }

    fn get_name(&self) -> String;

    /// For downcasting, as `HtmlExtension` does to reach `NodeExtension`.
    fn as_any(&self) -> &dyn Any;
}

/// A named translator entry.
pub(crate) fn entry<T>(name: &str, translator: T) -> (Vec<u8>, T) {
    (name.as_bytes().to_vec(), translator)
}

/// `$xpath->addCondition($condition)`, by value.
pub(crate) fn with_condition(mut xpath: XPathExpr, condition: &[u8]) -> XPathExpr {
    xpath.and(condition);
    xpath
}

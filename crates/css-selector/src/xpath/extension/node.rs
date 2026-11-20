//! Port of Symfony\Component\CssSelector\XPath\Extension\NodeExtension.php.
//! The flags are shared with the registered translators, so setting one
//! after registration (as `HtmlExtension` does) takes effect, as upstream's
//! `$this` closures see it.
//!
//! Upstream: symfony/css-selector XPath/Extension/NodeExtension.php

use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use super::{ExtensionInterface, NodeTranslator, entry, with_condition};
use crate::exception::Exception;
use crate::node::{
    AttributeNode, ClassNode, CombinedSelectorNode, ElementNode, FunctionNode, HashNode,
    NegationNode, Node, PseudoNode, RelationNode, SelectorNode,
};
use crate::php::{cat, strtolower, substr, truthy};
use crate::xpath::{Translator, XPathExpr};

#[derive(Debug, Clone, Default)]
pub struct NodeExtension {
    flags: Rc<Cell<u32>>,
}

/// `~^[a-zA-Z_][a-zA-Z0-9_.-]*$~`.
fn is_safe_name(name: &[u8]) -> bool {
    match name.split_first() {
        Some((first, rest)) => {
            (first.is_ascii_alphabetic() || *first == b'_')
                && rest
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        }
        None => false,
    }
}

impl NodeExtension {
    pub const ELEMENT_NAME_IN_LOWER_CASE: u32 = 1;
    pub const ATTRIBUTE_NAME_IN_LOWER_CASE: u32 = 2;
    pub const ATTRIBUTE_VALUE_IN_LOWER_CASE: u32 = 4;

    pub fn new(flags: u32) -> Self {
        Self {
            flags: Rc::new(Cell::new(flags)),
        }
    }

    pub fn set_flag(&self, flag: u32, on: bool) -> &Self {
        if on && !self.has_flag(flag) {
            self.flags.set(self.flags.get().wrapping_add(flag));
        }
        if !on && self.has_flag(flag) {
            self.flags.set(self.flags.get().wrapping_sub(flag));
        }
        self
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        self.flags.get() & flag != 0
    }

    pub fn translate_selector(
        &self,
        node: &SelectorNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        translator.node_to_xpath(node.get_tree())
    }

    pub fn translate_combined_selector(
        &self,
        node: &CombinedSelectorNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        translator.add_combination(
            node.get_combinator(),
            node.get_selector(),
            node.get_sub_selector(),
        )
    }

    pub fn translate_negation(
        &self,
        node: &NegationNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let xpath = translator.node_to_xpath(node.get_selector())?;
        let mut sub_xpath = translator.node_to_xpath(node.get_sub_selector())?;
        sub_xpath.add_name_test();
        if truthy(sub_xpath.get_condition()) {
            let condition = cat(&[b"not(", sub_xpath.get_condition(), b")"]);
            return Ok(with_condition(xpath, &condition));
        }
        Ok(with_condition(xpath, b"0"))
    }

    /// `:is()` and `:where()`.
    fn translate_matching_or_specificity_adjustment(
        &self,
        selector: &Node,
        arguments: &[Node],
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let mut xpath = translator.node_to_xpath(selector)?;
        let mut conditions = Vec::new();
        for argument in arguments {
            let mut expr = translator.node_to_xpath(argument)?;
            expr.add_name_test();
            if !expr.get_condition().is_empty() {
                conditions.push(expr.get_condition().to_vec());
            }
        }
        if !conditions.is_empty() {
            let condition = if conditions.len() == 1 {
                conditions.remove(0)
            } else {
                cat(&[b"(", &conditions.join(&b") or ("[..]), b")"])
            };
            xpath.and(&condition);
        }
        Ok(xpath)
    }

    pub fn translate_function(
        &self,
        node: &FunctionNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let xpath = translator.node_to_xpath(node.get_selector())?;
        translator.add_function(xpath, node)
    }

    pub fn translate_pseudo(
        &self,
        node: &PseudoNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let xpath = translator.node_to_xpath(node.get_selector())?;
        translator.add_pseudo_class(xpath, node.get_identifier())
    }

    pub fn translate_attribute(
        &self,
        node: &AttributeNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let mut name = node.get_attribute().to_vec();
        let mut safe = is_safe_name(&name);
        if self.has_flag(Self::ATTRIBUTE_NAME_IN_LOWER_CASE) {
            name = strtolower(&name);
        }
        if let Some(namespace) = node.get_namespace().filter(|ns| truthy(ns)) {
            name = cat(&[namespace, b":", &name]);
            safe = safe && is_safe_name(namespace);
        }
        let attribute = if safe {
            cat(&[b"@", &name])
        } else {
            cat(&[
                b"attribute::*[name() = ",
                &Translator::get_xpath_literal(&name),
                b"]",
            ])
        };
        let mut value = node.get_value().map(<[u8]>::to_vec);
        let xpath = translator.node_to_xpath(node.get_selector())?;
        if self.has_flag(Self::ATTRIBUTE_VALUE_IN_LOWER_CASE) {
            // strtolower(null) is "" (with a deprecation notice).
            value = Some(strtolower(value.as_deref().unwrap_or_default()));
        }
        translator.add_attribute_matching(xpath, node.get_operator(), &attribute, value.as_deref())
    }

    pub fn translate_class(
        &self,
        node: &ClassNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let xpath = translator.node_to_xpath(node.get_selector())?;
        translator.add_attribute_matching(xpath, b"~=", b"@class", Some(node.get_name()))
    }

    pub fn translate_hash(
        &self,
        node: &HashNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let xpath = translator.node_to_xpath(node.get_selector())?;
        translator.add_attribute_matching(xpath, b"=", b"@id", Some(node.get_id()))
    }

    pub fn translate_element(&self, node: &ElementNode) -> XPathExpr {
        let mut element = node.get_element().map(<[u8]>::to_vec);
        if let Some(e) = &element
            && truthy(e)
            && self.has_flag(Self::ELEMENT_NAME_IN_LOWER_CASE)
        {
            element = Some(strtolower(e));
        }
        let (mut element, mut safe) = match element {
            Some(e) if truthy(&e) => {
                let safe = is_safe_name(&e);
                (e, safe)
            }
            _ => (b"*".to_vec(), true),
        };
        if let Some(namespace) = node.get_namespace().filter(|ns| truthy(ns)) {
            element = cat(&[namespace, b":", &element]);
            safe = safe && is_safe_name(namespace);
        }
        let mut xpath = XPathExpr::new(b"", &element, b"", false);
        if !safe {
            xpath.add_name_test();
        }
        xpath
    }

    pub fn translate_relation(
        &self,
        node: &RelationNode,
        translator: &Translator,
    ) -> Result<XPathExpr, Exception> {
        let arguments = node.get_arguments();
        if let [(combinator, sub_selector)] = arguments {
            return translator.add_relative_combination(
                combinator,
                node.get_selector(),
                sub_selector,
            );
        }
        let mut conditions = Vec::new();
        let any_element = Node::Element(ElementNode::default());
        for (combinator, sub_selector) in arguments {
            let relative = translator
                .add_relative_combination(combinator, &any_element, sub_selector)?
                .to_php_string();
            // substr($relativeXpath, 2, -1)
            conditions.push(substr(&relative, 2, relative.len().saturating_sub(3)).to_vec());
        }
        let mut xpath = translator.node_to_xpath(node.get_selector())?;
        xpath.and(&cat(&[b"(", &conditions.join(&b") or ("[..]), b")"]));
        Ok(xpath)
    }
}

/// The node translator for `variant`, given the node its name selects.
macro_rules! node_translator {
    ($ext:expr, $variant:ident, $method:ident) => {{
        let ext = $ext.clone();
        let translate: NodeTranslator =
            Rc::new(move |node: &Node, translator: &Translator| match node {
                Node::$variant(n) => ext.$method(n, translator),
                other => Err(Exception::expression(format!(
                    "Node \"{}\" not supported.",
                    other.get_node_name()
                ))),
            });
        entry(stringify!($variant), translate)
    }};
}

impl ExtensionInterface for NodeExtension {
    fn get_node_translators(&self) -> Vec<(Vec<u8>, NodeTranslator)> {
        let matching = {
            let ext = self.clone();
            let translate: NodeTranslator =
                Rc::new(move |node: &Node, translator: &Translator| match node {
                    Node::Matching(n) => ext.translate_matching_or_specificity_adjustment(
                        &n.selector,
                        &n.arguments,
                        translator,
                    ),
                    Node::SpecificityAdjustment(n) => ext
                        .translate_matching_or_specificity_adjustment(
                            &n.selector,
                            &n.arguments,
                            translator,
                        ),
                    other => Err(Exception::expression(format!(
                        "Node \"{}\" not supported.",
                        other.get_node_name()
                    ))),
                });
            translate
        };
        let element = {
            let ext = self.clone();
            let translate: NodeTranslator =
                Rc::new(move |node: &Node, _: &Translator| match node {
                    Node::Element(n) => Ok(ext.translate_element(n)),
                    other => Err(Exception::expression(format!(
                        "Node \"{}\" not supported.",
                        other.get_node_name()
                    ))),
                });
            translate
        };
        vec![
            node_translator!(self, Selector, translate_selector),
            node_translator!(self, CombinedSelector, translate_combined_selector),
            node_translator!(self, Negation, translate_negation),
            entry("Matching", matching.clone()),
            entry("SpecificityAdjustment", matching),
            node_translator!(self, Function, translate_function),
            node_translator!(self, Pseudo, translate_pseudo),
            node_translator!(self, Attribute, translate_attribute),
            node_translator!(self, Class, translate_class),
            node_translator!(self, Hash, translate_hash),
            entry("Element", element),
            node_translator!(self, Relation, translate_relation),
        ]
    }

    fn get_name(&self) -> String {
        "node".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

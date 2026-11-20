//! Port of Symfony\Component\CssSelector\XPath\Translator.php and
//! TranslatorInterface.php: selector nodes to XPath 1.0, through the
//! registered extensions' translators.
//!
//! Upstream: symfony/css-selector XPath/Translator.php XPath/TranslatorInterface.php

pub mod extension;
mod xpath_expr;

use std::collections::HashMap;
use std::rc::Rc;

pub use xpath_expr::XPathExpr;

use crate::exception::Exception;
use crate::node::{FunctionNode, Node, SelectorNode};
use crate::parser::{Parser, ParserInterface};
use crate::php::{cat, strpos, substr};
use extension::{
    AttributeMatchingExtension, AttributeMatchingTranslator, CombinationExtension,
    CombinationTranslator, ExtensionInterface, FunctionExtension, FunctionTranslator,
    NodeExtension, NodeTranslator, PseudoClassExtension, PseudoClassTranslator, RelationExtension,
};

/// `TranslatorInterface`.
pub trait TranslatorInterface {
    fn css_to_xpath(&self, css_expr: &[u8], prefix: &[u8]) -> Result<Vec<u8>, Exception>;

    fn selector_to_xpath(
        &self,
        selector: &SelectorNode,
        prefix: &[u8],
    ) -> Result<Vec<u8>, Exception>;
}

/// The default prefix of [`TranslatorInterface::css_to_xpath`].
pub const DEFAULT_PREFIX: &[u8] = b"descendant-or-self::";

pub struct Translator {
    main_parser: Box<dyn ParserInterface>,
    shortcut_parsers: Vec<Box<dyn ParserInterface>>,
    extensions: HashMap<Vec<u8>, Rc<dyn ExtensionInterface>>,
    node_translators: HashMap<Vec<u8>, NodeTranslator>,
    combination_translators: HashMap<Vec<u8>, CombinationTranslator>,
    relative_combination_translators: HashMap<Vec<u8>, CombinationTranslator>,
    function_translators: HashMap<Vec<u8>, FunctionTranslator>,
    pseudo_class_translators: HashMap<Vec<u8>, PseudoClassTranslator>,
    attribute_matching_translators: HashMap<Vec<u8>, AttributeMatchingTranslator>,
}

impl Default for Translator {
    fn default() -> Self {
        Self::new(None)
    }
}

fn merge<T>(into: &mut HashMap<Vec<u8>, T>, from: Vec<(Vec<u8>, T)>) {
    into.extend(from);
}

impl Translator {
    pub fn new(parser: Option<Box<dyn ParserInterface>>) -> Self {
        let mut translator = Self {
            main_parser: parser.unwrap_or_else(|| Box::new(Parser::default())),
            shortcut_parsers: Vec::new(),
            extensions: HashMap::new(),
            node_translators: HashMap::new(),
            combination_translators: HashMap::new(),
            relative_combination_translators: HashMap::new(),
            function_translators: HashMap::new(),
            pseudo_class_translators: HashMap::new(),
            attribute_matching_translators: HashMap::new(),
        };
        translator
            .register_extension(NodeExtension::new(0))
            .register_extension(CombinationExtension)
            .register_extension(FunctionExtension)
            .register_extension(PseudoClassExtension)
            .register_extension(AttributeMatchingExtension)
            .register_extension(RelationExtension);
        translator
    }

    /// `'value'`, `"value"`, or a `concat()` when it has both quotes.
    pub fn get_xpath_literal(element: &[u8]) -> Vec<u8> {
        if !element.contains(&b'\'') {
            return cat(&[b"'", element, b"'"]);
        }
        if !element.contains(&b'"') {
            return cat(&[b"\"", element, b"\""]);
        }
        let mut string = element;
        let mut parts: Vec<Vec<u8>> = Vec::new();
        loop {
            if let Some(pos) = strpos(string, b"'", 0) {
                parts.push(cat(&[b"'", substr(string, 0, pos), b"'"]));
                parts.push(b"\"'\"".to_vec());
                string = &string[pos + 1..];
            } else {
                parts.push(cat(&[b"'", string, b"'"]));
                break;
            }
        }
        cat(&[b"concat(", &parts.join(&b", "[..]), b")"])
    }

    pub fn register_extension(
        &mut self,
        extension: impl ExtensionInterface + 'static,
    ) -> &mut Self {
        let extension: Rc<dyn ExtensionInterface> = Rc::new(extension);
        merge(&mut self.node_translators, extension.get_node_translators());
        merge(
            &mut self.combination_translators,
            extension.get_combination_translators(),
        );
        merge(
            &mut self.function_translators,
            extension.get_function_translators(),
        );
        merge(
            &mut self.pseudo_class_translators,
            extension.get_pseudo_class_translators(),
        );
        merge(
            &mut self.attribute_matching_translators,
            extension.get_attribute_matching_translators(),
        );
        merge(
            &mut self.relative_combination_translators,
            extension.get_relative_combination_translators(),
        );
        self.extensions
            .insert(extension.get_name().into_bytes(), extension);
        self
    }

    pub fn get_extension(&self, name: &str) -> Result<&dyn ExtensionInterface, Exception> {
        match self.extensions.get(name.as_bytes()) {
            Some(extension) => Ok(extension.as_ref()),
            None => Err(Exception::expression(format!(
                "Extension \"{name}\" not registered."
            ))),
        }
    }

    pub fn register_parser_shortcut(
        &mut self,
        shortcut: impl ParserInterface + 'static,
    ) -> &mut Self {
        self.shortcut_parsers.push(Box::new(shortcut));
        self
    }

    pub fn node_to_xpath(&self, node: &Node) -> Result<XPathExpr, Exception> {
        let name = node.get_node_name();
        match self.node_translators.get(name.as_bytes()) {
            Some(translate) => translate(node, self),
            None => Err(Exception::expression(format!(
                "Node \"{name}\" not supported."
            ))),
        }
    }

    pub fn add_combination(
        &self,
        combiner: &[u8],
        xpath: &Node,
        combined_xpath: &Node,
    ) -> Result<XPathExpr, Exception> {
        match self.combination_translators.get(combiner) {
            Some(translate) => translate(
                self.node_to_xpath(xpath)?,
                self.node_to_xpath(combined_xpath)?,
            ),
            None => Err(Exception::expression(cat(&[
                b"Combiner \"",
                combiner,
                b"\" not supported.",
            ]))),
        }
    }

    pub fn add_relative_combination(
        &self,
        combiner: &[u8],
        xpath: &Node,
        combined_xpath: &Node,
    ) -> Result<XPathExpr, Exception> {
        match self.relative_combination_translators.get(combiner) {
            Some(translate) => translate(
                self.node_to_xpath(xpath)?,
                self.node_to_xpath(combined_xpath)?,
            ),
            None => Err(Exception::expression(cat(&[
                b"Combiner \"",
                combiner,
                b"\" not supported.",
            ]))),
        }
    }

    pub fn add_function(
        &self,
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        match self.function_translators.get(function.get_name()) {
            Some(translate) => translate(xpath, function),
            None => Err(Exception::expression(cat(&[
                b"Function \"",
                function.get_name(),
                b"\" not supported.",
            ]))),
        }
    }

    pub fn add_pseudo_class(
        &self,
        xpath: XPathExpr,
        pseudo_class: &[u8],
    ) -> Result<XPathExpr, Exception> {
        match self.pseudo_class_translators.get(pseudo_class) {
            Some(translate) => translate(xpath),
            None => Err(Exception::expression(cat(&[
                b"Pseudo-class \"",
                pseudo_class,
                b"\" not supported.",
            ]))),
        }
    }

    pub fn add_attribute_matching(
        &self,
        xpath: XPathExpr,
        operator: &[u8],
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> Result<XPathExpr, Exception> {
        match self.attribute_matching_translators.get(operator) {
            Some(translate) => translate(xpath, attribute, value),
            None => Err(Exception::expression(cat(&[
                b"Attribute matcher operator \"",
                operator,
                b"\" not supported.",
            ]))),
        }
    }

    fn parse_selectors(&self, css: &[u8]) -> Result<Vec<SelectorNode>, Exception> {
        for shortcut in &self.shortcut_parsers {
            let tokens = shortcut.parse(css)?;
            if !tokens.is_empty() {
                return Ok(tokens);
            }
        }
        self.main_parser.parse(css)
    }
}

impl TranslatorInterface for Translator {
    fn css_to_xpath(&self, css_expr: &[u8], prefix: &[u8]) -> Result<Vec<u8>, Exception> {
        let selectors = self.parse_selectors(css_expr)?;
        let mut xpaths = Vec::with_capacity(selectors.len());
        for selector in &selectors {
            if selector.get_pseudo_element().is_some() {
                return Err(Exception::expression("Pseudo-elements are not supported."));
            }
            xpaths.push(self.selector_to_xpath(selector, prefix)?);
        }
        Ok(xpaths.join(&b" | "[..]))
    }

    fn selector_to_xpath(
        &self,
        selector: &SelectorNode,
        prefix: &[u8],
    ) -> Result<Vec<u8>, Exception> {
        let xpath = self.node_to_xpath(&Node::Selector(selector.clone()))?;
        Ok(cat(&[prefix, &xpath.to_php_string()]))
    }
}

//! Port of Symfony\Component\CssSelector\XPath\Extension\HtmlExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/HtmlExtension.php

use std::any::Any;
use std::rc::Rc;

use super::function::single_string_argument;
use super::{
    ExtensionInterface, FunctionTranslator, NodeExtension, PseudoClassTranslator, entry,
    with_condition,
};
use crate::exception::{Exception, Kind};
use crate::node::FunctionNode;
use crate::php::{cat, strtolower};
use crate::xpath::{Translator, XPathExpr};

// Each disabled fieldset ancestor disables the element, except the one whose first legend
// child the element sits in. Those excepted fieldsets are exactly the parents of the
// ancestors-or-self that are a first legend child of a disabled fieldset, one for one, so
// the element is disabled as soon as the first count exceeds the second one. Counting keeps
// this to two cheap ancestor walks instead of a predicate run on every ancestor.
const DISABLING_FIELDSET: &str = "count(ancestor::fieldset[@disabled]) > count(ancestor-or-self::legend[not(preceding-sibling::legend)][parent::fieldset[@disabled]])";

/// HTML's pseudo-classes and `:lang()`; registering it lowercases element
/// and attribute names (through the translator's node extension).
#[derive(Debug, Clone, Copy)]
pub struct HtmlExtension;

impl HtmlExtension {
    pub fn new(translator: &Translator) -> Result<Self, Exception> {
        let extension = translator.get_extension("node")?;
        match extension.as_any().downcast_ref::<NodeExtension>() {
            Some(node) => {
                node.set_flag(NodeExtension::ELEMENT_NAME_IN_LOWER_CASE, true)
                    .set_flag(NodeExtension::ATTRIBUTE_NAME_IN_LOWER_CASE, true);
                Ok(Self)
            }
            None => Err(Exception::new(
                Kind::Error,
                "Call to undefined method setFlag() on the \"node\" extension",
            )),
        }
    }

    pub fn translate_checked(xpath: XPathExpr) -> XPathExpr {
        with_condition(
            xpath,
            b"(@checked and (name(.) = 'input' or name(.) = 'command')and (@type = 'checkbox' or @type = 'radio'))",
        )
    }

    pub fn translate_link(xpath: XPathExpr) -> XPathExpr {
        with_condition(
            xpath,
            b"@href and (name(.) = 'a' or name(.) = 'link' or name(.) = 'area')",
        )
    }

    pub fn translate_disabled(xpath: XPathExpr) -> XPathExpr {
        let condition = cat(&[
            b"(@disabled and(",
            b"(name(.) = 'input' and @type != 'hidden')",
            b" or name(.) = 'button'",
            b" or name(.) = 'select'",
            b" or name(.) = 'textarea'",
            b" or name(.) = 'command'",
            b" or name(.) = 'fieldset'",
            b" or name(.) = 'optgroup'",
            b" or name(.) = 'option'",
            b")) or (",
            b"(name(.) = 'input' and @type != 'hidden')",
            b" or name(.) = 'button'",
            b" or name(.) = 'select'",
            b" or name(.) = 'textarea'",
            b" or name(.) = 'fieldset'",
            b") and ",
            DISABLING_FIELDSET.as_bytes(),
        ]);
        with_condition(xpath, &condition)
    }

    pub fn translate_enabled(xpath: XPathExpr) -> XPathExpr {
        let condition = cat(&[
            b"(@href and (",
            b"name(.) = 'a'",
            b" or name(.) = 'link'",
            b" or name(.) = 'area'",
            b")) or ((",
            b"name(.) = 'command'",
            b" or name(.) = 'optgroup'",
            b") and not(@disabled)) or (((",
            b"name(.) = 'input' and @type != 'hidden')",
            b" or name(.) = 'button'",
            b" or name(.) = 'select'",
            b" or name(.) = 'textarea'",
            b" or name(.) = 'keygen'",
            b" or name(.) = 'fieldset'",
            b") and not(@disabled or ",
            DISABLING_FIELDSET.as_bytes(),
            b")) or (name(.) = 'option' and not(",
            b"@disabled or ancestor::optgroup[@disabled]",
            b"))",
        ]);
        with_condition(xpath, &condition)
    }

    pub fn translate_lang(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        let value = single_string_argument(function, "lang")?;
        let condition = cat(&[
            b"ancestor-or-self::*[@lang][1][starts-with(concat(",
            b"translate(@lang, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz'), '-')",
            b", ",
            &Translator::get_xpath_literal(&cat(&[&strtolower(&value), b"-"])),
            b")]",
        ]);
        Ok(with_condition(xpath, &condition))
    }

    pub fn translate_selected(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"(@selected and name(.) = 'option')")
    }

    pub fn translate_invalid(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"0")
    }

    pub fn translate_hover(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"0")
    }

    pub fn translate_visited(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"0")
    }
}

impl ExtensionInterface for HtmlExtension {
    fn get_pseudo_class_translators(&self) -> Vec<(Vec<u8>, PseudoClassTranslator)> {
        vec![
            entry("checked", Rc::new(|x| Ok(Self::translate_checked(x)))),
            entry("link", Rc::new(|x| Ok(Self::translate_link(x)))),
            entry("disabled", Rc::new(|x| Ok(Self::translate_disabled(x)))),
            entry("enabled", Rc::new(|x| Ok(Self::translate_enabled(x)))),
            entry("selected", Rc::new(|x| Ok(Self::translate_selected(x)))),
            entry("invalid", Rc::new(|x| Ok(Self::translate_invalid(x)))),
            entry("hover", Rc::new(|x| Ok(Self::translate_hover(x)))),
            entry("visited", Rc::new(|x| Ok(Self::translate_visited(x)))),
        ]
    }

    fn get_function_translators(&self) -> Vec<(Vec<u8>, FunctionTranslator)> {
        vec![entry("lang", Rc::new(Self::translate_lang))]
    }

    fn get_name(&self) -> String {
        "html".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

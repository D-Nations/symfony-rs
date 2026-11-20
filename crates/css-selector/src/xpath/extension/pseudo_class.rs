//! Port of Symfony\Component\CssSelector\XPath\Extension\PseudoClassExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/PseudoClassExtension.php

use std::any::Any;
use std::rc::Rc;

use super::{ExtensionInterface, PseudoClassTranslator, entry, with_condition};
use crate::exception::Exception;
use crate::php::cat;
use crate::xpath::XPathExpr;

#[derive(Debug, Clone, Copy, Default)]
pub struct PseudoClassExtension;

impl PseudoClassExtension {
    pub fn translate_root(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"not(parent::*)")
    }

    pub fn translate_scope_pseudo(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"1")
    }

    pub fn translate_first_child(mut xpath: XPathExpr) -> XPathExpr {
        xpath
            .add_star_prefix()
            .add_name_test()
            .and(b"position() = 1");
        xpath
    }

    pub fn translate_last_child(mut xpath: XPathExpr) -> XPathExpr {
        xpath
            .add_star_prefix()
            .add_name_test()
            .and(b"position() = last()");
        xpath
    }

    pub fn translate_first_of_type(mut xpath: XPathExpr) -> Result<XPathExpr, Exception> {
        if xpath.get_element() == b"*" {
            return Err(Exception::expression(
                "\"*:first-of-type\" is not implemented.",
            ));
        }
        xpath.add_star_prefix().and(b"position() = 1");
        Ok(xpath)
    }

    pub fn translate_last_of_type(mut xpath: XPathExpr) -> Result<XPathExpr, Exception> {
        if xpath.get_element() == b"*" {
            return Err(Exception::expression(
                "\"*:last-of-type\" is not implemented.",
            ));
        }
        xpath.add_star_prefix().and(b"position() = last()");
        Ok(xpath)
    }

    pub fn translate_only_child(mut xpath: XPathExpr) -> XPathExpr {
        xpath.add_star_prefix().add_name_test().and(b"last() = 1");
        xpath
    }

    pub fn translate_only_of_type(xpath: XPathExpr) -> XPathExpr {
        let element = xpath.get_element().to_vec();
        let condition = cat(&[
            b"count(preceding-sibling::",
            &element,
            b")=0 and count(following-sibling::",
            &element,
            b")=0",
        ]);
        with_condition(xpath, &condition)
    }

    pub fn translate_empty(xpath: XPathExpr) -> XPathExpr {
        with_condition(xpath, b"not(*) and not(string-length())")
    }
}

impl ExtensionInterface for PseudoClassExtension {
    fn get_pseudo_class_translators(&self) -> Vec<(Vec<u8>, PseudoClassTranslator)> {
        vec![
            entry("root", Rc::new(|x| Ok(Self::translate_root(x)))),
            entry("scope", Rc::new(|x| Ok(Self::translate_scope_pseudo(x)))),
            entry(
                "first-child",
                Rc::new(|x| Ok(Self::translate_first_child(x))),
            ),
            entry("last-child", Rc::new(|x| Ok(Self::translate_last_child(x)))),
            entry("first-of-type", Rc::new(Self::translate_first_of_type)),
            entry("last-of-type", Rc::new(Self::translate_last_of_type)),
            entry("only-child", Rc::new(|x| Ok(Self::translate_only_child(x)))),
            entry(
                "only-of-type",
                Rc::new(|x| Ok(Self::translate_only_of_type(x))),
            ),
            entry("empty", Rc::new(|x| Ok(Self::translate_empty(x)))),
        ]
    }

    fn get_name(&self) -> String {
        "pseudo-class".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

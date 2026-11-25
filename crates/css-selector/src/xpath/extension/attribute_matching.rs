//! Port of Symfony\Component\CssSelector\XPath\Extension\AttributeMatchingExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/AttributeMatchingExtension.php

use std::any::Any;
use std::rc::Rc;

use super::{AttributeMatchingTranslator, ExtensionInterface, entry, with_condition};
use crate::exception::{Exception, Kind};
use crate::php::{cat, truthy_opt};
use crate::xpath::{Translator, XPathExpr};

#[derive(Debug, Clone, Copy, Default)]
pub struct AttributeMatchingExtension;

/// `Translator::getXpathLiteral($value)`, which refuses PHP's `null`.
fn literal(value: Option<&[u8]>) -> Result<Vec<u8>, Exception> {
    match value {
        Some(value) => Ok(Translator::get_xpath_literal(value)),
        None => Err(Exception::new(
            Kind::TypeError,
            "Symfony\\Component\\CssSelector\\XPath\\Translator::getXpathLiteral(): Argument #1 ($element) must be of type string, null given",
        )),
    }
}

impl AttributeMatchingExtension {
    pub fn translate_exists(
        xpath: XPathExpr,
        attribute: &[u8],
        _value: Option<&[u8]>,
    ) -> XPathExpr {
        with_condition(xpath, attribute)
    }

    pub fn translate_equals(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> Result<XPathExpr, Exception> {
        Ok(with_condition(
            xpath,
            &cat(&[attribute, b" = ", &literal(value)?]),
        ))
    }

    pub fn translate_includes(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> XPathExpr {
        let condition = match value {
            Some(v) if truthy_opt(value) => cat(&[
                attribute,
                b" and contains(concat(' ', normalize-space(",
                attribute,
                b"), ' '), ",
                &Translator::get_xpath_literal(&cat(&[b" ", v, b" "])),
                b")",
            ]),
            _ => b"0".to_vec(),
        };
        with_condition(xpath, &condition)
    }

    pub fn translate_dash_match(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> Result<XPathExpr, Exception> {
        let dashed = cat(&[value.unwrap_or_default(), b"-"]);
        let condition = cat(&[
            attribute,
            b" and (",
            attribute,
            b" = ",
            &literal(value)?,
            b" or starts-with(",
            attribute,
            b", ",
            &Translator::get_xpath_literal(&dashed),
            b"))",
        ]);
        Ok(with_condition(xpath, &condition))
    }

    pub fn translate_prefix_match(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> XPathExpr {
        let condition = match value {
            Some(v) if truthy_opt(value) => cat(&[
                attribute,
                b" and starts-with(",
                attribute,
                b", ",
                &Translator::get_xpath_literal(v),
                b")",
            ]),
            _ => b"0".to_vec(),
        };
        with_condition(xpath, &condition)
    }

    pub fn translate_suffix_match(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> XPathExpr {
        let condition = match value {
            Some(v) if truthy_opt(value) => cat(&[
                attribute,
                b" and substring(",
                attribute,
                b", string-length(",
                attribute,
                b")-",
                (i64::try_from(v.len()).unwrap_or(i64::MAX) - 1)
                    .to_string()
                    .as_bytes(),
                b") = ",
                &Translator::get_xpath_literal(v),
            ]),
            _ => b"0".to_vec(),
        };
        with_condition(xpath, &condition)
    }

    pub fn translate_substring_match(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> XPathExpr {
        let condition = match value {
            Some(v) if truthy_opt(value) => cat(&[
                attribute,
                b" and contains(",
                attribute,
                b", ",
                &Translator::get_xpath_literal(v),
                b")",
            ]),
            _ => b"0".to_vec(),
        };
        with_condition(xpath, &condition)
    }

    pub fn translate_different(
        xpath: XPathExpr,
        attribute: &[u8],
        value: Option<&[u8]>,
    ) -> Result<XPathExpr, Exception> {
        let literal = literal(value)?;
        let condition = if truthy_opt(value) {
            cat(&[b"not(", attribute, b") or ", attribute, b" != ", &literal])
        } else {
            cat(&[attribute, b" != ", &literal])
        };
        Ok(with_condition(xpath, &condition))
    }
}

impl ExtensionInterface for AttributeMatchingExtension {
    fn get_attribute_matching_translators(&self) -> Vec<(Vec<u8>, AttributeMatchingTranslator)> {
        vec![
            entry(
                "exists",
                Rc::new(|x, a, v| Ok(Self::translate_exists(x, a, v))),
            ),
            entry("=", Rc::new(Self::translate_equals)),
            entry(
                "~=",
                Rc::new(|x, a, v| Ok(Self::translate_includes(x, a, v))),
            ),
            entry("|=", Rc::new(Self::translate_dash_match)),
            entry(
                "^=",
                Rc::new(|x, a, v| Ok(Self::translate_prefix_match(x, a, v))),
            ),
            entry(
                "$=",
                Rc::new(|x, a, v| Ok(Self::translate_suffix_match(x, a, v))),
            ),
            entry(
                "*=",
                Rc::new(|x, a, v| Ok(Self::translate_substring_match(x, a, v))),
            ),
            entry("!=", Rc::new(Self::translate_different)),
        ]
    }

    fn get_name(&self) -> String {
        "attribute-matching".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

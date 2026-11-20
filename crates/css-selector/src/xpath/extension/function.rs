//! Port of Symfony\Component\CssSelector\XPath\Extension\FunctionExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/FunctionExtension.php

use std::any::Any;
use std::rc::Rc;

use super::{ExtensionInterface, FunctionTranslator, entry, with_condition};
use crate::exception::{Exception, Kind};
use crate::node::FunctionNode;
use crate::parser::{Parser, Token};
use crate::php::{cat, int_string};
use crate::xpath::{Translator, XPathExpr};

#[derive(Debug, Clone, Copy, Default)]
pub struct FunctionExtension;

/// `implode($glue, $tokens)`, through `Token::__toString()`.
pub(crate) fn implode_tokens(glue: &[u8], tokens: &[Token]) -> Vec<u8> {
    let parts: Vec<Vec<u8>> = tokens.iter().map(Token::to_php_string).collect();
    parts.join(glue)
}

/// The first argument's value, after checking every argument is a string
/// or an identifier (`:contains()`, `:lang()`).
pub(crate) fn single_string_argument(
    function: &FunctionNode,
    name: &str,
) -> Result<Vec<u8>, Exception> {
    let arguments = function.get_arguments();
    if arguments
        .iter()
        .any(|t| !(t.is_string() || t.is_identifier()))
    {
        return Err(Exception::expression(cat(&[
            b"Expected a single string or identifier for :",
            name.as_bytes(),
            b"(), got ",
            &implode_tokens(b", ", arguments),
        ])));
    }
    match arguments.first() {
        Some(token) => Ok(token.get_value().unwrap_or_default().to_vec()),
        // `$arguments[0]` on an empty list is null upstream.
        None => Err(Exception::new(
            Kind::Error,
            "Call to a member function getValue() on null",
        )),
    }
}

impl FunctionExtension {
    pub fn translate_nth_child(
        mut xpath: XPathExpr,
        function: &FunctionNode,
        last: bool,
        add_name_test: bool,
    ) -> Result<XPathExpr, Exception> {
        let (a, b) = Parser::parse_series(function.get_arguments()).map_err(|_| {
            Exception::expression(cat(&[
                b"Invalid series: \"",
                &implode_tokens(b"\", \"", function.get_arguments()),
                b"\".",
            ]))
        })?;
        // PHP's int arithmetic turns to float past the int range.
        let mut b = i128::from(b);

        xpath.add_star_prefix();
        if add_name_test {
            xpath.add_name_test();
        }

        if a == 0 {
            let position = if last {
                cat(&[b"last() - ", &int_string(b - 1)])
            } else {
                int_string(b)
            };
            return Ok(with_condition(xpath, &cat(&[b"position() = ", &position])));
        }

        let sign: &[u8] = if a < 0 {
            if b < 1 {
                return Ok(with_condition(xpath, b"false()"));
            }
            b"<="
        } else {
            b">="
        };

        let mut expr = b"position()".to_vec();
        if last {
            expr = cat(&[b"last() - ", &expr]);
            b -= 1;
        }
        if b != 0 {
            expr = cat(&[&expr, b" - ", &int_string(b)]);
        }

        let mut conditions = vec![cat(&[&expr, b" ", sign, b" 0"])];
        if a != 1 && a != -1 {
            conditions.push(cat(&[
                b"(",
                &expr,
                b") mod ",
                a.to_string().as_bytes(),
                b" = 0",
            ]));
        }
        Ok(with_condition(xpath, &conditions.join(&b" and "[..])))
    }

    pub fn translate_nth_last_child(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        Self::translate_nth_child(xpath, function, true, true)
    }

    pub fn translate_nth_of_type(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        Self::translate_nth_child(xpath, function, false, false)
    }

    pub fn translate_nth_last_of_type(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        if xpath.get_element() == b"*" {
            return Err(Exception::expression(
                "\"*:nth-of-type()\" is not implemented.",
            ));
        }
        Self::translate_nth_child(xpath, function, true, false)
    }

    pub fn translate_contains(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        let value = single_string_argument(function, "contains")?;
        Ok(with_condition(
            xpath,
            &cat(&[
                b"contains(string(.), ",
                &Translator::get_xpath_literal(&value),
                b")",
            ]),
        ))
    }

    pub fn translate_lang(
        xpath: XPathExpr,
        function: &FunctionNode,
    ) -> Result<XPathExpr, Exception> {
        let value = single_string_argument(function, "lang")?;
        Ok(with_condition(
            xpath,
            &cat(&[b"lang(", &Translator::get_xpath_literal(&value), b")"]),
        ))
    }
}

impl ExtensionInterface for FunctionExtension {
    fn get_function_translators(&self) -> Vec<(Vec<u8>, FunctionTranslator)> {
        vec![
            entry(
                "nth-child",
                Rc::new(|x, f| Self::translate_nth_child(x, f, false, true)),
            ),
            entry("nth-last-child", Rc::new(Self::translate_nth_last_child)),
            entry("nth-of-type", Rc::new(Self::translate_nth_of_type)),
            entry(
                "nth-last-of-type",
                Rc::new(Self::translate_nth_last_of_type),
            ),
            entry("contains", Rc::new(Self::translate_contains)),
            entry("lang", Rc::new(Self::translate_lang)),
        ]
    }

    fn get_name(&self) -> String {
        "function".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

//! TranslatorTest.php at symfony/css-selector v8.1.6, on upstream's
//! provider rows (tests/data/php.json).
//!
//! testXmlLang, testHtmlIds, testHtmlShakespear,
//! testOnlyOfTypeFindsSingleChildrenOfGivenType and
//! testDisabledAndEnabledInsideFieldsets evaluate the XPath on a document
//! (DOMDocument, SimpleXMLElement), which needs an XPath engine; they are
//! `n/a` in the manifest (PLAN.md decision 9). What this crate produces
//! for them, the XPath string, is checked against the one PHP generated
//! for every one of their rows in `dom_rows_translate_as_php_did`, and the
//! recording keeps what PHP's DOM selected for when an engine exists.
//!
//! Upstream: symfony/css-selector Tests/XPath/TranslatorTest.php

use symfony_css_selector::node::{ElementNode, FunctionNode};
use symfony_css_selector::parser::{Parser, ParserInterface};
use symfony_css_selector::xpath::extension::HtmlExtension;
use symfony_css_selector::xpath::{DEFAULT_PREFIX, Translator, TranslatorInterface, XPathExpr};
use symfony_css_selector::{Exception, Kind};

use crate::common::{data, decode, provider, show};

const CLASS: &str = "XPath/TranslatorTest";

fn html_translator() -> Translator {
    let mut translator = Translator::default();
    let html = HtmlExtension::new(&translator).unwrap();
    translator.register_extension(html);
    translator
}

fn expression_error<T: std::fmt::Debug>(result: Result<T, Exception>) {
    assert_eq!(Kind::ExpressionError, result.unwrap_err().kind());
}

#[test]
fn testXpathLiteral() {
    for (name, args) in provider(CLASS, "testXpathLiteral") {
        assert_eq!(
            show(&args[1].bytes()),
            show(&Translator::get_xpath_literal(&args[0].bytes())),
            "row {name}"
        );
    }
}

#[test]
fn testCssToXPath() {
    for (name, args) in provider(CLASS, "testCssToXPath") {
        let translator = html_translator();
        assert_eq!(
            show(&args[1].bytes()),
            show(&translator.css_to_xpath(&args[0].bytes(), b"").unwrap()),
            "row {name}"
        );
    }
}

#[test]
fn testHasUnsupportedSelector() {
    for (name, args) in provider(CLASS, "testHasUnsupportedSelector") {
        let translator = html_translator();
        let error = translator.css_to_xpath(&args[0].bytes(), b"").unwrap_err();
        assert_eq!(Kind::SyntaxError, error.kind(), "row {name}");
    }
}

#[test]
fn testCssToXPathPseudoElement() {
    let translator = html_translator();
    expression_error(translator.css_to_xpath(b"e::first-line", DEFAULT_PREFIX));
}

#[test]
fn testGetExtensionNotExistsExtension() {
    let translator = html_translator();
    expression_error(translator.get_extension("fake").map(|_| ()));
}

#[test]
fn testAddCombinationNotExistsExtension() {
    let translator = html_translator();
    let parser = Parser::default();
    let xpath = parser.parse(b"*").unwrap().remove(0);
    let combined_xpath = parser.parse(b"*").unwrap().remove(0);

    expression_error(translator.add_combination(b"fake", &xpath.into(), &combined_xpath.into()));
}

#[test]
fn testAddFunctionNotExistsFunction() {
    let translator = html_translator();
    let xpath = XPathExpr::default();
    let function = FunctionNode::new(ElementNode::default(), b"fake", Vec::new());

    expression_error(translator.add_function(xpath, &function));
}

#[test]
fn testAddPseudoClassNotExistsClass() {
    let translator = html_translator();
    let xpath = XPathExpr::default();

    expression_error(translator.add_pseudo_class(xpath, b"fake"));
}

#[test]
fn testAddAttributeMatchingClassNotExistsClass() {
    let translator = html_translator();
    let xpath = XPathExpr::default();

    expression_error(translator.add_attribute_matching(xpath, b"", b"", Some(b"")));
}

/// The XPath each DOM test row evaluates, as PHP generated it: testXmlLang
/// without the HTML extension, the others with it, all with the default
/// prefix.
#[test]
fn dom_rows_translate_as_php_did() {
    let dom = data()["dom"].as_object().expect("dom rows");
    let mut rows = 0;
    for (test, cases) in dom {
        for case in cases.as_array().expect("rows") {
            let css = decode(&case[0]).bytes();
            let expected = decode(&case[1]).bytes();
            let translator = if test == "testXmlLang" {
                Translator::default()
            } else {
                html_translator()
            };
            let actual = translator.css_to_xpath(&css, DEFAULT_PREFIX).unwrap();
            assert_eq!(show(&expected), show(&actual), "{test} {}", show(&css));
            rows += 1;
        }
    }
    assert_eq!(9 + 129 + 50 + 1 + 2, rows);
}

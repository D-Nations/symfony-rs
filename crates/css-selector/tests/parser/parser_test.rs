//! ParserTest.php at symfony/css-selector v8.1.6, on upstream's provider
//! rows (tests/data/php.json). Upstream sets and reads the nesting depths
//! by reflection; the parser's depth accessors stand in.
//!
//! Upstream: symfony/css-selector Tests/Parser/ParserTest.php

use symfony_css_selector::node::Node;
use symfony_css_selector::parser::{Parser, ParserInterface};
use symfony_css_selector::{Exception, Kind};

use crate::common::{provider, show};

const CLASS: &str = "Parser/ParserTest";

fn syntax_error(result: Result<impl std::fmt::Debug, Exception>, row: &str) -> Exception {
    let error = result.expect_err(row);
    assert_eq!(Kind::SyntaxError, error.kind(), "row {row}");
    error
}

#[test]
fn testParser() {
    for (name, args) in provider(CLASS, "testParser") {
        let parser = Parser::default();
        let expected: Vec<String> = args[1].list().iter().map(|v| show(&v.bytes())).collect();
        let actual: Vec<String> = parser
            .parse(&args[0].bytes())
            .unwrap_or_else(|e| panic!("row {name}: {e}"))
            .iter()
            .map(|node| show(&node.get_tree().to_php_string()))
            .collect();
        assert_eq!(expected, actual, "row {name}");
    }
}

#[test]
fn testParserException() {
    for (name, args) in provider(CLASS, "testParserException") {
        let parser = Parser::default();
        let error = syntax_error(parser.parse(&args[0].bytes()), &name);
        assert_eq!(show(&args[1].bytes()), show(error.message()), "row {name}");
    }
}

#[test]
fn testPseudoElements() {
    for (name, args) in provider(CLASS, "testPseudoElements") {
        let parser = Parser::default();
        let selectors = parser.parse(&args[0].bytes()).unwrap();
        assert_eq!(1, selectors.len(), "row {name}");

        let selector = &selectors[0];
        assert_eq!(
            show(&args[1].bytes()),
            show(&selector.get_tree().to_php_string()),
            "row {name}"
        );
        assert_eq!(
            show(&args[2].bytes()),
            show(selector.get_pseudo_element().unwrap_or_default()),
            "row {name}"
        );
    }
}

#[test]
fn testSpecificity() {
    for (name, args) in provider(CLASS, "testSpecificity") {
        let parser = Parser::default();
        let selectors = parser.parse(&args[0].bytes()).unwrap();
        assert_eq!(1, selectors.len(), "row {name}");
        assert_eq!(
            args[1].int(),
            selectors[0].get_specificity().get_value(),
            "row {name}"
        );
    }
}

fn series_function(series: &[u8]) -> Node {
    let parser = Parser::default();
    let source = [&b":nth-child("[..], series, b")"].concat();
    let selectors = parser.parse(&source).unwrap();
    assert_eq!(1, selectors.len());
    selectors[0].get_tree().clone()
}

#[test]
fn testParseSeries() {
    for (name, args) in provider(CLASS, "testParseSeries") {
        let Node::Function(function) = series_function(&args[0].bytes()) else {
            panic!("row {name}: not a function");
        };
        assert_eq!(
            Ok((args[1].int(), args[2].int())),
            Parser::parse_series(function.get_arguments()),
            "row {name}"
        );
    }
}

#[test]
fn testParseSeriesException() {
    for (name, args) in provider(CLASS, "testParseSeriesException") {
        let Node::Function(function) = series_function(&args[0].bytes()) else {
            panic!("row {name}: not a function");
        };
        syntax_error(Parser::parse_series(function.get_arguments()), &name);
    }
}

#[test]
fn testHasNestingDepthLimit() {
    let parser = Parser::default();
    parser.set_has_nesting_depth(16);

    let error = syntax_error(parser.parse(b":has(a)"), "");
    assert_eq!(Exception::nested_has().message(), error.message());
}

#[test]
fn testHasNestingWithinLimitIsAccepted() {
    let parser = Parser::default();
    parser.set_has_nesting_depth(15);

    assert_eq!(1, parser.parse(b":has(a)").unwrap().len());
    assert_eq!(15, parser.has_nesting_depth());
}

#[test]
fn testMatchingNestingDepthLimit() {
    for (name, args) in provider(CLASS, "testMatchingNestingDepthLimit") {
        let parser = Parser::default();
        parser.set_nesting_depth(usize::try_from(args[0].int()).unwrap_or_default());

        let error = syntax_error(parser.parse(&args[1].bytes()), &name);
        assert_eq!(show(&args[2].bytes()), show(error.message()), "row {name}");
    }
}

#[test]
fn testMatchingNestingWithinLimitIsAccepted() {
    let parser = Parser::default();
    parser.set_nesting_depth(15);

    assert_eq!(1, parser.parse(b":is(:where)").unwrap().len());
    assert_eq!(15, parser.nesting_depth());
}

#[test]
fn testDeeplyNestedSelectorsAreRejected() {
    for (name, args) in provider(CLASS, "testDeeplyNestedSelectorsAreRejected") {
        syntax_error(Parser::default().parse(&args[0].bytes()), &name);
    }
}

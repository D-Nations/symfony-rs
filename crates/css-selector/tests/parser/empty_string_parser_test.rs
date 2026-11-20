//! EmptyStringParserTest.php at symfony/css-selector v8.1.6.
//!
//! Upstream: symfony/css-selector Tests/Parser/Shortcut/EmptyStringParserTest.php

use symfony_css_selector::parser::ParserInterface;
use symfony_css_selector::parser::shortcut::EmptyStringParser;

#[test]
fn testParse() {
    let parser = EmptyStringParser;
    let selectors = parser.parse(b"").unwrap();
    assert_eq!(1, selectors.len());

    let selector = &selectors[0];
    assert_eq!(b"Element[*]".to_vec(), selector.get_tree().to_php_string());

    let selectors = parser.parse(b"this will produce an empty array").unwrap();
    assert_eq!(0, selectors.len());
}

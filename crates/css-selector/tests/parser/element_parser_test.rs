//! ElementParserTest.php at symfony/css-selector v8.1.6, on upstream's
//! provider rows.
//!
//! Upstream: symfony/css-selector Tests/Parser/Shortcut/ElementParserTest.php

use symfony_css_selector::parser::shortcut::ElementParser;

use crate::common::provider;

#[test]
fn testParse() {
    crate::shortcut_parse(
        &ElementParser,
        &provider("Parser/Shortcut/ElementParserTest", "testParse"),
    );
}

//! ClassParserTest.php at symfony/css-selector v8.1.6, on upstream's
//! provider rows.
//!
//! Upstream: symfony/css-selector Tests/Parser/Shortcut/ClassParserTest.php

use symfony_css_selector::parser::shortcut::ClassParser;

use crate::common::provider;

#[test]
fn testParse() {
    crate::shortcut_parse(
        &ClassParser,
        &provider("Parser/Shortcut/ClassParserTest", "testParse"),
    );
}

//! HashParserTest.php at symfony/css-selector v8.1.6, on upstream's
//! provider rows.
//!
//! Upstream: symfony/css-selector Tests/Parser/Shortcut/HashParserTest.php

use symfony_css_selector::parser::shortcut::HashParser;

use crate::common::provider;

#[test]
fn testParse() {
    crate::shortcut_parse(
        &HashParser,
        &provider("Parser/Shortcut/HashParserTest", "testParse"),
    );
}

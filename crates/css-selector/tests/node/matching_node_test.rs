//! MatchingNodeTest.php (through AbstractNodeTestCase.php) at symfony/css-selector
//! v8.1.6. The provider rows are upstream's, recorded with every node
//! property (tests/data/php.json).
//!
//! Upstream: symfony/css-selector Tests/Node/MatchingNodeTest.php Tests/Node/AbstractNodeTestCase.php

use crate::common::provider;

#[test]
fn testToStringConversion() {
    crate::to_string_conversion(
        "Node/MatchingNodeTest",
        &provider("Node/MatchingNodeTest", "testToStringConversion"),
    );
}

#[test]
fn testSpecificityValue() {
    crate::specificity_value(&provider("Node/MatchingNodeTest", "testSpecificityValue"));
}

//! FunctionNodeTest.php (through AbstractNodeTestCase.php) at symfony/css-selector
//! v8.1.6. The provider rows are upstream's, recorded with every node
//! property (tests/data/php.json).
//!
//! Upstream: symfony/css-selector Tests/Node/FunctionNodeTest.php Tests/Node/AbstractNodeTestCase.php

use crate::common::provider;

#[test]
fn testToStringConversion() {
    crate::to_string_conversion(
        "Node/FunctionNodeTest",
        &provider("Node/FunctionNodeTest", "testToStringConversion"),
    );
}

#[test]
fn testSpecificityValue() {
    crate::specificity_value(&provider("Node/FunctionNodeTest", "testSpecificityValue"));
}

//! SpecificityTest.php at symfony/css-selector v8.1.6, on upstream's
//! provider rows (tests/data/php.json).
//!
//! Upstream: symfony/css-selector Tests/Node/SpecificityTest.php

use crate::common::{provider, specificity};
use symfony_css_selector::node::Specificity;

#[test]
fn testValue() {
    for (name, args) in provider("Node/SpecificityTest", "testValue") {
        assert_eq!(
            args[1].int(),
            specificity(&args[0]).get_value(),
            "row {name}"
        );
    }
}

#[test]
fn testPlusValue() {
    for (name, args) in provider("Node/SpecificityTest", "testPlusValue") {
        let plus = specificity(&args[0]).plus(&Specificity::new(1, 2, 3));
        assert_eq!(args[1].int() + 123, plus.get_value(), "row {name}");
    }
}

#[test]
fn testCompareTo() {
    for (name, args) in provider("Node/SpecificityTest", "testCompareTo") {
        let result = specificity(&args[0]).compare_to(&specificity(&args[1]));
        assert_eq!(args[2].int(), i64::from(result), "row {name}");
    }
}

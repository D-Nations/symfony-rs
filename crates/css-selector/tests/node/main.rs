//! The Node tests at symfony/css-selector v8.1.6, one module per upstream
//! test class; AbstractNodeTestCase's two tests are the helpers below.
//!
//! Upstream: none
#![allow(non_snake_case)]

#[path = "../common/mod.rs"]
mod common;

mod attribute_node_test;
mod class_node_test;
mod combined_selector_node_test;
mod element_node_test;
mod function_node_test;
mod hash_node_test;
mod matching_node_test;
mod negation_node_test;
mod pseudo_node_test;
mod selector_node_test;
mod specificity_adjustment_node_test;
mod specificity_test;

use common::{V, node, show};

/// `assertEquals($representation, (string) $node)`.
fn to_string_conversion(class: &str, rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        let actual = node(&args[0]).to_php_string();
        assert_eq!(show(&args[1].bytes()), show(&actual), "{class} row {name}");
    }
}

/// `assertEquals($value, $node->getSpecificity()->getValue())`.
fn specificity_value(rows: &[(String, Vec<V>)]) {
    for (name, args) in rows {
        assert_eq!(
            args[1].int(),
            node(&args[0]).get_specificity().get_value(),
            "row {name}"
        );
    }
}

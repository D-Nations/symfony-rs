//! The Definition namespace: configuration trees and their processing.
//!
//! Each node class is a unit struct whose `new()` stands for PHP's
//! `new ArrayNode(...)` and returns the shared [`Node`] handle, which is
//! why `new` does not return `Self` here.
//!
//! Upstream: none
#![allow(
    clippy::new_ret_no_self,
    reason = "node classes construct the shared Node handle, as PHP's `new` does"
)]

pub mod array_node;
pub mod array_shape_generator;
pub mod base_node;
pub mod boolean_node;
pub mod builder;
pub mod configuration_interface;
pub mod dumper;
pub mod enum_node;
pub mod exception;
pub mod float_node;
pub mod integer_node;
pub mod numeric_node;
pub mod processor;
pub mod prototyped_array_node;
pub mod scalar_node;
pub mod string_node;
pub mod variable_node;

pub use array_node::ArrayNode;
pub use array_shape_generator::ArrayShapeGenerator;
pub use base_node::{
    Closure, CustomNode, DEFAULT_DEPRECATION_MESSAGE, DEFAULT_PATH_SEPARATOR, Deprecation, Node,
    reset_placeholders, set_placeholder, set_placeholder_unique_prefix,
};
pub use boolean_node::BooleanNode;
pub use configuration_interface::ConfigurationInterface;
pub use enum_node::EnumNode;
pub use exception::{Class, Error};
pub use float_node::FloatNode;
pub use integer_node::IntegerNode;
pub use numeric_node::NumericNode;
pub use processor::Processor;
pub use prototyped_array_node::PrototypedArrayNode;
pub use scalar_node::ScalarNode;
pub use string_node::StringNode;
pub use variable_node::{DefaultClosure, DefaultValue, VariableNode};

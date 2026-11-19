//! The Definition\Builder namespace: the fluent interface that builds a
//! configuration tree (TreeBuilder, NodeBuilder, the node definitions and
//! their expression, merge, normalization and validation builders).
//!
//! A definition is a handle to one shared, mutable definition, as a PHP
//! object is: the handle a builder method returns and the one its parent
//! keeps are the same definition. Each handle also carries the typed
//! parent `end()` returns (`Definition<C, P>`: class `C`, parent `P`), as
//! upstream's `@template TParent` generics describe it. The runtime
//! parent upstream reads when it creates a node is kept separately, so a
//! definition whose parent is not a node still fails `getNode()` as PHP's
//! constructors do.
//!
//! Upstream: none

pub mod array_node_definition;
pub mod boolean_node_definition;
pub mod enum_node_definition;
pub mod expr_builder;
pub mod float_node_definition;
pub mod integer_node_definition;
pub mod merge_builder;
pub mod node_builder;
pub mod node_definition;
pub mod normalization_builder;
pub mod numeric_node_definition;
pub mod scalar_node_definition;
pub mod string_node_definition;
pub mod tree_builder;
pub mod validation_builder;
pub mod variable_node_definition;

pub use array_node_definition::ArrayNodeDefinition;
pub use boolean_node_definition::BooleanNodeDefinition;
pub use enum_node_definition::EnumNodeDefinition;
pub use expr_builder::{ExprBuilder, ExprType};
pub use float_node_definition::FloatNodeDefinition;
pub use integer_node_definition::IntegerNodeDefinition;
pub use node_builder::{NodeBuilder, NodeBuilderClass};
pub use node_definition::{
    Builtin, Call, CustomDefinition, Definition, IntoName, NodeClass, NodeDefinition, Parent,
    ParentHandle, class,
};
pub use numeric_node_definition::IntoNumber;
pub use scalar_node_definition::ScalarNodeDefinition;
pub use string_node_definition::StringNodeDefinition;
pub use tree_builder::TreeBuilder;
pub use variable_node_definition::VariableNodeDefinition;

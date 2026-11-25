//! Port of Symfony\Component\Config\Definition\Builder\IntegerNodeDefinition.php:
//! the definition of an `IntegerNode`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/IntegerNodeDefinition.php

use crate::definition::{
    base_node::Node,
    builder::node_definition::{Def, Definition, class},
    exception::Error,
    integer_node::IntegerNode,
    numeric_node,
};

/// `IntegerNodeDefinition`.
pub type IntegerNodeDefinition<P = ()> = Definition<class::Integer, P>;

/// `instantiateNode()`.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(numeric_node::CLASS)?;
    let (separator, n) = def.read(|s| (s.path_separator.clone(), s.numeric));
    IntegerNode::with_separator(def.name_bytes(), parent.as_ref(), n.min, n.max, &separator)
}

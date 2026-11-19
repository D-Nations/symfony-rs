//! Port of Symfony\Component\Config\Definition\Builder\FloatNodeDefinition.php:
//! the definition of a FloatNode.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/FloatNodeDefinition.php

use crate::definition::{
    base_node::Node,
    builder::node_definition::{Def, Definition, class},
    exception::Error,
    float_node::FloatNode,
    numeric_node,
};

/// FloatNodeDefinition.
pub type FloatNodeDefinition<P = ()> = Definition<class::Float, P>;

/// `instantiateNode()`.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(numeric_node::CLASS)?;
    let (separator, n) = def.read(|s| (s.path_separator.clone(), s.numeric));
    FloatNode::with_separator(def.name_bytes(), parent.as_ref(), n.min, n.max, &separator)
}

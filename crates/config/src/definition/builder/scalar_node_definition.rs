//! Port of Symfony\Component\Config\Definition\Builder\ScalarNodeDefinition.php:
//! the definition of a ScalarNode.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/ScalarNodeDefinition.php

use crate::definition::{
    base_node::{BASE_NODE, Node},
    builder::node_definition::{Def, Definition, class},
    exception::Error,
    scalar_node::ScalarNode,
};

/// ScalarNodeDefinition.
pub type ScalarNodeDefinition<P = ()> = Definition<class::Scalar, P>;

/// `instantiateNode()`.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(BASE_NODE)?;
    ScalarNode::with_separator(
        def.name_bytes(),
        parent.as_ref(),
        &def.read(|s| s.path_separator.clone()),
    )
}

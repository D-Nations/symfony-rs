//! Port of Symfony\Component\Config\Definition\Builder\StringNodeDefinition.php:
//! the definition of a StringNode. Its null equivalent is `""` (set by
//! the constructor in [`crate::definition::builder::node_definition`]).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/StringNodeDefinition.php

use crate::definition::{
    base_node::{BASE_NODE, Node},
    builder::node_definition::{Def, Definition, class},
    exception::Error,
    string_node::StringNode,
};

/// StringNodeDefinition.
pub type StringNodeDefinition<P = ()> = Definition<class::String, P>;

/// `instantiateNode()`.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(BASE_NODE)?;
    StringNode::with_separator(
        def.name_bytes(),
        parent.as_ref(),
        &def.read(|s| s.path_separator.clone()),
    )
}

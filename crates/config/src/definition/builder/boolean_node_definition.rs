//! Port of Symfony\Component\Config\Definition\Builder\BooleanNodeDefinition.php:
//! the definition of a BooleanNode, nullable when its null equivalent is
//! null. Its null equivalent is `true` until `defaultValue(null)`; the
//! constructor, `defaultValue()` and `cannotBeEmpty()` (which it refuses)
//! are in [`crate::definition::builder::node_definition`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/BooleanNodeDefinition.php

use crate::definition::{
    base_node::Node,
    boolean_node::{self, BooleanNode},
    builder::node_definition::{Def, Definition, class},
    exception::Error,
};

/// BooleanNodeDefinition.
pub type BooleanNodeDefinition<P = ()> = Definition<class::Boolean, P>;

/// `instantiateNode()`.
pub(crate) fn instantiate_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(boolean_node::CLASS)?;
    let (separator, nullable) =
        def.read(|s| (s.path_separator.clone(), s.null_equivalent.is_null()));
    BooleanNode::with_options(def.name_bytes(), parent.as_ref(), &separator, nullable)
}

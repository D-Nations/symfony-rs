//! Upstream's Tests/Fixtures for the Builder tests: `BarNode` (an `ArrayNode`
//! subclass), Builder\BarNodeDefinition, Builder\NodeBuilder (maps "bar"
//! and "variable" to the fixture classes) and Builder\VariableNodeDefinition.
//!
//! Upstream: symfony/config Tests/Fixtures/BarNode.php Tests/Fixtures/Builder/BarNodeDefinition.php Tests/Fixtures/Builder/NodeBuilder.php Tests/Fixtures/Builder/VariableNodeDefinition.php

use std::sync::Arc;

use symfony_config::{
    Error, Node,
    definition::{
        ArrayNode,
        builder::{
            Builtin, CustomDefinition, IntoName, NodeBuilder, NodeBuilderClass, NodeClass,
            NodeDefinition,
        },
    },
};

pub const BAR_NODE: &str = "Symfony\\Component\\Config\\Tests\\Fixtures\\BarNode";
pub const BAR_NODE_DEFINITION: &str =
    "Symfony\\Component\\Config\\Tests\\Fixtures\\Builder\\BarNodeDefinition";
pub const CUSTOM_NODE_BUILDER: &str =
    "Symfony\\Component\\Config\\Tests\\Fixtures\\Builder\\NodeBuilder";
pub const VARIABLE_NODE_DEFINITION: &str =
    "Symfony\\Component\\Config\\Tests\\Fixtures\\Builder\\VariableNodeDefinition";

/// `class BarNodeDefinition extends NodeDefinition`.
pub struct BarNodeDefinition;

impl CustomDefinition for BarNodeDefinition {
    fn class(&self) -> &str {
        BAR_NODE_DEFINITION
    }

    /// `new BarNode($this->name)`.
    fn create_node(&self, definition: &NodeDefinition) -> Result<Node, Error> {
        let name = definition.name().map(|n| n.to_vec()).unwrap_or_default();
        Ok(ArrayNode::new(name, None)?.with_class(BAR_NODE))
    }
}

/// `class NodeBuilder extends BaseNodeBuilder`.
pub struct CustomNodeBuilder;

impl NodeBuilderClass for CustomNodeBuilder {
    fn class(&self) -> &str {
        CUSTOM_NODE_BUILDER
    }

    fn get_node_class(&self, type_name: &str) -> Option<NodeClass> {
        match type_name {
            "bar" => Some(NodeClass::Custom(Arc::new(BarNodeDefinition))),
            "variable" => Some(NodeClass::Subclass {
                class: Arc::from(VARIABLE_NODE_DEFINITION),
                of: Builtin::Variable,
            }),
            _ => None,
        }
    }
}

pub fn custom_node_builder() -> NodeBuilder {
    NodeBuilder::with_class(Arc::new(CustomNodeBuilder))
}

/// The fixture's `barNode($name)`.
pub fn bar_node<P: Clone>(
    builder: &NodeBuilder<P>,
    name: impl IntoName,
) -> Result<NodeDefinition<NodeBuilder<P>>, Error> {
    builder.node(name, "bar")
}

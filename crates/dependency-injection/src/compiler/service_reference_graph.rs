//! Port of Symfony\Component\DependencyInjection\Compiler\ServiceReferenceGraph.php,
//! ServiceReferenceGraphEdge.php and ServiceReferenceGraphNode.php.
//!
//! Nodes and edges are PHP objects that point at each other; here they
//! live in the graph and are named by [`NodeId`] and [`EdgeId`]
//! (`$edge->getSourceNode()` is [`ServiceReferenceGraphEdge::get_source_node`],
//! read back with [`ServiceReferenceGraph::node`]). A node replaced by
//! `createNode()` (same id, another value) keeps its id and edges, as
//! the old object does in PHP, but is no longer in `getNodes()`.
//! `clear()` empties the graph, ids included.
//!
//! Upstream: symfony/dependency-injection Compiler/ServiceReferenceGraph.php Compiler/ServiceReferenceGraphEdge.php Compiler/ServiceReferenceGraphNode.php

use php_value::Value;

use crate::alias::Alias;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// A node of the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

/// An edge of the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(usize);

/// `ServiceReferenceGraphNode`.
#[derive(Debug, Clone)]
pub struct ServiceReferenceGraphNode {
    id: String,
    value: Value,
    in_edges: Vec<EdgeId>,
    out_edges: Vec<EdgeId>,
}

impl ServiceReferenceGraphNode {
    /// `isAlias()`.
    pub fn is_alias(&self) -> bool {
        Alias::from_value(&self.value).is_some()
    }

    /// `isDefinition()`.
    pub fn is_definition(&self) -> bool {
        Definition::from_value(&self.value).is_some()
    }

    /// `getId()`.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// `getInEdges()`.
    pub fn get_in_edges(&self) -> &[EdgeId] {
        &self.in_edges
    }

    /// `getOutEdges()`.
    pub fn get_out_edges(&self) -> &[EdgeId] {
        &self.out_edges
    }

    /// `getValue()`.
    pub fn get_value(&self) -> &Value {
        &self.value
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        self.in_edges.clear();
        self.out_edges.clear();
    }
}

/// The flags of an edge (`$lazy`, `$weak`, `$byConstructor`,
/// `$byMultiUseArgument`, `$fromExpression`), all false by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's five flags")]
pub struct EdgeFlags {
    pub lazy: bool,
    pub weak: bool,
    pub by_constructor: bool,
    pub by_multi_use_argument: bool,
    pub from_expression: bool,
}

/// `ServiceReferenceGraphEdge`.
#[derive(Debug, Clone)]
pub struct ServiceReferenceGraphEdge {
    source_node: NodeId,
    dest_node: NodeId,
    value: Option<Reference>,
    flags: EdgeFlags,
}

impl ServiceReferenceGraphEdge {
    /// `getValue()`: the reference.
    pub fn get_value(&self) -> Option<&Reference> {
        self.value.as_ref()
    }

    /// `getSourceNode()`.
    pub fn get_source_node(&self) -> NodeId {
        self.source_node
    }

    /// `getDestNode()`.
    pub fn get_dest_node(&self) -> NodeId {
        self.dest_node
    }

    /// `isLazy()`.
    pub fn is_lazy(&self) -> bool {
        self.flags.lazy
    }

    /// `isWeak()`.
    pub fn is_weak(&self) -> bool {
        self.flags.weak
    }

    /// `isReferencedByConstructor()`.
    pub fn is_referenced_by_constructor(&self) -> bool {
        self.flags.by_constructor
    }

    /// `isFromMultiUseArgument()`.
    pub fn is_from_multi_use_argument(&self) -> bool {
        self.flags.by_multi_use_argument
    }

    /// `isFromExpression()`.
    pub fn is_from_expression(&self) -> bool {
        self.flags.from_expression
    }
}

/// `ServiceReferenceGraph`: a directed graph of the services.
#[derive(Debug, Clone, Default)]
pub struct ServiceReferenceGraph {
    by_id: IdMap<NodeId>,
    nodes: Vec<ServiceReferenceGraphNode>,
    edges: Vec<ServiceReferenceGraphEdge>,
}

impl ServiceReferenceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// `hasNode($id)`.
    pub fn has_node(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// `getNode($id)`.
    pub fn get_node(&self, id: &str) -> Result<NodeId> {
        self.by_id
            .get(id)
            .copied()
            .ok_or_else(|| Error::invalid_argument(format!("There is no node with id \"{id}\".")))
    }

    /// `getNodes()`: the ids and their nodes.
    pub fn get_nodes(&self) -> impl Iterator<Item = (&str, NodeId)> {
        self.by_id.iter().map(|(id, node)| (id, *node))
    }

    /// The node `id` names.
    pub fn node(&self, id: NodeId) -> &ServiceReferenceGraphNode {
        &self.nodes[id.0]
    }

    /// The edge `id` names.
    pub fn edge(&self, id: EdgeId) -> &ServiceReferenceGraphEdge {
        &self.edges[id.0]
    }

    /// `clear()`.
    pub fn clear(&mut self) {
        for (_, node) in self.by_id.iter() {
            self.nodes[node.0].clear();
        }
        self.by_id.clear();
        self.nodes.clear();
        self.edges.clear();
    }

    /// `connect($sourceId, $sourceValue, $destId, $destValue, $reference,
    /// ...flags)`: nothing when either id is null.
    pub fn connect(
        &mut self,
        source_id: Option<&str>,
        source_value: &Value,
        dest_id: Option<&str>,
        dest_value: &Value,
        reference: Option<Reference>,
        flags: EdgeFlags,
    ) {
        let (Some(source_id), Some(dest_id)) = (source_id, dest_id) else {
            return;
        };
        let source = self.create_node(source_id, source_value);
        let dest = self.create_node(dest_id, dest_value);
        let edge = EdgeId(self.edges.len());
        self.edges.push(ServiceReferenceGraphEdge {
            source_node: source,
            dest_node: dest,
            value: reference,
            flags,
        });
        self.nodes[source.0].out_edges.push(edge);
        self.nodes[dest.0].in_edges.push(edge);
    }

    fn create_node(&mut self, id: &str, value: &Value) -> NodeId {
        if let Some(&node) = self.by_id.get(id)
            && self.nodes[node.0].value.identical(value)
        {
            return node;
        }
        let node = NodeId(self.nodes.len());
        self.nodes.push(ServiceReferenceGraphNode {
            id: id.to_string(),
            value: value.clone(),
            in_edges: Vec::new(),
            out_edges: Vec::new(),
        });
        self.by_id.insert(id, node);
        node
    }
}

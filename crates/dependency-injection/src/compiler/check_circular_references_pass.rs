//! Port of Symfony\Component\DependencyInjection\Compiler\CheckCircularReferencesPass.php.
//!
//! Checks the compiler's service reference graph for circular
//! references. References from method calls are ignored since we might
//! be able to resolve these references depending on the order in which
//! services are called; those are only detected at run time.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckCircularReferencesPass.php

use std::collections::HashSet;

use php_value::Value;
use php_value::compare::loose_equals;

use super::compiler_pass_interface::CompilerPassInterface;
use super::service_reference_graph::{EdgeId, ServiceReferenceGraph};
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};

/// `CheckCircularReferencesPass`.
#[derive(Debug, Default)]
pub struct CheckCircularReferencesPass {
    current_path: Vec<String>,
    checked_nodes: HashSet<String>,
    checked_lazy_nodes: HashSet<String>,
}

impl CheckCircularReferencesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckCircularReferencesPass";

    pub fn new() -> Self {
        Self::default()
    }

    /// `checkOutEdges($edges)`.
    fn check_out_edges(&mut self, graph: &ServiceReferenceGraph, edges: &[EdgeId]) -> Result<()> {
        for edge in edges {
            let edge = graph.edge(*edge);
            let node = graph.node(edge.get_dest_node());
            let id = node.get_id().to_string();

            if self.checked_nodes.contains(&id) {
                continue;
            }

            let is_leaf = node.get_value().is_truthy();
            let is_concrete = !edge.is_lazy() && !edge.is_weak();

            // Skip already checked lazy services if they are still lazy. Will not gain any new information.
            if self.checked_lazy_nodes.contains(&id) && (!is_leaf || !is_concrete) {
                continue;
            }

            // Process concrete references, otherwise defer check circular references for lazy edges.
            if !is_leaf || is_concrete {
                // array_search() compares loosely.
                let needle = Value::from(id.as_str());
                let search_key = self
                    .current_path
                    .iter()
                    .position(|p| loose_equals(&needle, &Value::from(p.as_str())));
                self.current_path.push(id.clone());

                if let Some(at) = search_key {
                    return Err(Error::service_circular_reference(
                        id,
                        self.current_path[at..].to_vec(),
                    ));
                }

                self.check_out_edges(graph, node.get_out_edges())?;

                self.checked_nodes.insert(id.clone());
                self.checked_lazy_nodes.remove(&id);
            } else {
                self.checked_lazy_nodes.insert(id);
            }

            self.current_path.pop();
        }
        Ok(())
    }
}

impl CompilerPassInterface for CheckCircularReferencesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let graph = container.get_compiler().get_service_reference_graph();

        self.checked_nodes.clear();
        for (id, node) in graph.get_nodes() {
            self.current_path = vec![id.to_string()];

            self.check_out_edges(graph, graph.node(node).get_out_edges())?;
        }
        Ok(())
    }
}

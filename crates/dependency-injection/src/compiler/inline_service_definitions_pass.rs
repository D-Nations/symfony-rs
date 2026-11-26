//! Port of Symfony\Component\DependencyInjection\Compiler\InlineServiceDefinitionsPass.php.
//!
//! Replaces references to private services used once (and to non-shared
//! services) by their definitions. With an analyzing pass, the graph is
//! built on a copy of the container, as upstream does; the port has no
//! expression language providers to copy into it.
//!
//! Upstream: symfony/dependency-injection Compiler/InlineServiceDefinitionsPass.php

use std::collections::HashSet;

use php_value::{Value, array::Key};

use super::abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState};
use super::analyze_service_references_pass::AnalyzeServiceReferencesPass;
use super::compiler_pass_interface::CompilerPassInterface;
use super::service_reference_graph::ServiceReferenceGraph;
use crate::argument::Argument;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `InlineServiceDefinitionsPass`.
#[derive(Debug)]
pub struct InlineServiceDefinitionsPass {
    state: RecursivePassState,
    cloning_ids: Vec<String>,
    connected_ids: HashSet<String>,
    not_inlined_ids: HashSet<String>,
    inlined_ids: IdMap<bool>,
    not_inlinable_ids: HashSet<String>,
    autowire_inline: HashSet<String>,
    graph: Option<ServiceReferenceGraph>,
    analyzing_pass: Option<AnalyzeServiceReferencesPass>,
}

impl Default for InlineServiceDefinitionsPass {
    fn default() -> Self {
        Self::new(None)
    }
}

impl InlineServiceDefinitionsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\InlineServiceDefinitionsPass";

    /// `new InlineServiceDefinitionsPass($analyzingPass)`.
    pub fn new(analyzing_pass: Option<AnalyzeServiceReferencesPass>) -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            cloning_ids: Vec::new(),
            connected_ids: HashSet::new(),
            not_inlined_ids: HashSet::new(),
            inlined_ids: IdMap::new(),
            not_inlinable_ids: HashSet::new(),
            autowire_inline: HashSet::new(),
            graph: None,
            analyzing_pass,
        }
    }

    fn run(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let mut analyzed = if self.analyzing_pass.is_some() {
            let mut analyzed = ContainerBuilder::new();
            analyzed.set_aliases(
                container
                    .get_aliases()
                    .iter()
                    .map(|(id, alias)| (id.to_string(), alias.clone())),
            )?;
            analyzed.set_definitions(definitions_of(container))?;
            Some(analyzed)
        } else {
            None
        };

        let mut not_inlinable_ids: HashSet<String> = HashSet::new();
        let mut remaining_inlined_ids: IdMap<()> = IdMap::new();
        self.connected_ids = container
            .get_definitions()
            .keys()
            .map(str::to_string)
            .collect();
        self.not_inlined_ids = self.connected_ids.clone();
        loop {
            if let (Some(pass), Some(analyzed)) = (self.analyzing_pass.as_mut(), analyzed.as_mut())
            {
                let kept: Vec<(String, Definition)> = definitions_of(analyzed)
                    .into_iter()
                    .filter(|(id, _)| self.connected_ids.contains(id))
                    .collect();
                analyzed.set_definitions(kept)?;
                pass.process(analyzed)?;
            }
            let analyzed_container = analyzed.as_mut().unwrap_or(&mut *container);
            self.graph = Some(
                analyzed_container
                    .get_compiler()
                    .get_service_reference_graph()
                    .clone(),
            );
            let definitions = definitions_of(analyzed_container);
            let not_inlined_ids = std::mem::take(&mut self.not_inlined_ids);
            not_inlinable_ids.extend(std::mem::take(&mut self.not_inlinable_ids));
            self.connected_ids.clear();
            self.inlined_ids.clear();

            for (id, definition) in definitions {
                let graph = self.graph()?;
                let Ok(node) = graph.get_node(&id) else {
                    continue;
                };
                let referenced = graph.node(node).get_out_edges().iter().any(|edge| {
                    let source = graph.node(graph.edge(*edge).get_source_node());
                    not_inlined_ids.contains(source.get_id())
                });
                if definition.is_public() {
                    self.connected_ids.insert(id.clone());
                }
                if referenced {
                    self.state.current_id = Some(id.clone());
                    self.process_value(container, definition.to_value(), true)?;
                }
            }

            let inlined: Vec<(String, bool)> = self
                .inlined_ids
                .iter()
                .map(|(id, flag)| (id.to_string(), *flag))
                .collect();
            for (id, is_public_or_not_shared) in inlined {
                if is_public_or_not_shared {
                    remaining_inlined_ids.insert(id, ());
                } else {
                    container.remove_definition(&id);
                    if !self.autowire_inline.contains(&id)
                        && let Some(analyzed) = analyzed.as_mut()
                    {
                        analyzed.remove_definition(&id);
                    }
                }
            }
            if self.inlined_ids.is_empty() || self.analyzing_pass.is_none() {
                break;
            }
        }

        for id in remaining_inlined_ids.keys() {
            if not_inlinable_ids.contains(id) {
                continue;
            }
            let definition = container.get_definition(id)?;
            if !definition.is_shared() && !definition.is_public() {
                container.remove_definition(id);
            }
        }
        Ok(())
    }

    fn graph(&self) -> Result<&ServiceReferenceGraph> {
        self.graph
            .as_ref()
            .ok_or_else(|| Error::logic("The service reference graph is not set."))
    }

    /// `isInlineableDefinition($id, $definition)`.
    fn is_inlineable_definition(
        &mut self,
        container: &ContainerBuilder,
        id: &str,
        definition: &Definition,
    ) -> Result<bool> {
        if id.starts_with(".autowire_inline.") {
            self.autowire_inline.insert(id.to_string());
            return Ok(true);
        }
        if definition.has_errors()
            || definition.is_deprecated()
            || definition.is_lazy()
            || definition.is_synthetic()
            || definition.has_tag("container.do_not_inline")
        {
            return Ok(false);
        }

        let Some(graph) = self.graph.as_ref() else {
            return Err(Error::logic("The service reference graph is not set."));
        };

        if !definition.is_shared() {
            let Ok(node) = graph.get_node(id) else {
                return Ok(true);
            };
            for edge in graph.node(node).get_in_edges() {
                let edge = graph.edge(*edge);
                let src_id = graph.node(edge.get_source_node()).get_id();
                self.connected_ids.insert(src_id.to_string());
                if edge.is_weak() || edge.is_lazy() {
                    self.connected_ids.insert(id.to_string());
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        if definition.is_public() || self.state.current_id.as_deref() == Some(id) {
            return Ok(false);
        }
        let Ok(node) = graph.get_node(id) else {
            return Ok(false);
        };

        self.connected_ids.insert(id.to_string());

        let mut src_ids: Vec<String> = Vec::new();
        let mut src_count = 0;
        for edge in graph.node(node).get_in_edges() {
            let edge = graph.edge(*edge);
            let src_id = graph.node(edge.get_source_node()).get_id().to_string();
            self.connected_ids.insert(src_id.clone());
            if edge.is_weak() || edge.is_lazy() {
                return Ok(false);
            }
            if !src_ids.contains(&src_id) {
                src_ids.push(src_id);
            }
            src_count += 1;
        }

        if src_ids.len() != 1 {
            self.not_inlined_ids.insert(id.to_string());
            return Ok(false);
        }

        if src_count > 1
            && let Value::Array(factory) = definition.factory()
            && factory.get(&Key::Int(0)).is_some_and(|f| {
                Reference::from_value(f).is_some() || Definition::from_value(f).is_some()
            })
        {
            return Ok(false);
        }

        let src_definition = container.get_definition(&src_ids[0])?;
        Ok(src_definition.is_shared() && !src_definition.is_lazy())
    }
}

fn definitions_of(container: &ContainerBuilder) -> Vec<(String, Definition)> {
    container
        .get_definitions()
        .iter()
        .map(|(id, definition)| (id.to_string(), definition.clone()))
        .collect()
}

impl CompilerPassInterface for InlineServiceDefinitionsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let result = self.run(container);
        self.connected_ids.clear();
        self.not_inlined_ids.clear();
        self.inlined_ids.clear();
        self.not_inlinable_ids.clear();
        self.graph = None;
        result
    }
}

impl AbstractRecursivePass for InlineServiceDefinitionsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if Argument::from_value(&value).is_some() {
            // References found in ArgumentInterface::getValues() are not inlineable
            return Ok(value);
        }

        let mut value = value;
        if !self.cloning_ids.is_empty()
            && let Some(definition) = Definition::from_value(&value)
        {
            if definition.is_shared() {
                return Ok(value);
            }
            value = definition.clone_object().to_value();
        }

        let Some(reference) = Reference::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        let id = reference.id().to_string();
        if !container.has_definition(&id) {
            return Ok(value);
        }

        let definition = container.get_definition(&id)?;

        if self.not_inlinable_ids.contains(&id)
            || !self.is_inlineable_definition(container, &id, &definition)?
        {
            if self.state.current_id.as_deref() != Some(id.as_str()) {
                self.not_inlinable_ids.insert(id);
            }
            return Ok(value);
        }

        let current_id = self.state.current_id.clone().unwrap_or_default();
        container.log(
            Self::CLASS,
            &format!("Inlined service \"{id}\" to \"{current_id}\"."),
        )?;
        self.inlined_ids.insert(
            id.clone(),
            definition.is_public() || !definition.is_shared(),
        );
        self.not_inlined_ids.insert(current_id);

        if definition.is_shared() {
            return Ok(definition.to_value());
        }

        if let Some(at) = self.cloning_ids.iter().position(|c| *c == id) {
            let mut path = self.cloning_ids[at..].to_vec();
            path.push(id.clone());
            return Err(Error::service_circular_reference(id, path));
        }

        self.cloning_ids.push(id.clone());
        let result = self.process_value(container, definition.to_value(), false);
        self.cloning_ids.retain(|c| *c != id);
        result
    }
}

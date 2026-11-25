//! Port of Symfony\Component\DependencyInjection\Compiler\AnalyzeServiceReferencesPass.php.
//!
//! Fills the compiler's [`ServiceReferenceGraph`](super::ServiceReferenceGraph)
//! with the references between services. The port has no
//! `ExpressionLanguage`: a factory written as an expression (`@=...`)
//! fails with upstream's message for the missing component, and no edge
//! comes from an expression (`.internal.reference_in_expression`).
//!
//! Upstream: symfony/dependency-injection Compiler/AnalyzeServiceReferencesPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::service_reference_graph::EdgeFlags;
use crate::alias::Alias;
use crate::argument::Argument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `AnalyzeServiceReferencesPass`.
#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's properties")]
pub struct AnalyzeServiceReferencesPass {
    state: RecursivePassState,
    current_definition: Option<Definition>,
    lazy: bool,
    by_constructor: bool,
    by_factory: bool,
    by_multi_use_argument: bool,
    definitions: IdMap<Definition>,
    aliases: IdMap<Alias>,
    only_constructor_arguments: bool,
    has_proxy_dumper: bool,
}

impl Default for AnalyzeServiceReferencesPass {
    fn default() -> Self {
        Self::new(false, true)
    }
}

impl AnalyzeServiceReferencesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\AnalyzeServiceReferencesPass";

    /// `new AnalyzeServiceReferencesPass($onlyConstructorArguments,
    /// $hasProxyDumper)`.
    pub fn new(only_constructor_arguments: bool, has_proxy_dumper: bool) -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            current_definition: None,
            lazy: false,
            by_constructor: false,
            by_factory: false,
            by_multi_use_argument: false,
            definitions: IdMap::new(),
            aliases: IdMap::new(),
            only_constructor_arguments,
            has_proxy_dumper,
        }
    }

    fn get_definition_id(&self, id: &str) -> Option<String> {
        let mut id = id.to_string();
        while let Some(alias) = self.aliases.get(&id) {
            id = alias.id();
        }
        self.definitions.contains_key(&id).then_some(id)
    }

    fn target_value(&self, target_id: Option<&str>) -> Value {
        target_id
            .and_then(|id| self.definitions.get(id))
            .map_or(Value::Null, Definition::to_value)
    }

    fn run(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let aliases: Vec<(String, Alias)> = self
            .aliases
            .iter()
            .map(|(id, alias)| (id.to_string(), alias.clone()))
            .collect();
        for (id, alias) in aliases {
            let target_id = self.get_definition_id(&alias.id());
            let target = self.target_value(target_id.as_deref());
            container
                .get_compiler()
                .get_service_reference_graph_mut()
                .connect(
                    Some(&id),
                    &alias.to_value(),
                    target_id.as_deref(),
                    &target,
                    None,
                    EdgeFlags::default(),
                );
        }
        recursive_process(self, container)
    }

    fn process_definition(
        &mut self,
        container: &mut ContainerBuilder,
        value: &Definition,
        is_root: bool,
        lazy: bool,
    ) -> Result<()> {
        if is_root {
            if value.is_synthetic() || value.is_abstract() {
                return Ok(());
            }
            self.current_definition = Some(value.clone());
        } else if self
            .current_definition
            .as_ref()
            .is_some_and(|d| d.ptr_eq(value))
        {
            return Ok(());
        }
        self.lazy = false;

        let by_constructor = self.by_constructor;
        self.by_constructor = is_root || by_constructor;

        let by_factory = self.by_factory;
        self.by_factory = true;
        let factory = value.factory();
        if let Value::String(s) = &factory
            && s.as_bytes().starts_with(b"@=")
        {
            return Err(Error::logic(
                "Expressions cannot be used in service factories without the ExpressionLanguage component. Try running \"composer require symfony/expression-language\".",
            ));
        }
        self.process_value(container, factory, false)?;
        self.by_factory = by_factory;

        // a lazy closure gets its service only when it is called
        let closure_factory = Value::Array(PhpArray::list([
            Value::from("Closure"),
            Value::from("fromCallable"),
        ]));
        let class = value.class().filter(|c| !c.is_empty() && c != "0");
        self.lazy = closure_factory.identical(&value.factory())
            && (class.as_deref().unwrap_or("Closure") != "Closure" || value.is_lazy());
        self.process_value(container, Value::Array(value.arguments()), false)?;
        self.lazy = false;

        let mut properties = value.properties();
        let mut setters = value.method_calls();

        // Any references before a "wither" are part of the constructor-instantiation graph
        let mut last_wither_index: Option<Key> = None;
        for (k, call) in setters.iter() {
            if call
                .as_array()
                .and_then(|c| c.get(&Key::Int(2)))
                .is_some_and(Value::is_truthy)
            {
                last_wither_index = Some(k.clone());
            }
        }

        if last_wither_index.is_some() {
            self.process_value(container, Value::Array(properties), false)?;
            setters = PhpArray::new();
            properties = PhpArray::new();

            for (k, call) in value.method_calls().iter() {
                if last_wither_index.is_none() {
                    let _ = setters.push(call.clone());
                    continue;
                }
                if last_wither_index.as_ref() == Some(k) {
                    last_wither_index = None;
                }
                self.process_value(container, call.clone(), false)?;
            }
        }

        self.by_constructor = by_constructor;

        if !self.only_constructor_arguments {
            self.process_value(container, Value::Array(properties), false)?;
            self.process_value(container, Value::Array(setters), false)?;
            self.process_value(container, value.configurator(), false)?;
        }
        self.lazy = lazy;
        Ok(())
    }
}

impl CompilerPassInterface for AnalyzeServiceReferencesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    /// `process($container)`: the graph is cleared and filled again.
    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        container
            .get_compiler()
            .get_service_reference_graph_mut()
            .clear();
        self.lazy = false;
        self.by_constructor = false;
        self.by_factory = false;
        self.by_multi_use_argument = false;
        self.definitions = container.get_definitions().clone();
        self.aliases = container.get_aliases().clone();

        let result = self.run(container);
        self.aliases = IdMap::new();
        self.definitions = IdMap::new();
        result
    }
}

impl AbstractRecursivePass for AnalyzeServiceReferencesPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let lazy = self.lazy;
        self.in_expression(true);

        if let Some(argument) = Argument::from_value(&value) {
            let is_iterator = matches!(argument, Argument::Iterator(_));
            self.lazy = !self.by_factory || !is_iterator;
            let by_multi_use_argument = self.by_multi_use_argument;
            if is_iterator {
                self.by_multi_use_argument = true;
            }
            self.parent_process_value(container, Value::Array(argument.values()), false)?;
            self.by_multi_use_argument = by_multi_use_argument;
            self.lazy = lazy;
            return Ok(value);
        }
        if let Some(reference) = Reference::from_value(&value) {
            let target_id = self.get_definition_id(reference.id());
            let target = self.target_value(target_id.as_deref());
            let target_lazy = Definition::from_value(&target).is_some_and(|d| d.is_lazy());
            let flags = EdgeFlags {
                lazy: self.lazy || (self.has_proxy_dumper && target_lazy),
                weak: reference.invalid_behavior() == InvalidBehavior::IgnoreUninitialized,
                by_constructor: self.by_constructor,
                by_multi_use_argument: self.by_multi_use_argument,
                from_expression: false,
            };
            let source = self
                .current_definition
                .as_ref()
                .map_or(Value::Null, Definition::to_value);
            let current_id = self.state.current_id.clone();
            container
                .get_compiler()
                .get_service_reference_graph_mut()
                .connect(
                    current_id.as_deref(),
                    &source,
                    target_id.as_deref(),
                    &target,
                    Some(reference.clone()),
                    flags,
                );
            return Ok(value);
        }
        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        self.process_definition(container, &definition, is_root, lazy)?;
        Ok(value)
    }
}

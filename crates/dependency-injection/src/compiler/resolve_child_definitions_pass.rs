//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveChildDefinitionsPass.php.
//!
//! Replaces every `ChildDefinition` (in the container and inline) with
//! the equivalent `Definition`, merged from its parents.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveChildDefinitionsPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind, Result};

const EXCEPTION_INTERFACE: &str =
    "Symfony\\Component\\DependencyInjection\\Exception\\ExceptionInterface";

/// `ResolveChildDefinitionsPass`.
#[derive(Debug, Clone)]
pub struct ResolveChildDefinitionsPass {
    state: RecursivePassState,
    current_path: Vec<String>,
}

impl Default for ResolveChildDefinitionsPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveChildDefinitionsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveChildDefinitionsPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            current_path: Vec::new(),
        }
    }

    fn current_id(&self) -> String {
        self.state.current_id.clone().unwrap_or_default()
    }

    /// `resolveDefinition($definition)`.
    fn resolve_definition(
        &mut self,
        container: &mut ContainerBuilder,
        definition: &Definition,
    ) -> Result<Definition> {
        match self.do_resolve_definition(container, definition) {
            Ok(definition) => Ok(definition),
            Err(e) if e.is(&ErrorKind::ServiceCircularReference) => Err(e),
            Err(mut e) if e.instance_of(EXCEPTION_INTERFACE) => {
                let message = format!("Service \"{}\": {}", self.current_id(), e.message());
                e.set_message(message);
                Err(e)
            }
            Err(e) => Err(e),
        }
    }

    /// `doResolveDefinition($definition)`.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn do_resolve_definition(
        &mut self,
        container: &mut ContainerBuilder,
        definition: &Definition,
    ) -> Result<Definition> {
        let parent = definition.parent().unwrap_or_default();
        if !container.has(&parent) {
            return Err(Error::runtime(format!(
                "Parent definition \"{parent}\" does not exist."
            )));
        }

        let search_key = self.current_path.iter().position(|p| *p == parent);
        self.current_path.push(parent.clone());

        if let Some(search_key) = search_key {
            return Err(Error::service_circular_reference(
                parent,
                self.current_path[search_key..].to_vec(),
            ));
        }

        let mut parent_def = container.find_definition(&parent)?;
        if parent_def.is_child() {
            let id = self.state.current_id.clone();
            self.state.current_id = Some(parent.clone());
            parent_def = self.resolve_definition(container, &parent_def)?;
            container.set_definition(&parent, parent_def.clone())?;
            self.state.current_id = id;
        }

        container.log(
            Self::CLASS,
            &format!(
                "Resolving inheritance for \"{}\" (parent: {parent}).",
                self.current_id()
            ),
        )?;
        let def = Definition::default();

        // merge in parent definition
        // purposely ignored attributes: abstract, shared, tags, autoconfigured
        def.set_class(parent_def.class().as_deref());
        def.set_arguments(parent_def.arguments());
        def.set_method_calls(&parent_def.method_calls())?;
        def.set_properties(parent_def.properties());
        if let Some(deprecation) = parent_def.deprecation("%service_id%") {
            def.set_deprecated(
                &deprecation.package,
                &deprecation.version,
                &deprecation.message,
            )?;
        }
        def.set_factory(parent_def.factory());
        def.set_configurator(parent_def.configurator());
        def.set_file(parent_def.file().as_deref());
        def.set_public(parent_def.is_public());
        def.set_lazy(parent_def.is_lazy());
        def.set_autowired(parent_def.is_autowired());
        def.set_changes(parent_def.changes());

        let mut bindings = definition.bindings();
        bindings.union(&parent_def.bindings());
        def.set_bindings(&bindings);

        def.set_synthetic(definition.is_synthetic());

        // overwrite with values specified in the decorator
        let changes = definition.changes();
        let is_changed = |key: &str| changes.iter().any(|c| c == key);
        if is_changed("class") {
            def.set_class(definition.class().as_deref());
        }
        if is_changed("factory") {
            def.set_factory(definition.factory());
        }
        if is_changed("configurator") {
            def.set_configurator(definition.configurator());
        }
        if is_changed("file") {
            def.set_file(definition.file().as_deref());
        }
        if is_changed("public") {
            def.set_public(definition.is_public());
        } else {
            def.set_public(parent_def.is_public());
        }
        if is_changed("lazy") {
            def.set_lazy(definition.is_lazy());
        }
        if is_changed("deprecated")
            && let Some(deprecation) = definition.deprecation("%service_id%")
        {
            def.set_deprecated(
                &deprecation.package,
                &deprecation.version,
                &deprecation.message,
            )?;
        }
        if is_changed("autowired") {
            def.set_autowired(definition.is_autowired());
        }
        if is_changed("shared") {
            def.set_shared(definition.is_shared());
        }
        if is_changed("decorated_service") {
            match definition.decorated_service() {
                None => {
                    def.set_decorated_service(None, None, 0, InvalidBehavior::Exception)?;
                }
                Some(decorated) => {
                    def.set_decorated_service(
                        Some(&decorated.id),
                        decorated.renamed_id.as_deref(),
                        decorated.priority,
                        decorated
                            .invalid_behavior
                            .unwrap_or(InvalidBehavior::Exception),
                    )?;
                }
            }
        }

        // merge arguments
        for (k, v) in definition.arguments().iter() {
            match k {
                Key::Int(_) => {
                    def.add_argument(v.clone());
                }
                Key::Str(s) if php_value::numeric::is_numeric(s.as_bytes()) => {
                    def.add_argument(v.clone());
                }
                Key::Str(s) if s.as_bytes().starts_with(b"index_") => {
                    let index = php_value::numeric::string_to_int(&s.as_bytes()[6..]);
                    def.replace_argument(index, v.clone())?;
                }
                Key::Str(s) => {
                    def.set_argument(s.to_string_lossy().into_owned(), v.clone());
                }
            }
        }

        // merge properties
        for (k, v) in definition.properties().iter() {
            def.set_property(&String::from_utf8_lossy(&k.bytes()), v.clone());
        }

        // append method calls
        let calls = definition.method_calls();
        if !calls.is_empty() {
            let merged = PhpArray::list(def.method_calls().values().chain(calls.values()).cloned());
            def.set_method_calls(&merged)?;
        }

        def.add_errors_of(&parent_def);
        def.add_errors_of(definition);

        // these attributes are always taken from the child
        def.set_abstract(definition.is_abstract());
        def.set_tags(definition.tags());
        // autoconfigure is never taken from parent (on purpose)
        // and it's not legal on an instanceof
        def.set_autoconfigured(definition.is_autoconfigured());

        if !def.has_tag("proxy") {
            for v in parent_def.tag("proxy").values() {
                def.add_tag("proxy", v.as_array().cloned().unwrap_or_default());
            }
        }

        Ok(def)
    }
}

impl CompilerPassInterface for ResolveChildDefinitionsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveChildDefinitionsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(mut definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        if is_root {
            // yes, we are specifically fetching the definition from the
            // container to ensure we are not operating on stale data
            definition = container.get_definition(&self.current_id())?;
        }
        if definition.is_child() {
            self.current_path = Vec::new();
            definition = self.resolve_definition(container, &definition)?;
            if is_root {
                container.set_definition(&self.current_id(), definition.clone())?;
            }
        }

        self.parent_process_value(container, definition.to_value(), is_root)
    }
}

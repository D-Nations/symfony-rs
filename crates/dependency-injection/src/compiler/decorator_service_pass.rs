//! Port of Symfony\Component\DependencyInjection\Compiler\DecoratorServicePass.php.
//!
//! Overwrites a service but keeps the overridden one: each decorator
//! takes the decorated id (an alias to the decorator), the decorated
//! service moves to the inner id, and role-describing tags move to the
//! outermost decorator.
//!
//! Upstream: symfony/dependency-injection Compiler/DecoratorServicePass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{AbstractRecursivePass, RecursivePassState};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::alias::Alias;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `DecoratorServicePass`.
#[derive(Debug)]
pub struct DecoratorServicePass {
    state: RecursivePassState,
}

impl Default for DecoratorServicePass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

/// The behaviour-describing tags kept on the decorated service when the
/// `container.behavior_describing_tags` parameter is not set.
const DEFAULT_TAGS_TO_KEEP: [&str; 5] = [
    "proxy",
    "container.do_not_inline",
    "container.service_locator",
    "container.service_subscriber",
    "container.service_subscriber.locator",
];

/// `!$value` for an optional id.
fn is_falsy(id: Option<&str>) -> bool {
    id.is_none_or(|id| id.is_empty() || id == "0")
}

/// `array_merge($a, $b)`.
fn array_merge(a: &PhpArray, b: &PhpArray) -> PhpArray {
    let mut merged = PhpArray::new();
    for (k, v) in a.iter().chain(b.iter()) {
        match k {
            Key::Int(_) => {
                let _ = merged.push(v.clone());
            }
            Key::Str(_) => merged.insert(k.clone(), v.clone()),
        }
    }
    merged
}

/// `delayDecoratorsOfDecorators($container, $definitions)`: delays the
/// decorators of a decorator until all the decorators of the service it
/// decorates are processed. Tags move from a decorated service to its
/// outermost decorator one decorator at a time, and decorating a
/// decorator too early would strand them on it.
fn delay_decorators_of_decorators(
    container: &ContainerBuilder,
    definitions: Vec<(String, Definition)>,
) -> Vec<(String, Definition)> {
    let mut decorated_ids: IdMap<String> = IdMap::new();
    let mut remaining: IdMap<i64> = IdMap::new();

    for (id, definition) in &definitions {
        let inner = definition
            .decorated_service()
            .map(|d| d.id)
            .unwrap_or_default();
        decorated_ids.insert(id.as_str(), inner.clone());
        *remaining.get_or_insert_with(&inner, || 0) += 1;
    }

    let mut sorted: Vec<(String, Definition)> = Vec::new();
    let mut delayed: Vec<(String, Definition)> = Vec::new();

    for entry in definitions {
        delayed.push(entry);

        loop {
            let count = sorted.len();

            let mut still_delayed = Vec::new();
            for entry in std::mem::take(&mut delayed) {
                let inner = decorated_ids.get(&entry.0).cloned().unwrap_or_default();

                // a missing service has no tags to move, and delaying would change which decorators end up ignored
                if let Some(inner_of_inner) = decorated_ids.get(&inner)
                    && remaining.get(inner_of_inner).is_some_and(|r| *r != 0)
                    && container.has(inner_of_inner)
                {
                    still_delayed.push(entry);
                    continue;
                }

                sorted.push(entry);
                *remaining.get_or_insert_with(&inner, || 0) -= 1;
            }
            delayed = still_delayed;

            if delayed.is_empty() || count >= sorted.len() {
                break;
            }
        }
    }

    sorted.extend(delayed);
    sorted
}

impl DecoratorServicePass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\DecoratorServicePass";

    pub fn new() -> Self {
        Self::default()
    }
}

impl CompilerPassInterface for DecoratorServicePass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        // `SplPriorityQueue` with `[$priority, --$order]`: by priority, then
        // in the order of the definitions.
        let mut definitions: Vec<(i64, String, Definition)> = Vec::new();
        for (id, definition) in container.get_definitions().iter() {
            let Some(decorated) = definition.decorated_service() else {
                continue;
            };
            definitions.push((decorated.priority, id.to_string(), definition.clone()));
        }
        definitions.sort_by_key(|d| std::cmp::Reverse(d.0));
        let definitions = definitions
            .into_iter()
            .map(|(_, id, definition)| (id, definition))
            .collect();

        let mut decorating_definitions: IdMap<Definition> = IdMap::new();
        let mut decorated_ids: IdMap<String> = IdMap::new();

        // Behavior-describing tags must stay on the decorated service;
        // role-describing tags (e.g. "kernel.event_listener") move to its decorators.
        // The parameter is consumed and removed by ResolveInstanceofConditionalsPass, which runs
        // before this pass; it's read here for container builders that keep it around.
        let tags_to_keep: Vec<String> =
            if container.has_parameter("container.behavior_describing_tags") {
                let tags = container.get_parameter("container.behavior_describing_tags")?;
                tags.as_array()
                    .into_iter()
                    .flat_map(PhpArray::values)
                    .filter_map(Value::to_php_string)
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect()
            } else {
                DEFAULT_TAGS_TO_KEEP
                    .iter()
                    .map(|t| (*t).to_string())
                    .collect()
            };

        for (id, definition) in delay_decorators_of_decorators(container, definitions) {
            let Some(decorated_service) = definition.decorated_service() else {
                continue;
            };
            let inner = decorated_service.id.clone();
            let invalid_behavior = decorated_service
                .invalid_behavior
                .unwrap_or(InvalidBehavior::Exception);

            definition.set_decorated_service(None, None, 0, InvalidBehavior::Exception)?;

            let renamed_id = match decorated_service.renamed_id.as_deref() {
                Some(renamed) if !is_falsy(Some(renamed)) => renamed.to_string(),
                _ => format!("{id}.inner"),
            };

            decorated_ids.get_or_insert_with(&inner, || renamed_id.clone());
            self.state.current_id = Some(renamed_id.clone());
            self.process_value(container, definition.to_value(), false)?;

            definition.set_inner_service_id(Some(&renamed_id));
            definition.set_decoration_on_invalid(Some(invalid_behavior));
            definition.set_decoration_priority(Some(decorated_service.priority));

            // we create a new alias/service for the service we are replacing
            // to be able to reference it in the new one
            let public;
            let decorated_definition;
            if container.has_alias(&inner) {
                let alias = container.get_alias(&inner)?;
                public = alias.is_public();
                container.set_alias(&renamed_id, Alias::new(alias.id(), false))?;
                decorated_definition = Some(container.find_definition(&alias.id())?);
            } else if container.has_definition(&inner) {
                let decorated = container.get_definition(&inner)?;
                public = decorated.is_public();
                decorated.set_public(false);
                container.set_definition(&renamed_id, decorated.clone())?;
                decorating_definitions.insert(inner.as_str(), decorated.clone());
                decorated_definition = Some(decorated);
            } else if invalid_behavior == InvalidBehavior::Ignore {
                container.remove_definition(&id);
                continue;
            } else if invalid_behavior == InvalidBehavior::Null {
                public = definition.is_public();
                decorated_definition = None;
            } else {
                return Err(Error::service_not_found(inner, Some(id), Vec::new(), None));
            }

            if decorated_definition.is_some_and(|d| d.is_synthetic()) {
                return Err(Error::invalid_argument(format!(
                    "A synthetic service cannot be decorated: service \"{id}\" cannot decorate \"{inner}\"."
                )));
            }

            if let Some(decorating_definition) = decorating_definitions.get(&inner).cloned() {
                let mut decorating_tags = decorating_definition.tags();
                let mut reset_tags = PhpArray::new();

                for container_tag in &tags_to_keep {
                    let key = Key::from(container_tag.as_str());
                    if let Some(tag) = decorating_tags.remove(&key) {
                        reset_tags.insert(key, tag);
                    }
                }

                definition.set_tags(array_merge(&decorating_tags, &definition.tags()));
                decorating_definition.set_tags(reset_tags);
                decorating_definitions.insert(inner.as_str(), definition.clone());
            }

            container.set_alias(&inner, id.as_str())?.set_public(public);
        }

        for (inner, definition) in decorating_definitions.iter() {
            let mut attributes = PhpArray::new();
            attributes.insert("id", Value::from(inner));
            attributes.insert(
                "inner",
                Value::from(decorated_ids.get(inner).map_or("", String::as_str)),
            );
            definition.add_tag("container.decorator", attributes);
        }
        Ok(())
    }
}

impl AbstractRecursivePass for DecoratorServicePass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(reference) = Reference::from_value(&value)
            && reference.id() == ".inner"
        {
            let current_id = self.state.current_id.clone().unwrap_or_default();
            return Ok(Reference::with_behavior(current_id, reference.invalid_behavior()).into());
        }
        self.parent_process_value(container, value, is_root)
    }
}

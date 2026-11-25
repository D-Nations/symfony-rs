//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveInstanceofConditionalsPass.php.
//!
//! Applies the `instanceof` conditionals of each definition and, for an
//! autoconfigured one, the container's autoconfigured instanceof
//! definitions: the definition becomes a `ChildDefinition` of a chain of
//! abstract `.instanceof.*` parents, and gets their tags, method calls
//! and bindings. `is_subclass_of()`, `class_parents()` and
//! `class_implements()` read the class metadata; `DeepCloner` is
//! [`super::deep_clone`].
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveInstanceofConditionalsPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use super::deep_clone::deep_clone_definition;
use crate::class_metadata::ClassRegistry;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;

const BEHAVIOR_DESCRIBING_TAGS: &str = "container.behavior_describing_tags";

/// `ResolveInstanceofConditionalsPass`.
#[derive(Debug, Clone, Default)]
pub struct ResolveInstanceofConditionalsPass;

impl ResolveInstanceofConditionalsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveInstanceofConditionalsPass";

    pub fn new() -> Self {
        Self
    }
}

/// `is_subclass_of($class, $parent)`: a subclass or an implementation,
/// not the class itself.
fn is_subclass_of(classes: &ClassRegistry, class: &str, parent: &str) -> bool {
    !class
        .trim_start_matches('\\')
        .eq_ignore_ascii_case(parent.trim_start_matches('\\'))
        && classes.is_a(class, parent)
}

/// `\count(class_parents($c) ?: []) + \count(class_implements($c) ?: [])`.
fn depth(classes: &ClassRegistry, class: &str) -> usize {
    let Ok(Some(metadata)) = classes.reflect(class) else {
        return 0;
    };
    let mut parents = 0;
    let mut current = metadata.parent.clone();
    while let Some(name) = current {
        parents += 1;
        current = classes.class(&name).and_then(|c| c.parent.clone());
    }
    parents + classes.interface_names(&metadata).len()
}

/// `strtr($interface, "\0\r\n", '---')`.
fn dashes(interface: &str) -> String {
    interface.replace(['\0', '\r', '\n'], "-")
}

impl CompilerPassInterface for ResolveInstanceofConditionalsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        for (interface, definition) in container.get_autoconfigured_instanceof().iter() {
            if !definition.arguments().is_empty() {
                return Err(Error::invalid_argument(format!(
                    "Autoconfigured instanceof for type \"{interface}\" defines arguments but these are not supported and should be removed."
                )));
            }
        }

        let mut tags_to_keep: Vec<Value> = Vec::new();

        if container.has_parameter(BEHAVIOR_DESCRIBING_TAGS)
            && let Value::Array(tags) = container.get_parameter(BEHAVIOR_DESCRIBING_TAGS)?
        {
            tags_to_keep = tags.values().cloned().collect();
        }

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            let processed = process_definition(container, &id, definition, &tags_to_keep)?;
            container.set_definition(&id, processed)?;
        }

        if container.has_parameter(BEHAVIOR_DESCRIBING_TAGS) {
            container
                .get_parameter_bag_mut()
                .remove(BEHAVIOR_DESCRIBING_TAGS)?;
        }
        Ok(())
    }
}

/// `processDefinition($container, $id, $definition, $tagsToKeep)`.
#[expect(clippy::too_many_lines, reason = "one upstream method")]
fn process_definition(
    container: &mut ContainerBuilder,
    id: &str,
    mut definition: Definition,
    tags_to_keep: &[Value],
) -> Result<Definition> {
    let instanceof_conditionals = definition.instanceof_conditionals();
    let autoconfigured_instanceof = if definition.is_autoconfigured() {
        container.get_autoconfigured_instanceof().clone()
    } else {
        IdMap::new()
    };
    if instanceof_conditionals.is_empty() && autoconfigured_instanceof.is_empty() {
        return Ok(definition);
    }

    let class = definition
        .class()
        .map_or(Value::Null, |c| Value::from(c.as_str()));
    let class = container.get_parameter_bag().resolve_value(&class, &[])?;
    if !class.is_truthy() {
        return Ok(definition);
    }
    let class = class
        .to_php_string_or_stringable()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let conditionals = merge_conditionals(
        &autoconfigured_instanceof,
        &instanceof_conditionals,
        container,
    )?;

    definition.set_instanceof_conditionals(PhpArray::new());
    let mut shared: Option<bool> = None;
    let mut instanceof_tags: Vec<(String, PhpArray)> = Vec::new();
    let mut instanceof_calls: Vec<Value> = Vec::new();
    let mut instanceof_bindings = PhpArray::new();
    let mut reflection_class: Option<bool> = None;
    let mut parent: Option<String> = definition.parent();
    let classes = container.class_registry();

    for (interface, instanceof_defs) in conditionals.iter() {
        if interface != class {
            if reflection_class.is_none() {
                reflection_class = Some(
                    container
                        .get_reflection_class(Some(&class), false)?
                        .is_some(),
                );
            }
            if reflection_class != Some(true) {
                continue;
            }
        }

        if interface != class && !is_subclass_of(&classes, &class, interface) {
            continue;
        }

        for (key, instanceof_def) in instanceof_defs.iter().enumerate() {
            let instanceof_def = instanceof_def.clone_object();
            let abstract_parent = format!(".abstract.instanceof.{id}");
            let new_parent = match parent.as_deref() {
                Some(p) if !p.is_empty() && p != "0" => p.to_string(),
                _ => abstract_parent,
            };
            instanceof_def.set_abstract(true).set_parent(&new_parent);
            let next = format!(".instanceof.{}.{key}.{id}", dashes(interface));
            container.set_definition(&next, instanceof_def.clone())?;
            parent = Some(next);
            instanceof_tags.push((interface.to_string(), instanceof_def.tags()));
            let mut bindings = instanceof_def.bindings();
            bindings.union(&instanceof_bindings);
            instanceof_bindings = bindings;

            for method_call in instanceof_def.method_calls().values() {
                instanceof_calls.push(method_call.clone());
            }

            instanceof_def.set_tags(PhpArray::new());
            instanceof_def.set_method_calls(&PhpArray::new())?;
            instanceof_def.set_bindings(&PhpArray::new());

            if instanceof_def.changes().iter().any(|c| c == "shared") {
                shared = Some(instanceof_def.is_shared());
            }
        }
    }

    if let Some(parent) = parent.filter(|p| !p.is_empty() && p != "0") {
        let bindings = definition.bindings();
        let abstract_ =
            container.set_definition(&format!(".abstract.instanceof.{id}"), definition.clone())?;
        definition.set_bindings(&PhpArray::new());
        definition = deep_clone_definition(&definition)?;
        definition.set_parent(&parent);

        if let Some(shared) = shared
            && !definition.changes().iter().any(|c| c == "shared")
        {
            definition.set_shared(shared);
        }

        // Tags are added from the most specific type to the least specific one
        if instanceof_tags.len() > 1 {
            let mut depths: IdMap<usize> = IdMap::new();
            for (interface, _) in &instanceof_tags {
                if !depths.contains_key(interface) {
                    depths.insert(interface.as_str(), depth(&classes, interface));
                }
            }
            let depth_of = |interface: &str| depths.get(interface).copied().unwrap_or(0);
            instanceof_tags.sort_by_key(|(interface, _)| depth_of(interface));
        }

        // Don't add tags to service decorators
        for (interface, tags) in instanceof_tags.iter().rev() {
            for (k, v) in tags.iter() {
                let name = String::from_utf8_lossy(&k.bytes()).into_owned();
                if definition.decorated_service().is_none()
                    || definition.class().as_deref() == Some(interface.as_str())
                    || tags_to_keep.iter().any(|t| t.identical(&k.to_value()))
                {
                    let Some(list) = v.as_array() else {
                        continue;
                    };
                    for v in list.values() {
                        if definition.has_tag(&name)
                            && definition.tag(&name).values().any(|t| t.identical(v))
                        {
                            continue;
                        }
                        definition.add_tag(&name, v.as_array().cloned().unwrap_or_default());
                    }
                }
            }
        }

        let calls = PhpArray::list(
            instanceof_calls
                .into_iter()
                .chain(definition.method_calls().values().cloned()),
        );
        definition.set_method_calls(&calls)?;
        let mut merged_bindings = bindings;
        merged_bindings.union(&instanceof_bindings);
        definition.set_bindings(&merged_bindings);

        // reset fields with "merge" behavior
        abstract_
            .set_bindings(&PhpArray::new())
            .set_arguments(PhpArray::new())
            .set_method_calls(&PhpArray::new())?
            .set_decorated_service(None, None, 0, InvalidBehavior::Exception)?
            .set_tags(PhpArray::new())
            .set_abstract(true);
    }

    if definition.is_synthetic() {
        // Ignore container.excluded tag on synthetic services
        definition.clear_tag("container.excluded");
    }

    Ok(definition)
}

/// `mergeConditionals($autoconfiguredInstanceof, $instanceofConditionals, $container)`.
fn merge_conditionals(
    autoconfigured_instanceof: &IdMap<Definition>,
    instanceof_conditionals: &PhpArray,
    container: &mut ContainerBuilder,
) -> Result<IdMap<Vec<Definition>>> {
    // make each value an array of ChildDefinition
    let mut conditionals: IdMap<Vec<Definition>> = IdMap::new();
    for (interface, child_def) in autoconfigured_instanceof.iter() {
        conditionals.insert(interface, vec![child_def.clone()]);
    }

    for (interface, instanceof_def) in instanceof_conditionals.iter() {
        let interface = String::from_utf8_lossy(&interface.bytes()).into_owned();
        // make sure the interface/class exists (but don't validate automaticInstanceofConditionals)
        if container
            .get_reflection_class(Some(&interface), true)?
            .is_none()
        {
            return Err(Error::runtime(format!(
                "\"{interface}\" is set as an \"instanceof\" conditional, but it does not exist."
            )));
        }

        if !autoconfigured_instanceof.contains_key(&interface) {
            conditionals.insert(interface.as_str(), Vec::new());
        }

        if let Some(definition) = Definition::from_value(instanceof_def) {
            conditionals
                .get_or_insert_with(&interface, Vec::new)
                .push(definition);
        }
    }

    Ok(conditionals)
}

#[cfg(test)]
mod tests {
    use super::dashes;

    #[test]
    fn control_characters_become_dashes() {
        assert_eq!("a-b-c-", dashes("a\0b\rc\n"));
    }
}

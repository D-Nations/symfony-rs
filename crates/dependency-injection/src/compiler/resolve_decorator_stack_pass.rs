//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveDecoratorStackPass.php.
//!
//! Turns each definition tagged `container.stack` (a `ChildDefinition`
//! whose arguments are the decorators, outermost first) into a chain of
//! decorating definitions and an alias to the outermost one. A stack
//! tagged `container.tag_decorator` is cloned once per service of the
//! tag it decorates. `DeepCloner` is [`super::deep_clone`].
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveDecoratorStackPass.php

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use super::deep_clone::{deep_clone, deep_clone_definition};
use crate::alias::Alias;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `ResolveDecoratorStackPass`.
#[derive(Debug, Clone, Default)]
pub struct ResolveDecoratorStackPass;

impl ResolveDecoratorStackPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveDecoratorStackPass";

    pub fn new() -> Self {
        Self
    }
}

/// An `int` tag attribute (`$tag['priority'] ?? 0`).
fn int_attribute(tag: &PhpArray, name: &str, default: i64) -> i64 {
    match tag.get(&Key::from(name)) {
        Some(Value::Int(i)) => *i,
        Some(Value::Bool(b)) => i64::from(*b),
        Some(Value::String(s)) => php_value::numeric::string_to_int(s.as_bytes()),
        #[expect(clippy::cast_possible_truncation, reason = "PHP's (int) of a float")]
        Some(Value::Float(f)) => *f as i64,
        _ => default,
    }
}

/// `setDecoratedService($id, $renamedId, $priority, $invalidBehavior)` of
/// a decorated service read back.
fn copy_decoration(from: &crate::definition::DecoratedService, to: &Definition) -> Result<()> {
    to.set_decorated_service(
        Some(&from.id),
        from.renamed_id.as_deref(),
        from.priority,
        from.invalid_behavior.unwrap_or(InvalidBehavior::Exception),
    )?;
    Ok(())
}

impl CompilerPassInterface for ResolveDecoratorStackPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let mut stacks: IdMap<PhpArray> = IdMap::new();

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            if !definition.has_tag("container.stack") {
                continue;
            }

            if !definition.is_child() {
                return Err(Error::invalid_argument(format!(
                    "Invalid service \"{id}\": only definitions with a \"parent\" can have the \"container.stack\" tag."
                )));
            }

            let stack = definition.arguments();
            if stack.is_empty() {
                return Err(Error::invalid_argument(format!(
                    "Invalid service \"{id}\": the stack of decorators is empty."
                )));
            }

            stacks.insert(id.as_str(), stack.clone());
            let mut has_tag_decorator = false;

            for tag in definition.tag("container.tag_decorator").values() {
                let tag = tag.as_array().cloned().unwrap_or_default();
                let decorates_tag = tag
                    .get(&Key::from("decorates_tag"))
                    .cloned()
                    .unwrap_or(Value::Null);
                if !decorates_tag.is_truthy() {
                    continue;
                }
                let decorates_tag = decorates_tag
                    .to_php_string()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

                if definition.decorated_service().is_some() {
                    return Err(Error::invalid_argument(format!(
                        "Service stack \"{id}\" cannot have both \"decorates\" and \"decorates_tag\"."
                    )));
                }

                let priority = int_attribute(&tag, "priority", 0);
                let on_invalid =
                    int_attribute(&tag, "on_invalid", InvalidBehavior::Exception.as_int());
                let invalid_behavior =
                    InvalidBehavior::from_int(on_invalid).unwrap_or(InvalidBehavior::Exception);

                let tagged_services = container.find_tagged_service_ids(&decorates_tag, false)?;
                if tagged_services.is_empty() {
                    if on_invalid == InvalidBehavior::Ignore.as_int() {
                        continue;
                    }

                    if on_invalid == InvalidBehavior::Exception.as_int() {
                        return Err(Error::service_not_found(
                            decorates_tag,
                            Some(id.clone()),
                            Vec::new(),
                            None,
                        ));
                    }
                }
                has_tag_decorator = true;

                for tagged_service_id in tagged_services.keys() {
                    let clone_id = format!(".stack.{tagged_service_id}.{id}");
                    let clone =
                        container.set_definition(&clone_id, deep_clone_definition(&definition)?)?;
                    clone
                        .clear_tag("container.tag_decorator")
                        .clear_tag("container.excluded")
                        .set_decorated_service(
                            Some(tagged_service_id),
                            None,
                            priority,
                            invalid_behavior,
                        )?;
                    let stack_clone = deep_clone(&Value::Array(stack.clone()))?;
                    stacks.insert(
                        clone_id,
                        stack_clone.as_array().cloned().unwrap_or_default(),
                    );
                }
            }

            if has_tag_decorator {
                container.remove_definition(&id);
                stacks.remove(&id);
            }
        }

        if stacks.is_empty() {
            return Ok(());
        }

        let mut resolved_definitions: IdMap<Definition> = IdMap::new();

        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            if !stacks.contains_key(&id) {
                resolved_definitions.insert(id, definition);
                continue;
            }

            let resolved = resolve_stack(&stacks, vec![id.clone()])?;

            if let Some(decorated_service) = definition.decorated_service() {
                // Propagate decoration to the innermost definition in the stack,
                // so that DecoratorServicePass can wire it to the original service.
                if let Some((_, last)) = resolved.iter().last() {
                    copy_decoration(&decorated_service, last)?;
                }
            }

            let entries: Vec<(String, Definition)> = resolved
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
            let mut k = String::new();
            for (key, v) in entries.into_iter().rev() {
                resolved_definitions.insert(key.as_str(), v);
                k = key;
            }

            let alias = container.set_alias(&id, k)?;

            if definition.changes().iter().any(|c| c == "public") {
                alias.set_public(definition.is_public());
            }

            if let Some(deprecation) = definition.deprecation("%alias_id%") {
                alias.set_deprecated(
                    &deprecation.package,
                    &deprecation.version,
                    &deprecation.message,
                )?;
            }
        }

        container.set_definitions(
            resolved_definitions
                .iter()
                .map(|(id, d)| (id.to_string(), d.clone()))
                .collect::<Vec<_>>(),
        )
    }
}

/// `resolveStack($stacks, $path)`.
fn resolve_stack(stacks: &IdMap<PhpArray>, mut path: Vec<String>) -> Result<IdMap<Definition>> {
    let mut definitions: IdMap<Definition> = IdMap::new();
    let id = path.last().cloned().unwrap_or_default();
    let prefix = format!(".{id}.");

    let Some(stack) = stacks.get(&id) else {
        let mut single = IdMap::new();
        single.insert(id.as_str(), Definition::child(&id));
        return Ok(single);
    };

    if let Some(search_key) = path.iter().position(|p| *p == id)
        && search_key != path.len() - 1
    {
        return Err(Error::service_circular_reference(
            id,
            path[search_key..].to_vec(),
        ));
    }

    let mut decorated_id = String::new();
    for (k, value) in stack.iter() {
        let k = String::from_utf8_lossy(&k.bytes()).into_owned();
        let mut definition: Option<Definition> = None;
        if let Some(d) = Definition::from_value(value)
            && let Some(parent) = d.parent()
            && stacks.contains_key(&parent)
        {
            path.push(parent);
            definition = Definition::from_value(&deep_clone(value)?);
        } else if let Some(d) = Definition::from_value(value) {
            decorated_id = format!("{prefix}{k}");
            definitions.insert(decorated_id.as_str(), d);
            continue;
        } else if let Some(reference) = Reference::from_value(value) {
            path.push(reference.id().to_string());
        } else if let Some(alias) = Alias::from_value(value) {
            path.push(alias.id());
        } else {
            return Err(Error::invalid_argument(format!(
                "Invalid service \"{id}\": unexpected value of type \"{}\" found in the stack of decorators.",
                value.debug_type()
            )));
        }

        let p = format!("{prefix}{k}");

        let inner = resolve_stack(stacks, path.clone())?;
        for (k, _) in inner.iter() {
            decorated_id = format!("{p}{k}");
            let entry = match definition.take() {
                Some(child) => {
                    child.set_parent(k);
                    child
                }
                None => Definition::child(k),
            };
            definitions.insert(decorated_id.as_str(), entry);
        }
        path.pop();
    }

    if path.len() == 1 {
        let mut last = None;
        for (_, definition) in definitions.iter() {
            definition
                .set_public(false)
                .set_tags(PhpArray::new())
                .set_decorated_service(Some(&decorated_id), None, 0, InvalidBehavior::Exception)?;
            last = Some(definition.clone());
        }
        if let Some(last) = last {
            last.set_decorated_service(None, None, 0, InvalidBehavior::Exception)?;
        }
    }

    Ok(definitions)
}

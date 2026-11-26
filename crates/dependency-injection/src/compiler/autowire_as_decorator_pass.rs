//! Port of Symfony\Component\DependencyInjection\Compiler\AutowireAsDecoratorPass.php:
//! reads `#[AsDecorator]` and `#[AsTagDecorator]` attributes on
//! definitions that are autowired and don't have the
//! `container.ignore_attributes` tag.
//!
//! The attributes come from the class metadata; their arguments are the
//! constructor's (`decorates`/`tag`, `priority`, `onInvalid`), by
//! position or by name.
//!
//! Upstream: symfony/dependency-injection Compiler/AutowireAsDecoratorPass.php

use php_value::{PhpArray, Value, array::Key};

use super::autowire_required_properties_pass::attributes_of;
use super::compiler_pass_interface::CompilerPassInterface;
use crate::class_metadata::AttributeMetadata;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::exception::{Error, Result};

/// `AsDecorator::class`.
pub const AS_DECORATOR: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsDecorator";
/// `AsTagDecorator::class`.
pub const AS_TAG_DECORATOR: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AsTagDecorator";

/// `AutowireAsDecoratorPass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutowireAsDecoratorPass;

/// The `new AsDecorator(...)` / `new AsTagDecorator(...)` properties:
/// the first argument, `priority` and `onInvalid`.
struct Decoration {
    target: String,
    priority: i64,
    on_invalid: i64,
}

fn decoration(attribute: &AttributeMetadata, first: &str) -> Result<Decoration> {
    let argument = |position: i64, name: &str| {
        attribute
            .arguments
            .get(&Key::from(name))
            .or_else(|| attribute.arguments.get(&Key::Int(position)))
            .cloned()
    };
    let Some(target) = argument(0, first)
        .and_then(|v| v.to_php_string().map(|s| s.to_string_lossy().into_owned()))
    else {
        return Err(Error::logic(format!(
            "The attribute \"{}\" needs its \"${first}\" argument.",
            attribute.class
        )));
    };
    let int = |v: Option<Value>, default: i64| match v {
        Some(Value::Int(i)) => i,
        _ => default,
    };
    Ok(Decoration {
        target,
        priority: int(argument(1, "priority"), 0),
        on_invalid: int(
            argument(2, "onInvalid"),
            InvalidBehavior::Exception.as_int(),
        ),
    })
}

impl CompilerPassInterface for AutowireAsDecoratorPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AutowireAsDecoratorPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let definitions: Vec<_> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        let registry = container.class_registry();
        for (id, definition) in definitions {
            if definition.has_tag("container.ignore_attributes") || !definition.is_autowired() {
                continue;
            }
            let class = definition.class();
            let Some(reflection_class) = container.get_reflection_class(class.as_deref(), false)?
            else {
                continue;
            };

            let decorator_attributes: Vec<_> =
                attributes_of(&registry, &reflection_class.attributes, AS_DECORATOR, true)
                    .collect();
            let tag_decorator_attributes: Vec<_> = attributes_of(
                &registry,
                &reflection_class.attributes,
                AS_TAG_DECORATOR,
                true,
            )
            .collect();

            if decorator_attributes.is_empty() && tag_decorator_attributes.is_empty() {
                continue;
            }

            if decorator_attributes.len() == 1 && tag_decorator_attributes.is_empty() {
                let attribute = decoration(decorator_attributes[0], "decorates")?;
                definition.set_decorated_service(
                    Some(&attribute.target),
                    None,
                    attribute.priority,
                    invalid_behavior(attribute.on_invalid),
                )?;
                continue;
            }

            for attribute in decorator_attributes {
                let attribute = decoration(attribute, "decorates")?;

                let cloned_definition = definition.clone_object();
                cloned_definition.set_decorated_service(
                    Some(&attribute.target),
                    None,
                    attribute.priority,
                    invalid_behavior(attribute.on_invalid),
                )?;
                container.set_definition(
                    &format!(".decorator.{}.{id}", attribute.target),
                    cloned_definition,
                )?;
            }

            for attribute in tag_decorator_attributes {
                let attribute = decoration(attribute, "tag")?;

                let cloned_definition = definition.clone_object();
                let mut tag_attributes = PhpArray::new();
                tag_attributes.insert("decorates_tag", Value::from(attribute.target.as_str()));
                tag_attributes.insert("priority", Value::Int(attribute.priority));

                if InvalidBehavior::Exception.as_int() != attribute.on_invalid {
                    tag_attributes.insert("on_invalid", Value::Int(attribute.on_invalid));
                }

                cloned_definition.add_resource_tag("container.tag_decorator", tag_attributes);
                container.set_definition(
                    &format!(".tag_decorator.{}.{id}", attribute.target),
                    cloned_definition,
                )?;
            }

            container.remove_definition(&id);
        }
        Ok(())
    }
}

/// The `ContainerInterface` constant; another integer is the default.
fn invalid_behavior(value: i64) -> InvalidBehavior {
    InvalidBehavior::from_int(value).unwrap_or_default()
}

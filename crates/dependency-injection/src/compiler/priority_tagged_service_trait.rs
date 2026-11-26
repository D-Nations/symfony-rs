//! Port of Symfony\Component\DependencyInjection\Compiler\PriorityTaggedServiceTrait.php:
//! finds the services with a tag and orders them by priority, keeping
//! the order of addition for equal priorities.
//!
//! The trait's private method is the function
//! [`find_and_sort_tagged_services`]; `PriorityTaggedServiceUtil` is
//! [`get_default`]. `#[AsTaggedItem]` attributes come from the class
//! metadata. A default index or priority method is a static method that
//! upstream calls: its result is the method's
//! [`MethodMetadata::returns`](crate::class_metadata::MethodMetadata::returns),
//! and a method without one is refused.
//!
//! Upstream: symfony/dependency-injection Compiler/PriorityTaggedServiceTrait.php

use std::cmp::Ordering;
use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key};
use symfony_deprecation_contracts::trigger_deprecation;

use crate::argument::IteratorArgument;
use crate::class_metadata::{AttributeMetadata, ClassMetadata, ClassRegistry};
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `AsTaggedItem::class`.
pub const AS_TAGGED_ITEM: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsTaggedItem";

/// The `string|TaggedIteratorArgument $tagName` of the trait.
#[derive(Debug, Clone)]
pub enum TagName {
    Name(String),
    Iterator(IteratorArgument),
}

impl From<&str> for TagName {
    fn from(name: &str) -> Self {
        Self::Name(name.to_string())
    }
}

impl From<String> for TagName {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

impl From<IteratorArgument> for TagName {
    fn from(argument: IteratorArgument) -> Self {
        Self::Iterator(argument)
    }
}

impl From<&IteratorArgument> for TagName {
    fn from(argument: &IteratorArgument) -> Self {
        Self::Iterator(argument.clone())
    }
}

/// An attribute's constructor argument, by name or by position
/// (`$attribute->newInstance()->$name`); `None` when not given.
pub fn attribute_argument(
    attribute: &AttributeMetadata,
    position: i64,
    name: &str,
) -> Option<Value> {
    attribute
        .arguments
        .get(&Key::from(name))
        .or_else(|| attribute.arguments.get(&Key::Int(position)))
        .cloned()
}

/// `$r->getAttributes($class)`: the class's attributes of exactly that
/// class.
pub fn class_attributes<'a>(
    class: &'a ClassMetadata,
    attribute_class: &'a str,
) -> impl Iterator<Item = &'a AttributeMetadata> {
    class
        .attributes
        .iter()
        .filter(move |a| same_class(&a.class, attribute_class))
}

fn same_class(a: &str, b: &str) -> bool {
    a.trim_start_matches('\\')
        .eq_ignore_ascii_case(b.trim_start_matches('\\'))
}

fn to_string(value: &Value) -> Option<String> {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
}

/// `$a <=> $b` for priorities (ints, floats or numeric strings).
fn spaceship(a: &Value, b: &Value) -> Ordering {
    fn number(v: &Value) -> Option<f64> {
        match v {
            #[expect(
                clippy::cast_precision_loss,
                reason = "PHP compares an int and a float as floats"
            )]
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            Value::Null => Some(0.0),
            Value::String(s) => s.to_string_lossy().trim().parse::<f64>().ok(),
            _ => None,
        }
    }
    if let (Value::Int(a), Value::Int(b)) = (a, b) {
        return a.cmp(b);
    }
    match (number(a), number(b)) {
        (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
        _ => to_string(a).cmp(&to_string(b)),
    }
}

/// `$definition->getTag('container.decorator')[0][$key] ?? null`.
fn decorator_tag(definition: &crate::definition::Definition, key: &str) -> Option<Value> {
    let tag = definition.tag("container.decorator");
    match tag.get(&Key::Int(0)) {
        Some(Value::Array(first)) => first.get(&Key::from(key)).filter(|v| !v.is_null()).cloned(),
        _ => None,
    }
}

/// `isset($array[$key])`.
fn isset(array: &PhpArray, key: &str) -> Option<Value> {
    array.get(&Key::from(key)).filter(|v| !v.is_null()).cloned()
}

struct Found {
    priority: Value,
    position: usize,
    index: Option<Value>,
    service_id: String,
    class: Option<String>,
}

/// `findAndSortTaggedServices($tagName, $container, $exclude)`: the
/// references, a list, or keyed by index when the tag is indexed.
#[expect(clippy::too_many_lines, reason = "one upstream method")]
pub fn find_and_sort_tagged_services(
    tag_name: impl Into<TagName>,
    container: &mut ContainerBuilder,
    exclude: &[String],
) -> Result<PhpArray> {
    let mut exclude = exclude.to_vec();
    let (tag_name, index_attribute, default_index_method, needs_indexes, default_priority_method) =
        match tag_name.into() {
            TagName::Name(name) => (name, None, None, false, None),
            TagName::Iterator(argument) => {
                exclude.extend(argument.exclude());
                (
                    argument.tag().unwrap_or_default(),
                    argument.index_attribute(),
                    argument.default_index_method(false),
                    argument.needs_indexes(),
                    Some(
                        argument
                            .default_priority_method(false)
                            .unwrap_or_else(|| "getDefaultPriority".to_string()),
                    ),
                )
            }
        };

    let registry = container.class_registry();
    let mut services: Vec<Found> = Vec::new();
    let tagged = container.find_tagged_service_ids(&tag_name, true)?;
    for (service_id, attributes) in tagged.iter() {
        if exclude.iter().any(|e| e == service_id) {
            continue;
        }

        let mut default_priority: Option<Value> = None;
        let mut default_attribute_priority: Option<Value> = None;
        let mut default_index: Option<Value> = None;
        let mut default_attribute_index: Option<Value> = None;
        let mut indexes: Vec<Key> = Vec::new();

        let definition = container.get_definition(service_id)?;
        let class = match definition.class() {
            Some(class) => container
                .get_parameter_bag()
                .resolve_value(&Value::from(class), &[])?,
            None => Value::Null,
        };
        let class = if class.is_truthy() {
            to_string(&class)
        } else {
            None
        };
        let reflector = match &class {
            Some(class) => container.get_reflection_class(Some(class), true)?,
            None => None,
        };

        let mut php_attributes: Vec<Value> = Vec::new();
        if definition.is_autoconfigured()
            && !definition.has_tag("container.ignore_attributes")
            && let Some(r) = &reflector
        {
            for attribute in class_attributes(r, AS_TAGGED_ITEM) {
                let index = attribute_argument(attribute, 0, "index").unwrap_or_default();
                let priority = attribute_argument(attribute, 1, "priority").unwrap_or_default();
                let mut entry = PhpArray::new();
                entry.insert("priority", priority.clone());
                entry.insert(index_attribute.as_deref().unwrap_or(""), index.clone());
                php_attributes.push(Value::Array(entry));
                if default_attribute_priority.is_none() {
                    default_attribute_priority = Some(if priority.is_null() {
                        Value::Int(0)
                    } else {
                        priority
                    });
                    default_attribute_index = Some(index).filter(|i| !i.is_null());
                }
            }
        }
        if php_attributes.len() <= 1 {
            php_attributes.clear();
        }

        // For decorated services, walk the decoration chain to find #[AsTaggedItem] on the original service
        let mut inner_class: Option<String> = None;
        let mut inner_definition = definition.clone();
        while let Some(inner_id) =
            decorator_tag(&inner_definition, "inner").filter(Value::is_truthy)
        {
            let inner_id = to_string(&inner_id).unwrap_or_default();
            if !container.has(&inner_id) {
                break;
            }
            inner_definition = container.find_definition(&inner_id)?;
            let resolved = match inner_definition.class() {
                Some(class) => container
                    .get_parameter_bag()
                    .resolve_value(&Value::from(class), &[])?,
                None => Value::Null,
            };
            inner_class = if resolved.is_truthy() {
                to_string(&resolved)
            } else {
                None
            };
        }
        let inner_reflector = match &inner_class {
            Some(class) => container.get_reflection_class(Some(class), true)?,
            None => None,
        };

        let mut attributes: Vec<Value> = attributes.values().cloned().collect();
        let mut i = 0;
        'attributes: while i < attributes.len() {
            let attribute = attributes[i].clone();
            if !attribute.is_truthy() && !php_attributes.is_empty() {
                attributes.splice(i..=i, php_attributes.iter().cloned());
                continue;
            }
            let attribute = match attribute {
                Value::Array(a) => a,
                _ => PhpArray::new(),
            };
            let mut index: Option<Value> = None;
            let mut priority: Option<Value> = None;

            if let Some(p) = isset(&attribute, "priority") {
                priority = Some(p);
            } else if default_priority.is_none()
                && let (Some(method), Some(r)) = (&default_priority_method, &reflector)
                && !method.is_empty()
            {
                default_priority = get_default(
                    &registry,
                    service_id,
                    r,
                    method,
                    &tag_name,
                    Some("priority"),
                )?
                .or_else(|| default_attribute_priority.clone());
                if default_priority.is_none()
                    && let Some(inner) = &inner_reflector
                {
                    default_priority = get_default(
                        &registry,
                        service_id,
                        inner,
                        method,
                        &tag_name,
                        Some("priority"),
                    )?;
                }
            }
            let priority = match priority {
                Some(p) => p,
                None => default_priority.get_or_insert(Value::Int(0)).clone(),
            };

            if index_attribute.is_none()
                && !default_index_method
                    .as_deref()
                    .is_some_and(|m| !m.is_empty() && m != "0")
                && !needs_indexes
            {
                services.push(Found {
                    priority,
                    position: i,
                    index: None,
                    service_id: service_id.to_string(),
                    class: None,
                });
                break 'attributes;
            }

            if let Some(attr) = &index_attribute
                && let Some(value) = isset(&attribute, attr)
            {
                index = Some(container.get_parameter_bag().resolve_value(&value, &[])?)
                    .filter(|v| !v.is_null());
            }
            if index.is_none()
                && default_index.is_none()
                && default_priority_method
                    .as_deref()
                    .is_some_and(|m| !m.is_empty())
                && let Some(r) = &reflector
            {
                let method = default_index_method
                    .clone()
                    .unwrap_or_else(|| "getDefaultName".to_string());
                default_index = get_default(
                    &registry,
                    service_id,
                    r,
                    &method,
                    &tag_name,
                    index_attribute.as_deref(),
                )?
                .or_else(|| default_attribute_index.clone());
                if default_index.is_none()
                    && let Some(inner) = &inner_reflector
                {
                    default_index = get_default(
                        &registry,
                        service_id,
                        inner,
                        &method,
                        &tag_name,
                        index_attribute.as_deref(),
                    )?;
                    if default_index.is_none() {
                        default_index = class_attributes(inner, AS_TAGGED_ITEM)
                            .next()
                            .and_then(|a| attribute_argument(a, 0, "index"))
                            .filter(|v| !v.is_null());
                    }
                }
            }
            let index = match index {
                Some(index) => index,
                None => default_index
                    .get_or_insert_with(|| {
                        decorator_tag(&definition, "id").unwrap_or_else(|| Value::from(service_id))
                    })
                    .clone(),
            };

            let key = Key::from_value(&index).ok_or_else(|| {
                Error::new(
                    crate::exception::ErrorKind::Other("TypeError".to_string()),
                    format!(
                        "Cannot access offset of type {} on array",
                        index.debug_type()
                    ),
                )
            })?;
            if indexes.contains(&key) {
                i += 1;
                continue;
            }
            indexes.push(key);
            services.push(Found {
                priority,
                position: i,
                index: Some(index),
                service_id: service_id.to_string(),
                class: class.clone(),
            });
            i += 1;
        }
    }

    services.sort_by(|a, b| {
        spaceship(&b.priority, &a.priority).then_with(|| a.position.cmp(&b.position))
    });

    let mut refs = PhpArray::new();
    for found in services {
        let reference = match (&found.class, &found.index) {
            (None, _) => Reference::new(found.service_id.clone()),
            (Some(class), Some(Value::String(index)))
                if index.as_bytes() == found.service_id.as_bytes() =>
            {
                Reference::typed(
                    found.service_id.clone(),
                    class.clone(),
                    InvalidBehavior::Exception,
                    None,
                    PhpArray::new(),
                )
            }
            (Some(class), index) => Reference::typed(
                found.service_id.clone(),
                class.clone(),
                InvalidBehavior::Exception,
                index.as_ref().and_then(to_string),
                PhpArray::new(),
            ),
        };
        match found.index {
            None => {
                // `$refs[] = $reference`: a full array cannot happen here.
                let _ = refs.push(Value::from(reference));
            }
            Some(index) => {
                if let Some(key) = Key::from_value(&index) {
                    refs.insert(key, Value::from(reference));
                }
            }
        }
    }
    Ok(refs)
}

/// `PriorityTaggedServiceUtil::getDefault($serviceId, $r,
/// $defaultMethod, $tagName, $indexAttribute)`: the value the static
/// method returns, checked.
pub fn get_default(
    registry: &ClassRegistry,
    service_id: &str,
    r: &Arc<ClassMetadata>,
    default_method: &str,
    tag_name: &str,
    index_attribute: Option<&str>,
) -> Result<Option<Value>> {
    if r.is_interface() {
        return Ok(None);
    }
    let class = r.name.clone();
    let message: [String; 2] = if let Some(attribute) = index_attribute {
        let service = if class == service_id {
            "on the corresponding service".to_string()
        } else {
            format!("service \"{service_id}\"")
        };
        [
            format!("Either method \"{class}::{default_method}()\" should "),
            format!(" or tag \"{tag_name}\" on {service} is missing attribute \"{attribute}\"."),
        ]
    } else {
        [
            format!("Method \"{class}::{default_method}()\" should "),
            ".".to_string(),
        ]
    };
    let implode = |middle: &str| format!("{}{middle}{}", message[0], message[1]);

    // `$r->hasMethod()` and `$r->getMethod()`, through the class chain.
    let Some((_, rm)) = registry.find_method(r, default_method) else {
        return Ok(None);
    };
    if !rm.is_static {
        return Err(Error::invalid_argument(implode("be static")));
    }
    if !rm.is_public() {
        return Err(Error::invalid_argument(implode("be public")));
    }

    trigger_deprecation(
        b"symfony/dependency-injection",
        b"8.1",
        format!(
            "Calling \"{class}::{default_method}()\" to get the \"{}\" index is deprecated, use the #[AsTaggedItem] attribute instead.",
            index_attribute.unwrap_or("")
        )
        .as_bytes(),
    );
    let Some(default) = rm.returns.clone() else {
        return Err(Error::logic(format!(
            "Calling \"{class}::{default_method}()\" needs PHP code to run, which this build-time port does not do; the class metadata must give its return value."
        )));
    };

    if index_attribute == Some("priority") {
        if !matches!(default, Value::Int(_)) {
            return Err(Error::invalid_argument(implode(&format!(
                "return int (got \"{}\")",
                default.debug_type()
            ))));
        }
        return Ok(Some(default));
    }
    let default = match default {
        Value::Int(i) => Value::from(i.to_string()),
        other => other,
    };
    if !matches!(default, Value::String(_)) {
        return Err(Error::invalid_argument(implode(&format!(
            "return string|int (got \"{}\")",
            default.debug_type()
        ))));
    }
    Ok(Some(default))
}

//! Port of Symfony\Component\DependencyInjection\Compiler\AutowireRequiredPropertiesPass.php:
//! looks for definitions with autowiring enabled and registers their
//! corresponding `#[Required]` properties.
//!
//! `getProperties()` reads the class metadata: a class's properties come
//! first, then the non-private ones it inherits. A readonly property
//! without a declared set visibility is `protected(set)`, as in PHP.
//!
//! `$attribute->newInstance()` is [`new_attribute_instance`]: it builds
//! `#[Target]`, `#[AutowireDecorated]` and `#[Autowire]` objects as their
//! constructors do (an object of the attribute class with its public
//! properties) and refuses any other class, whose constructor is PHP code.
//!
//! Upstream: symfony/dependency-injection Compiler/AutowireRequiredPropertiesPass.php

use php_value::object::GenericObject;
use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::autowire_required_methods_pass::REQUIRED;
use super::compiler_pass_interface::CompilerPassInterface;
use crate::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, PropertyMetadata, Visibility,
};
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `Autowire::class`.
pub const AUTOWIRE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Autowire";
/// `AutowireDecorated::class`.
pub const AUTOWIRE_DECORATED: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireDecorated";
/// `Target::class`.
pub const TARGET: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Target";

/// `AutowireRequiredPropertiesPass`.
#[derive(Debug, Clone)]
pub struct AutowireRequiredPropertiesPass {
    state: RecursivePassState,
}

impl Default for AutowireRequiredPropertiesPass {
    fn default() -> Self {
        Self::new()
    }
}

impl AutowireRequiredPropertiesPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

fn key(class: &str) -> String {
    class.trim_start_matches('\\').to_ascii_lowercase()
}

/// `$r->getAttributes($class)`, or with `\ReflectionAttribute::IS_INSTANCEOF`
/// when `instance_of` is set.
pub fn attributes_of<'a>(
    registry: &'a ClassRegistry,
    attributes: &'a [AttributeMetadata],
    class: &'a str,
    instance_of: bool,
) -> impl Iterator<Item = &'a AttributeMetadata> {
    attributes.iter().filter(move |a| {
        key(&a.class) == key(class) || (instance_of && registry.is_a(&a.class, class))
    })
}

/// `$r->getProperties()`.
pub fn class_properties(registry: &ClassRegistry, class: &ClassMetadata) -> Vec<PropertyMetadata> {
    let mut properties: Vec<PropertyMetadata> = class.properties.clone();
    let mut current = class.parent.as_deref().and_then(|p| registry.class(p));
    while let Some(c) = current {
        for property in &c.properties {
            if property.visibility != Visibility::Private
                && !properties.iter().any(|p| p.name == property.name)
            {
                properties.push(property.clone());
            }
        }
        current = c.parent.as_deref().and_then(|p| registry.class(p));
    }
    properties
}

/// `$type instanceof \ReflectionNamedType ? $type->getName() : null`.
pub fn named_type(type_: Option<&str>) -> Option<&str> {
    let type_ = type_?.trim();
    if type_.contains(['|', '&', '(']) {
        return None;
    }
    Some(type_.strip_prefix('?').unwrap_or(type_))
}

fn argument(attribute: &AttributeMetadata, position: i64, name: &str) -> Option<Value> {
    attribute
        .arguments
        .get(&Key::from(name))
        .or_else(|| attribute.arguments.get(&Key::Int(position)))
        .filter(|v| !v.is_null())
        .cloned()
}

fn string_argument(attribute: &AttributeMetadata, position: i64, name: &str) -> Option<String> {
    argument(attribute, position, name)
        .and_then(|v| v.to_php_string().map(|s| s.to_string_lossy().into_owned()))
}

/// `$attribute->newInstance()` for the attributes the compiler passes read.
pub fn new_attribute_instance(attribute: &AttributeMetadata) -> Result<Value> {
    let class = attribute.class.trim_start_matches('\\');
    let object = |properties: PhpArray| {
        Value::Object(std::sync::Arc::new(GenericObject::new(class, properties)))
    };
    if key(class) == key(TARGET) {
        let mut properties = PhpArray::new();
        properties.insert(
            "name",
            argument(attribute, 0, "name").unwrap_or(Value::Null),
        );
        return Ok(object(properties));
    }
    if key(class) == key(AUTOWIRE_DECORATED) {
        return Ok(object(PhpArray::new()));
    }
    if key(class) != key(AUTOWIRE) {
        return Err(Error::logic(format!(
            "Instantiating the attribute \"{class}\" needs PHP code to run, which this build-time port does not do."
        )));
    }

    let mut value = argument(attribute, 0, "value");
    let mut service = string_argument(attribute, 1, "service");
    let mut expression = string_argument(attribute, 2, "expression");
    let env = string_argument(attribute, 3, "env");
    let param = string_argument(attribute, 4, "param");
    let lazy = match argument(attribute, 5, "lazy") {
        Some(Value::String(s)) => Value::Array(PhpArray::list([Value::String(s)])),
        Some(lazy) => lazy,
        None => Value::Bool(false),
    };

    if lazy.is_truthy() {
        if expression.is_some() || env.is_some() || param.is_some() {
            return Err(Error::logic(
                "#[Autowire] attribute cannot be $lazy and use $expression, $env, or $param.",
            ));
        }
        if value.is_some() && service.is_some() {
            return Err(Error::logic(
                "#[Autowire] attribute cannot declare $value and $service at the same time.",
            ));
        }
    } else if 1
        != [
            value.is_some(),
            service.is_some(),
            expression.is_some(),
            env.is_some(),
            param.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
    {
        return Err(Error::logic(
            "#[Autowire] attribute must declare exactly one of $service, $expression, $env, $param or $value.",
        ));
    }

    if let Some(Value::String(s)) = &value {
        let s = s.to_string_lossy().into_owned();
        if let Some(rest) = s.strip_prefix("@@") {
            value = Some(Value::from(format!("@{rest}")));
        } else if let Some(rest) = s.strip_prefix("@=") {
            expression = Some(rest.to_string());
        } else if let Some(rest) = s.strip_prefix('@') {
            service = Some(rest.to_string());
        }
    }

    let value = if let Some(service) = service {
        Value::from(Reference::new(service))
    } else if let Some(expression) = expression {
        return Err(Error::logic(format!(
            "Cannot use the expression \"{expression}\" of a #[Autowire] attribute: this port has no ExpressionLanguage."
        )));
    } else if let Some(env) = env {
        Value::from(format!("%env({env})%"))
    } else if let Some(param) = param {
        Value::from(format!("%{param}%"))
    } else {
        value.unwrap_or(Value::Null)
    };

    let mut properties = PhpArray::new();
    properties.insert("value", value);
    properties.insert("lazy", lazy);
    Ok(object(properties))
}

impl CompilerPassInterface for AutowireRequiredPropertiesPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AutowireRequiredPropertiesPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for AutowireRequiredPropertiesPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let value = self.parent_process_value(container, value, is_root)?;

        let Some(definition) = Definition::from_value(&value) else {
            return Ok(value);
        };
        let Some(class) = definition.class().filter(|c| !c.is_empty() && c != "0") else {
            return Ok(value);
        };
        if !definition.is_autowired() || definition.is_abstract() {
            return Ok(value);
        }
        let Some(reflection_class) = container.get_reflection_class(Some(&class), false)? else {
            return Ok(value);
        };
        let registry = container.class_registry();

        let properties = definition.properties();
        for property in class_properties(&registry, &reflection_class) {
            let Some(type_) = named_type(property.type_.as_deref()) else {
                continue;
            };
            if attributes_of(&registry, &property.attributes, REQUIRED, false)
                .next()
                .is_none()
            {
                continue;
            }
            let name = property.name.as_str();
            if properties.contains_key(&Key::from(name)) {
                continue;
            }
            let set_visibility = property.set_visibility.or(if property.is_readonly {
                Some(Visibility::Protected)
            } else {
                None
            });
            if matches!(
                set_visibility,
                Some(Visibility::Private | Visibility::Protected)
            ) || property.visibility != Visibility::Public
            {
                return Err(Error::invalid_argument(format!(
                    "Cannot autowire non-public(set) property \"{}::${name}\" with #[{REQUIRED}].",
                    reflection_class.name
                )));
            }

            let mut attributes = Vec::new();
            for attribute in attributes_of(&registry, &property.attributes, AUTOWIRE, true)
                .chain(attributes_of(
                    &registry,
                    &property.attributes,
                    AUTOWIRE_DECORATED,
                    false,
                ))
                .chain(attributes_of(
                    &registry,
                    &property.attributes,
                    TARGET,
                    false,
                ))
            {
                attributes.push(new_attribute_instance(attribute)?);
            }
            definition.set_property(
                name,
                Reference::typed(
                    type_,
                    type_,
                    InvalidBehavior::Exception,
                    Some(name.to_string()),
                    PhpArray::list(attributes),
                ),
            );
        }

        Ok(value)
    }
}

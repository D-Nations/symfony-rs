//! Port of Symfony\Component\DependencyInjection\Compiler\RegisterServiceSubscribersPass.php:
//! compiles the `container.service_subscriber` tag into a service
//! locator for each subscriber.
//!
//! A subscriber's static `getSubscribedServices()` is not run: its result
//! is the method's [`MethodMetadata::returns`](crate::class_metadata::MethodMetadata::returns),
//! an array of types, `SubscribedService` objects (any object of that
//! class, read through its `key`, `type`, `nullable` and `attributes`
//! properties) and `#[Autowire]` attribute objects (objects whose class
//! is `Autowire` or a registered subclass of it). A class without one is
//! refused.
//!
//! Upstream: symfony/dependency-injection Compiler/RegisterServiceSubscribersPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::service_locator_tag_pass::ServiceLocatorTagPass;
use crate::argument::BoundArgument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `ServiceSubscriberInterface::class`.
pub const SERVICE_SUBSCRIBER_INTERFACE: &str =
    "Symfony\\Contracts\\Service\\ServiceSubscriberInterface";
/// `SubscribedService::class`.
pub const SUBSCRIBED_SERVICE: &str = "Symfony\\Contracts\\Service\\Attribute\\SubscribedService";
/// `Autowire::class`.
pub const AUTOWIRE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Autowire";

const PSR_CONTAINER_INTERFACE: &str = "Psr\\Container\\ContainerInterface";
const SERVICE_PROVIDER_INTERFACE: &str = "Symfony\\Contracts\\Service\\ServiceProviderInterface";
const SERVICE_COLLECTION_INTERFACE: &str =
    "Symfony\\Contracts\\Service\\ServiceCollectionInterface";

/// `RegisterServiceSubscribersPass`.
#[derive(Debug, Clone)]
pub struct RegisterServiceSubscribersPass {
    state: RecursivePassState,
}

impl Default for RegisterServiceSubscribersPass {
    fn default() -> Self {
        Self::new()
    }
}

impl RegisterServiceSubscribersPass {
    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

fn to_string(value: &Value) -> Option<String> {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
}

fn is_cn_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f
}

fn is_cn(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x7f
}

/// `(?&fqcn)`: `cn(\\cn)*`.
fn is_fqcn(s: &str) -> bool {
    !s.is_empty()
        && s.split('\\').all(|cn| {
            let bytes = cn.as_bytes();
            bytes.first().is_some_and(|&b| is_cn_start(b)) && bytes.iter().all(|&b| is_cn(b))
        })
}

/// The type pattern of `processValue()`: an optional `?`, then class
/// names joined by `|` only or by `&` only.
fn is_valid_type(type_: &str) -> bool {
    let type_ = type_.strip_suffix('\n').unwrap_or(type_);
    let type_ = type_.strip_prefix('?').unwrap_or(type_);
    let separator = if type_.contains('|') { '|' } else { '&' };
    type_.split(separator).all(is_fqcn)
}

/// `lcfirst(str_replace(' ', '', ucwords(preg_replace('/[^a-zA-Z0-9\x7f-\xff]++/', ' ', $name))))`.
fn camel_case(name: &str) -> String {
    let keep = |b: u8| b.is_ascii_alphanumeric() || b >= 0x7f;
    let mut replaced = Vec::new();
    let mut in_run = false;
    for &b in name.as_bytes() {
        if keep(b) {
            replaced.push(b);
            in_run = false;
        } else if !in_run {
            replaced.push(b' ');
            in_run = true;
        }
    }
    let mut out = Vec::new();
    let mut upper = true;
    for b in replaced {
        if b == b' ' {
            upper = true;
            continue;
        }
        out.push(if upper { b.to_ascii_uppercase() } else { b });
        upper = false;
    }
    lcfirst(&String::from_utf8_lossy(&out))
}

fn lcfirst(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    if let Some(first) = bytes.first_mut() {
        first.make_ascii_lowercase();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

impl CompilerPassInterface for RegisterServiceSubscribersPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\RegisterServiceSubscribersPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for RegisterServiceSubscribersPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(definition) = Definition::from_value(&value).filter(|d| {
            !d.is_abstract() && !d.is_synthetic() && d.has_tag("container.service_subscriber")
        }) else {
            return self.parent_process_value(container, value, is_root);
        };
        let current_id = self.state.current_id.clone().unwrap_or_default();

        let mut service_map = PhpArray::new();
        let mut autowire = definition.is_autowired();

        for attributes in definition.tag("container.service_subscriber").values() {
            let mut attributes = match attributes {
                Value::Array(a) => a.clone(),
                _ => PhpArray::new(),
            };
            if attributes.is_empty() {
                autowire = true;
                continue;
            }
            attributes.ksort();
            let keys: Vec<String> = attributes.keys().map(key_string).collect();
            if keys.iter().any(|k| k != "id" && k != "key") {
                return Err(Error::invalid_argument(format!(
                    "The \"container.service_subscriber\" tag accepts only the \"key\" and \"id\" attributes, \"{}\" given for service \"{current_id}\".",
                    keys.join("\", \"")
                )));
            }
            let Some(id) = attributes.get(&Key::from("id")).cloned() else {
                let key = attributes
                    .get(&Key::from("key"))
                    .and_then(to_string)
                    .unwrap_or_default();
                return Err(Error::invalid_argument(format!(
                    "Missing \"id\" attribute on \"container.service_subscriber\" tag with key=\"{key}\" for service \"{current_id}\"."
                )));
            };
            let key = attributes
                .get(&Key::from("key"))
                .cloned()
                .unwrap_or_else(|| id.clone());
            let key = Key::from_value(&key).unwrap_or_else(|| Key::from(""));
            if service_map.get(&key).is_some_and(|v| !v.is_null()) {
                continue;
            }
            service_map.insert(
                key,
                Value::from(Reference::new(to_string(&id).unwrap_or_default())),
            );
        }
        let class = definition.class();

        let registry = container.class_registry();
        let Some(r) = container.get_reflection_class(class.as_deref(), true)? else {
            return Err(Error::invalid_argument(format!(
                "Class \"{}\" used for service \"{current_id}\" cannot be found.",
                class.unwrap_or_default()
            )));
        };
        if r.name.eq_ignore_ascii_case(SERVICE_SUBSCRIBER_INTERFACE)
            || !registry.is_a(&r.name, SERVICE_SUBSCRIBER_INTERFACE)
        {
            return Err(Error::invalid_argument(format!(
                "Service \"{current_id}\" must implement interface \"{SERVICE_SUBSCRIBER_INTERFACE}\"."
            )));
        }
        let class = r.name.clone();
        let mut subscriber_map = PhpArray::new();

        let subscribed = registry
            .find_method(&r, "getSubscribedServices")
            .and_then(|(_, m)| m.returns);
        let Some(Value::Array(subscribed)) = subscribed else {
            return Err(Error::logic(format!(
                "Calling \"{class}::getSubscribedServices()\" needs PHP code to run, which this build-time port does not do; the class metadata must give its return value."
            )));
        };

        for (key, type_) in subscribed.iter() {
            let mut key = key.clone();
            let mut type_ = type_.clone();
            let mut attributes = PhpArray::new();

            let object_class = type_.as_object().map(|o| o.class().to_string());
            if !service_map.contains_key(&key)
                && object_class
                    .as_deref()
                    .is_some_and(|c| registry.is_a(c, AUTOWIRE))
            {
                subscriber_map.insert(key, type_);
                continue;
            }

            if let Some(object) = type_.as_object().filter(|_| {
                object_class
                    .as_deref()
                    .is_some_and(|c| registry.is_a(c, SUBSCRIBED_SERVICE))
            }) {
                let properties = object.properties();
                let property = |name: &str| {
                    properties
                        .get(&Key::from(name))
                        .cloned()
                        .unwrap_or_default()
                };
                if let Some(k) =
                    Key::from_value(&property("key")).filter(|_| !property("key").is_null())
                {
                    key = k;
                }
                attributes = match property("attributes") {
                    Value::Array(a) => a,
                    Value::Null => PhpArray::new(),
                    other => PhpArray::list([other]),
                };
                let Some(t) = to_string(&property("type")).filter(|_| !property("type").is_null())
                else {
                    return Err(Error::invalid_argument(format!(
                        "When \"{class}::getSubscribedServices()\" returns \"{SUBSCRIBED_SERVICE}\", a type must be set."
                    )));
                };
                let nullable = if property("nullable").is_truthy() {
                    "?"
                } else {
                    ""
                };
                type_ = Value::from(format!("{nullable}{t}"));
            }

            let type_string = match &type_ {
                Value::String(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            };
            let Some(mut type_) = type_string.filter(|t| is_valid_type(t)) else {
                let given = match &type_ {
                    Value::String(s) => s.to_string_lossy().into_owned(),
                    other => other.debug_type(),
                };
                return Err(Error::invalid_argument(format!(
                    "\"{class}::getSubscribedServices()\" must return valid PHP types for service \"{current_id}\" key \"{}\", \"{given}\" returned.",
                    key_string(&key)
                )));
            };
            let mut optional_behavior = InvalidBehavior::Exception;
            if let Some(rest) = type_.strip_prefix('?') {
                type_ = rest.to_string();
                optional_behavior = InvalidBehavior::Ignore;
            }
            let mut name = match &key {
                Key::Int(_) => {
                    key = Key::from(type_.as_str());
                    None
                }
                Key::Str(s) => Some(s.to_string_lossy().into_owned()),
            };
            if !service_map.contains_key(&key) {
                if !autowire {
                    return Err(Error::invalid_argument(format!(
                        "Service \"{current_id}\" misses a \"container.service_subscriber\" tag with \"key\"/\"id\" attributes corresponding to entry \"{}\" as returned by \"{class}::getSubscribedServices()\".",
                        key_string(&key)
                    )));
                }
                service_map.insert(key.clone(), Value::from(Reference::new(type_.clone())));
            }

            if let Some(n) = name.clone().filter(|n| !n.is_empty() && n != "0") {
                if let Some(i) = n.find("::get") {
                    name = Some(lcfirst(&n[5 + i..]));
                } else if n.contains("::") {
                    name = None;
                }
            }

            if let Some(n) = name.clone()
                && !container.has(&n)
                && !container.has(&format!("{type_} ${n}"))
            {
                let camel_case_name = camel_case(&n);
                if container.has(&format!("{type_} ${camel_case_name}")) {
                    name = Some(camel_case_name);
                }
            }

            let id = service_map
                .get(&key)
                .and_then(Reference::from_value)
                .map(|r| r.id().to_string())
                .unwrap_or_default();
            subscriber_map.insert(
                key.clone(),
                Value::from(Reference::typed(
                    id,
                    type_,
                    optional_behavior,
                    name,
                    attributes,
                )),
            );
            service_map.remove(&key);
        }

        if !service_map.is_empty() {
            let keys: Vec<String> = service_map.keys().map(key_string).collect();
            let message = if keys.len() > 1 {
                format!("keys \"{}\" do", keys.join("\", \""))
            } else {
                format!("key \"{}\" does", keys.join("\", \""))
            };
            return Err(Error::invalid_argument(format!(
                "Service {message} not exist in the map returned by \"{class}::getSubscribedServices()\" for service \"{current_id}\"."
            )));
        }

        let locator_ref =
            ServiceLocatorTagPass::register(container, &subscriber_map, Some(&current_id))?;

        let mut tag = PhpArray::new();
        tag.insert("id", Value::from(locator_ref.id()));
        definition.add_tag("container.service_subscriber.locator", tag);

        let bound =
            || BoundArgument::new(Value::from(locator_ref.clone()), false, 0, None).to_value();
        let mut bindings = PhpArray::new();
        bindings.insert(PSR_CONTAINER_INTERFACE, bound());
        bindings.insert(SERVICE_PROVIDER_INTERFACE, bound());
        bindings.insert(SERVICE_COLLECTION_INTERFACE, bound());
        bindings.union(&definition.bindings());
        definition.set_bindings(&bindings);

        self.parent_process_value(container, value, false)
    }
}

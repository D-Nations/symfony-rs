//! Port of Symfony\Component\DependencyInjection\Dumper\YamlDumper.php.
//!
//! Values go through symfony-yaml's `Dumper` (4 spaces of indentation)
//! and an inline service is read back with its `Parser`, as upstream
//! does. The Yaml component is always there, so the `LogicException`
//! for a missing one cannot happen.
//!
//! Upstream: symfony/dependency-injection Dumper/YamlDumper.php

use std::sync::Arc;

use std::fmt::Write as _;

use php_value::object::EnumCase;
use php_value::{PhpArray, PhpString, Value, array::Key};
use symfony_yaml::{Flags, Parser, TaggedValue};

use super::xml_dumper::{
    class_identical, default_prefix, is_closure_from_callable, replace_percent,
};
use super::{Dumper, DumperInterface, expression_text, to_php_string};
use crate::alias::Alias;
use crate::argument::{
    AbstractArgument, Argument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind, Result};
use crate::object::downcast;
use crate::reference::{Parameter, Reference};

/// `YamlDumper`: dumps a service container as a YAML string.
#[derive(Debug)]
pub struct YamlDumper<'a> {
    base: Dumper<'a>,
    dumper: symfony_yaml::Dumper,
}

fn yaml_error(error: &symfony_yaml::DumpError) -> Error {
    Error::new(ErrorKind::Other(error.class().to_string()), error.message())
}

fn tagged(tag: &str, value: Value) -> Value {
    Value::Object(Arc::new(TaggedValue::new(tag, value)))
}

impl<'a> YamlDumper<'a> {
    /// `new YamlDumper($container)`.
    pub fn new(container: &'a mut ContainerBuilder) -> Self {
        Self {
            base: Dumper::new(container),
            dumper: symfony_yaml::Dumper::new(4)
                .unwrap_or_else(|_| unreachable!("4 is a valid indentation")),
        }
    }

    /// `dump()`: the container as YAML.
    pub fn dump_yaml(&mut self) -> Result<PhpString> {
        let mut out = self.add_parameters()?;
        out.push(b'\n');
        out.extend(self.add_services()?);
        Ok(PhpString::from(out))
    }

    fn container(&mut self) -> &mut ContainerBuilder {
        self.base.container_mut()
    }

    /// `$this->dumper->dump($input, $inline, $indent)`.
    fn yaml(&self, input: &Value, inline: i64, indent: usize) -> Result<Vec<u8>> {
        self.dumper
            .dump(input, inline, indent, Flags::NONE)
            .map(|s| s.as_bytes().to_vec())
            .map_err(|e| yaml_error(&e))
    }

    fn resolve(&mut self, value: Value) -> Result<Value> {
        self.container()
            .resolve_env_placeholders(value, &EnvFormat::Placeholder, None)
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn add_service(&mut self, id: &[u8], definition: &Definition) -> Result<Vec<u8>> {
        let mut code = b"    ".to_vec();
        code.extend(self.yaml(&Value::String(PhpString::from(id.to_vec())), 0, 0)?);
        code.extend_from_slice(b":\n");
        if let Some(class) = definition.class().filter(|c| !c.is_empty() && c != "0") {
            let class = class.strip_prefix('\\').unwrap_or(&class).to_string();
            let class = self.resolve(Value::from(class))?;
            code.extend_from_slice(b"        class: ");
            code.extend(self.yaml(&class, 0, 0)?);
            code.push(b'\n');
        }

        if !definition.is_private() {
            code.extend_from_slice(if definition.is_public() {
                b"        public: true\n"
            } else {
                b"        public: false\n"
            });
        }

        let mut tags_code = Vec::new();
        let mut tags = definition.tags();
        let errors = PhpArray::list(definition.errors().into_iter().map(|e| {
            let mut error = PhpArray::new();
            error.insert("message", Value::from(e));
            Value::Array(error)
        }));
        tags.insert("container.error", Value::Array(errors));
        for (name, tags) in tags.iter() {
            let Value::Array(tags) = tags else { continue };
            for attributes in tags.values() {
                let Value::Array(attributes) = attributes else {
                    continue;
                };
                let mut att = Vec::new();
                for (key, value) in attributes.iter() {
                    let mut entry = self.yaml(&key.to_value(), 0, 0)?;
                    entry.extend_from_slice(b": ");
                    entry.extend(self.yaml(value, 0, 0)?);
                    att.push(entry);
                }
                let att = if att.is_empty() {
                    Vec::new()
                } else {
                    let mut out = b": { ".to_vec();
                    out.extend(att.join(&b", "[..]));
                    out.extend_from_slice(b" }");
                    out
                };
                tags_code.extend_from_slice(b"            - ");
                tags_code.extend(self.yaml(&name.to_value(), 0, 0)?);
                tags_code.extend(att);
                tags_code.push(b'\n');
            }
        }
        if !tags_code.is_empty() {
            code.extend_from_slice(b"        tags:\n");
            code.extend(tags_code);
        }

        if let Some(file) = definition.file().filter(|f| !f.is_empty() && f != "0") {
            let file = self.resolve(Value::from(file))?;
            code.extend_from_slice(b"        file: ");
            code.extend(self.yaml(&file, 0, 0)?);
            code.push(b'\n');
        }

        if definition.is_synthetic() {
            code.extend_from_slice(b"        synthetic: true\n");
        }

        if let Some(deprecation) = definition.deprecation("%service_id%") {
            code.extend_from_slice(b"        deprecated:\n");
            for (key, value) in deprecation.to_array().iter() {
                if value.as_string().is_some_and(|s| !s.is_empty()) {
                    code.extend_from_slice(b"            ");
                    code.extend(key.bytes().iter());
                    code.extend_from_slice(b": ");
                    code.extend(self.yaml(value, 0, 0)?);
                    code.push(b'\n');
                }
            }
        }

        if definition.is_autowired() {
            code.extend_from_slice(b"        autowire: true\n");
        }

        if definition.is_autoconfigured() {
            code.extend_from_slice(b"        autoconfigure: true\n");
        }

        if definition.is_abstract() {
            code.extend_from_slice(b"        abstract: true\n");
        }

        if definition.is_lazy() {
            code.extend_from_slice(b"        lazy: true\n");
        }

        let arguments = definition.arguments();
        if !arguments.is_empty() {
            let value = self.dump_value(&Value::Array(arguments))?;
            code.extend_from_slice(b"        arguments: ");
            code.extend(self.yaml(&value, 0, 0)?);
            code.push(b'\n');
        }

        let properties = definition.properties();
        if !properties.is_empty() {
            let value = self.dump_value(&Value::Array(properties))?;
            code.extend_from_slice(b"        properties: ");
            code.extend(self.yaml(&value, 0, 0)?);
            code.push(b'\n');
        }

        let method_calls = definition.method_calls();
        if !method_calls.is_empty() {
            let value = self.dump_value(&Value::Array(method_calls))?;
            code.extend_from_slice(b"        calls:\n");
            code.extend(self.yaml(&value, 1, 12)?);
            code.push(b'\n');
        }

        if !definition.is_shared() {
            code.extend_from_slice(b"        shared: false\n");
        }

        if let Some(decorated) = definition.decorated_service() {
            code.extend_from_slice(format!("        decorates: {}\n", decorated.id).as_bytes());
            if let Some(renamed_id) = &decorated.renamed_id {
                code.extend_from_slice(
                    format!("        decoration_inner_name: {renamed_id}\n").as_bytes(),
                );
            }
            if decorated.priority != 0 {
                code.extend_from_slice(
                    format!("        decoration_priority: {}\n", decorated.priority).as_bytes(),
                );
            }
            match decorated.invalid_behavior {
                Some(InvalidBehavior::Null) => {
                    code.extend_from_slice(b"        decoration_on_invalid: null\n");
                }
                Some(InvalidBehavior::Ignore) => {
                    code.extend_from_slice(b"        decoration_on_invalid: ignore\n");
                }
                _ => {}
            }
        }

        let factory = definition.factory();
        if factory.is_truthy() {
            if let Value::Array(callable) = &factory
                && !is_closure_from_callable(callable)
                && class_identical(definition, callable.get(&Key::Int(0)))
            {
                code.extend_from_slice(b"        constructor: ");
                code.extend(
                    callable
                        .get(&Key::Int(1))
                        .map(to_php_string)
                        .unwrap_or_default()
                        .as_bytes(),
                );
                code.push(b'\n');
            } else {
                let callable = self.dump_callable(&factory)?;
                code.extend_from_slice(b"        factory: ");
                code.extend(self.yaml(&callable, 0, 0)?);
                code.push(b'\n');
            }
        }

        let configurator = definition.configurator();
        if configurator.is_truthy() {
            let callable = self.dump_callable(&configurator)?;
            code.extend_from_slice(b"        configurator: ");
            code.extend(self.yaml(&callable, 0, 0)?);
            code.push(b'\n');
        }

        Ok(code)
    }

    fn add_service_alias(alias: &str, id: &Alias) -> Vec<u8> {
        let mut deprecated = String::new();

        let deprecation = id.deprecation("%alias_id%");
        if let Some(deprecation) = &deprecation {
            deprecated.push_str("        deprecated:\n");
            for (key, value) in [
                ("package", &deprecation.package),
                ("version", &deprecation.version),
                ("message", &deprecation.message),
            ] {
                if !value.is_empty() {
                    let _ = writeln!(deprecated, "            {key}: {value}");
                }
            }
        }

        if deprecation.is_none() && id.is_private() {
            return format!("    {alias}: '@{}'\n", id.id()).into_bytes();
        }

        if id.is_public() {
            deprecated = format!("        public: true\n{deprecated}");
        }

        format!("    {alias}:\n        alias: {}\n{deprecated}", id.id()).into_bytes()
    }

    fn add_services(&mut self) -> Result<Vec<u8>> {
        let definitions: Vec<(String, Definition)> = self
            .base
            .container()
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        if definitions.is_empty() {
            return Ok(Vec::new());
        }

        let mut code = b"services:\n".to_vec();
        for (id, definition) in &definitions {
            code.extend(self.add_service(id.as_bytes(), definition)?);
        }

        let aliases = self.base.container().get_aliases();
        for (alias, id) in aliases.iter() {
            let mut id = id;
            let mut steps = 0;
            while let Some(next) = aliases.get(&id.id()) {
                if steps > aliases.len() {
                    break;
                }
                steps += 1;
                id = next;
            }
            code.extend(Self::add_service_alias(alias, id));
        }

        Ok(code)
    }

    fn add_parameters(&mut self) -> Result<Vec<u8>> {
        let all = self.base.container().get_parameter_bag().all().clone();
        if all.is_empty() {
            return Ok(Vec::new());
        }

        let compiled = self.base.container().is_compiled();
        let parameters = self.prepare_parameters(&all, compiled)?;

        let mut root = PhpArray::new();
        root.insert("parameters", parameters);
        self.yaml(&Value::Array(root), 2, 0)
    }

    /// `dumpCallable($callable)`.
    fn dump_callable(&mut self, callable: &Value) -> Result<Value> {
        let callable = if let Value::Array(array) = callable {
            let first = array.get(&Key::Int(0)).cloned().unwrap_or_default();
            let method = array.get(&Key::Int(1)).cloned().unwrap_or_default();
            let first = match Reference::from_value(&first) {
                Some(reference) => Value::from(service_call(reference.id(), Some(reference))),
                None => first,
            };
            Value::Array(PhpArray::list([first, method]))
        } else {
            callable.clone()
        };

        self.resolve(callable)
    }

    /// `dumpValue($value)`: the value as the YAML dumper takes it.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn dump_value(&mut self, value: &Value) -> Result<Value> {
        if let Some(closure) = ServiceClosureArgument::from_value(value) {
            let inner = self.dump_value(&closure.value())?;
            return Ok(tagged("service_closure", inner));
        }
        if let Some(env_closure) = EnvClosureArgument::from_value(value) {
            let env_expr = self.resolve(Value::from(env_closure.value()))?;
            let default = env_closure.default_value().clone();

            if !env_closure.is_stringable() {
                return Ok(tagged(
                    "env_closure",
                    if default.is_null() {
                        env_expr
                    } else {
                        Value::Array(PhpArray::list([env_expr, default, Value::Bool(false)]))
                    },
                ));
            }

            return Ok(tagged(
                "env_closure",
                Value::Array(PhpArray::list([env_expr, default])),
            ));
        }
        if let Some(argument) = Argument::from_value(value) {
            let iterator = IteratorArgument::from_value(value);
            let locator = ServiceLocatorArgument::from_value(value);
            let tag = match (&iterator, &locator) {
                (Some(iterator), _) if iterator.is_tagged() => Some((iterator.clone(), true)),
                (_, Some(locator)) => locator.tagged_iterator_argument().map(|t| (t, false)),
                _ => None,
            };

            if let Some((tag, is_iterator)) = tag {
                let tag_name = Value::from(tag.tag().unwrap_or_default());
                let mut content = match tag.index_attribute() {
                    None => tag_name,
                    Some(index_attribute) => {
                        let mut content = PhpArray::new();
                        content.insert("tag", tag_name);
                        content.insert("index_by", Value::from(index_attribute.as_str()));

                        let default_prefix = default_prefix(&index_attribute);
                        if let Some(method) = tag.default_index_method(false)
                            && method != format!("{default_prefix}Name")
                        {
                            content.insert("default_index_method", Value::from(method));
                        }
                        if let Some(method) = tag.default_priority_method(false)
                            && method != format!("{default_prefix}Priority")
                        {
                            content.insert("default_priority_method", Value::from(method));
                        }
                        Value::Array(content)
                    }
                };
                let excludes = tag.exclude();
                if !excludes.is_empty() {
                    let mut map = match content {
                        Value::Array(map) => map,
                        other => {
                            let mut map = PhpArray::new();
                            map.insert("tag", other);
                            map
                        }
                    };
                    map.insert(
                        "exclude",
                        if excludes.len() == 1 {
                            Value::from(excludes[0].as_str())
                        } else {
                            Value::Array(PhpArray::list(
                                excludes.iter().map(|e| Value::from(e.as_str())),
                            ))
                        },
                    );
                    content = Value::Array(map);
                }
                if !tag.exclude_self() {
                    // `$content['exclude_self'] = false` on the tag name
                    // (a string) is PHP's `TypeError`.
                    let Value::Array(mut map) = content else {
                        return Err(Error::new(
                            ErrorKind::Other("TypeError".to_string()),
                            "Cannot access offset of type string on string",
                        ));
                    };
                    map.insert("exclude_self", Value::Bool(false));
                    content = Value::Array(map);
                }

                return Ok(tagged(
                    if is_iterator {
                        "tagged_iterator"
                    } else {
                        "tagged_locator"
                    },
                    content,
                ));
            }

            let tag = if iterator.is_some() {
                "iterator"
            } else if locator.is_some() {
                "service_locator"
            } else {
                return Err(Error::runtime(format!(
                    "Unspecified Yaml tag for type \"{}\".",
                    value.debug_type()
                )));
            };

            let values = self.dump_value(&Value::Array(argument.values()))?;
            return Ok(tagged(tag, values));
        }

        if let Value::Array(array) = value {
            let mut code = PhpArray::new();
            for (k, v) in array.iter() {
                let key = match k {
                    Key::Str(s) => {
                        let resolved = self.resolve(Value::String(s.clone()))?;
                        Key::from_value(&resolved).unwrap_or_else(|| k.clone())
                    }
                    Key::Int(_) => k.clone(),
                };
                let v = self.dump_value(v)?;
                code.insert(key, v);
            }
            return Ok(Value::Array(code));
        }
        if let Some(reference) = Reference::from_value(value) {
            return Ok(Value::from(service_call(reference.id(), Some(reference))));
        }
        if let Some(parameter) = Parameter::from_value(value) {
            return Ok(Value::from(format!("%{}%", parameter.id())));
        }
        if let Some(text) = expression_text(value) {
            let mut out = b"@=".to_vec();
            out.extend_from_slice(text.as_bytes());
            return Ok(Value::String(PhpString::from(out)));
        }
        if let Some(definition) = Definition::from_value(value) {
            let mut yaml = b"_:\n".to_vec();
            yaml.extend(self.add_service(b"_", &definition)?);
            let parsed = Parser::new()
                .parse(&yaml, Flags::PARSE_CUSTOM_TAGS)
                .map_err(|e| {
                    Error::new(
                        ErrorKind::Other(
                            "Symfony\\Component\\Yaml\\Exception\\ParseException".to_string(),
                        ),
                        e.to_string(),
                    )
                })?;
            let service = index(&index(&parsed, "_"), "_");
            return Ok(tagged("service", service));
        }
        if let Some(case) = downcast::<EnumCase>(value) {
            let class = value
                .as_object()
                .map(|o| o.class().to_string())
                .unwrap_or_default();
            return Ok(tagged(
                "php/enum",
                Value::from(format!("{class}::{}", case.name())),
            ));
        }
        if let Some(argument) = AbstractArgument::from_value(value) {
            return Ok(tagged("abstract", Value::from(argument.text())));
        }
        if let Value::Object(_) = value {
            return Err(Error::runtime(format!(
                "Unable to dump a service container if a parameter is an object or a resource, got \"{}\".",
                value.debug_type()
            )));
        }

        self.resolve(value.clone())
    }

    fn prepare_parameters(&mut self, parameters: &PhpArray, escape: bool) -> Result<Value> {
        let mut filtered = PhpArray::new();
        for (key, value) in parameters.iter() {
            let value = if let Value::Array(array) = value {
                self.prepare_parameters(array, escape)?
            } else if let Some(reference) = Reference::from_value(value) {
                Value::from(format!("@{}", reference.id()))
            } else if let Value::String(s) = value
                && s.as_bytes().starts_with(b"@")
            {
                let mut out = b"@".to_vec();
                out.extend_from_slice(s.as_bytes());
                Value::String(PhpString::from(out))
            } else {
                value.clone()
            };

            filtered.insert(key.clone(), value);
        }

        if escape {
            self.resolve(Value::Array(escape_array(&filtered)))
        } else {
            Ok(Value::Array(filtered))
        }
    }
}

impl DumperInterface for YamlDumper<'_> {
    fn dump(&mut self, _options: &PhpArray) -> Result<PhpString> {
        self.dump_yaml()
    }
}

/// `$value[$key]` of a parsed document, or null.
fn index(value: &Value, key: &str) -> Value {
    match value {
        Value::Array(array) => array.get(&Key::from(key)).cloned().unwrap_or_default(),
        _ => Value::Null,
    }
}

/// `getServiceCall($id, $reference)`.
fn service_call(id: &str, reference: Option<&Reference>) -> String {
    if let Some(reference) = reference {
        match reference.invalid_behavior() {
            InvalidBehavior::RuntimeException | InvalidBehavior::Exception => {}
            InvalidBehavior::IgnoreUninitialized => return format!("@!{id}"),
            _ => return format!("@?{id}"),
        }
    }

    format!("@{id}")
}

/// `escape($arguments)`.
fn escape_array(arguments: &PhpArray) -> PhpArray {
    let mut args = PhpArray::new();
    for (k, v) in arguments.iter() {
        let v = match v {
            Value::Array(array) => Value::Array(escape_array(array)),
            Value::String(s) => Value::String(PhpString::from(replace_percent(s.as_bytes()))),
            other => other.clone(),
        };
        args.insert(k.clone(), v);
    }
    args
}

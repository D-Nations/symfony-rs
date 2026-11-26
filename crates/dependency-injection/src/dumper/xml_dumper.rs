//! Port of Symfony\Component\DependencyInjection\Dumper\XmlDumper.php.
//!
//! The output is upstream's byte for byte. `htmlspecialchars()` with
//! `ENT_XML1 | ENT_SUBSTITUTE` is written here: `& < >` (and `"` where
//! upstream passes `ENT_COMPAT`) become entities, and each invalid UTF-8
//! sequence becomes U+FFFD as PHP's decoder splits them.
//!
//! An alias chain that loops (which upstream's `while` never leaves) is
//! followed once round, then dumped where it stops.
//!
//! Upstream: symfony/dependency-injection Dumper/XmlDumper.php

use std::fmt::Write as _;

use php_value::numeric::is_numeric;
use php_value::object::EnumCase;
use php_value::{PhpArray, PhpString, Value, array::Key};

use super::{Dumper, DumperInterface, expression_text, to_php_string};
use crate::alias::Alias;
use crate::argument::{
    AbstractArgument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::object::downcast;
use crate::reference::{Parameter, Reference};

const HEADER: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container xmlns=\"http://symfony.com/schema/dic/services\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://symfony.com/schema/dic/services https://symfony.com/schema/dic/services/services-1.0.xsd\">";

/// `XmlDumper`: dumps a service container as an XML string.
#[derive(Debug)]
pub struct XmlDumper<'a> {
    base: Dumper<'a>,
}

impl<'a> XmlDumper<'a> {
    /// `new XmlDumper($container)`.
    pub fn new(container: &'a mut ContainerBuilder) -> Self {
        Self {
            base: Dumper::new(container),
        }
    }

    /// `dump()`: the container as XML.
    pub fn dump_xml(&mut self) -> Result<PhpString> {
        let mut xml = HEADER.to_string();
        for line in self.add_parameters()? {
            xml.push_str("\n  ");
            xml.push_str(&line);
        }
        for line in self.add_services()? {
            xml.push_str("\n  ");
            xml.push_str(&line);
        }
        xml.push_str("\n</container>\n");

        let resolved = self.container().resolve_env_placeholders(
            Value::from(xml),
            &EnvFormat::Placeholder,
            None,
        )?;
        Ok(to_php_string(&resolved))
    }

    fn container(&mut self) -> &mut ContainerBuilder {
        self.base.container_mut()
    }

    fn add_parameters(&mut self) -> Result<Vec<String>> {
        let mut data = self.base.container().get_parameter_bag().all().clone();
        if data.is_empty() {
            return Ok(Vec::new());
        }
        if self.base.container().is_compiled() {
            data = escape(&data);
        }

        let mut lines = vec!["<parameters>".to_string()];
        for line in self.convert_parameters(&data, "parameter", "key")? {
            lines.push(format!("  {line}"));
        }
        lines.push("</parameters>".to_string());
        Ok(lines)
    }

    fn add_method_calls(&mut self, method_calls: &PhpArray) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        for call in method_calls.values() {
            let Value::Array(call) = call else { continue };
            let method = call
                .get(&Key::Int(0))
                .map(to_php_string)
                .unwrap_or_default();
            let returns_clone = call.get(&Key::Int(2)).is_some_and(Value::is_truthy);
            let xml_attr = format!(
                " method=\"{}\"{}",
                encode(method.as_bytes(), true),
                if returns_clone {
                    " returns-clone=\"true\""
                } else {
                    ""
                }
            );
            match call.get(&Key::Int(1)) {
                Some(Value::Array(arguments)) if !arguments.is_empty() => {
                    lines.push(format!("<call{xml_attr}>"));
                    for line in self.convert_parameters(arguments, "argument", "key")? {
                        lines.push(format!("  {line}"));
                    }
                    lines.push("</call>".to_string());
                }
                _ => lines.push(format!("<call{xml_attr}/>")),
            }
        }
        Ok(lines)
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn add_service(&mut self, definition: &Definition, id: Option<&str>) -> Result<Vec<String>> {
        let mut xml_attr = String::new();
        if let Some(id) = id {
            let _ = write!(xml_attr, " id=\"{}\"", encode(id.as_bytes(), true));
        }
        if let Some(class) = definition.class().filter(|c| truthy(c)) {
            let class = class.strip_prefix('\\').unwrap_or(&class);
            let _ = write!(xml_attr, " class=\"{}\"", encode(class.as_bytes(), true));
        }
        if !definition.is_shared() {
            xml_attr.push_str(" shared=\"false\"");
        }
        if definition.is_public() {
            xml_attr.push_str(" public=\"true\"");
        }
        if definition.is_synthetic() {
            xml_attr.push_str(" synthetic=\"true\"");
        }
        if definition.is_lazy() {
            xml_attr.push_str(" lazy=\"true\"");
        }
        if let Some(decorated) = definition.decorated_service() {
            let _ = write!(
                xml_attr,
                " decorates=\"{}\"",
                encode(decorated.id.as_bytes(), true)
            );
            match decorated.invalid_behavior {
                Some(InvalidBehavior::Null) => xml_attr.push_str(" decoration-on-invalid=\"null\""),
                Some(InvalidBehavior::Ignore) => {
                    xml_attr.push_str(" decoration-on-invalid=\"ignore\"");
                }
                _ => {}
            }
            if let Some(renamed_id) = &decorated.renamed_id {
                let _ = write!(
                    xml_attr,
                    " decoration-inner-name=\"{}\"",
                    encode(renamed_id.as_bytes(), true)
                );
            }
            if decorated.priority != 0 {
                let _ = write!(xml_attr, " decoration-priority=\"{}\"", decorated.priority);
            }
        }

        let mut xml = Vec::new();

        let mut tags = definition.tags();
        let errors = PhpArray::list(definition.errors().into_iter().map(|e| {
            let mut error = PhpArray::new();
            error.insert("message", Value::from(e));
            Value::Array(error)
        }));
        tags.insert("container.error", Value::Array(errors));
        for (name, tags) in tags.iter() {
            let name = name.bytes();
            let Value::Array(tags) = tags else { continue };
            for attributes in tags.values() {
                let Value::Array(attributes) = attributes else {
                    continue;
                };
                // Check if we have recursive attributes
                if attributes.values().any(|v| matches!(v, Value::Array(_))) {
                    xml.push(format!("  <tag name=\"{}\">", encode(&name, true)));
                    for line in add_tag_recursive_attributes(attributes)? {
                        xml.push(format!("    {line}"));
                    }
                    xml.push("  </tag>".to_string());
                } else {
                    let name_key = Key::from("name");
                    let name_attr = attributes.get(&name_key);
                    let shown = match name_attr {
                        Some(value) => to_php_string(value).as_bytes().to_vec(),
                        None => name.to_vec(),
                    };
                    let mut attr = format!(" name=\"{}\"", encode(&shown, true));
                    for (key, value) in attributes.iter() {
                        if *key != name_key {
                            let value = php_to_xml_or_empty(value)?;
                            let _ = write!(
                                attr,
                                " {}=\"{}\"",
                                encode(&key.bytes(), true),
                                encode(value.as_bytes(), true)
                            );
                        }
                    }
                    if name_attr.is_some() {
                        xml.push(format!("  <tag{attr}>{}</tag>", encode(&name, false)));
                    } else {
                        xml.push(format!("  <tag{attr}/>"));
                    }
                }
            }
        }

        if let Some(file) = definition.file().filter(|f| truthy(f)) {
            xml.push(format!("  <file>{}</file>", encode(file.as_bytes(), false)));
        }

        for line in self.convert_parameters(&definition.arguments(), "argument", "key")? {
            xml.push(format!("  {line}"));
        }

        for line in self.convert_parameters(&definition.properties(), "property", "name")? {
            xml.push(format!("  {line}"));
        }

        for line in self.add_method_calls(&definition.method_calls())? {
            xml.push(format!("  {line}"));
        }

        let factory = definition.factory();
        if factory.is_truthy() {
            if let Value::Array(callable) = &factory
                && !is_closure_from_callable(callable)
                && class_identical(definition, callable.get(&Key::Int(0)))
            {
                let _ = write!(
                    xml_attr,
                    " constructor=\"{}\"",
                    encode(callable_method(callable).as_bytes(), true)
                );
            } else if let Value::Array(callable) = &factory {
                let first = callable.get(&Key::Int(0)).cloned().unwrap_or_default();
                let method = callable_method(callable);
                if let Some(inner) = Definition::from_value(&first) {
                    xml.push(format!(
                        "  <factory method=\"{}\">",
                        encode(method.as_bytes(), true)
                    ));
                    for line in self.add_service(&inner, None)? {
                        xml.push(format!("    {line}"));
                    }
                    xml.push("  </factory>".to_string());
                } else if !first.is_null() {
                    xml.push(format!(
                        "  <factory {}=\"{}\" method=\"{}\"/>",
                        if Reference::from_value(&first).is_some() {
                            "service"
                        } else {
                            "class"
                        },
                        encode(to_php_string(&first).as_bytes(), true),
                        encode(method.as_bytes(), true)
                    ));
                } else {
                    xml.push(format!(
                        "  <factory method=\"{}\"/>",
                        encode(method.as_bytes(), true)
                    ));
                }
            } else {
                xml.push(format!(
                    "  <factory function=\"{}\"/>",
                    encode(to_php_string(&factory).as_bytes(), true)
                ));
            }
        }

        if let Some(deprecation) = definition.deprecation("%service_id%") {
            xml.push(format!(
                "  <deprecated package=\"{}\" version=\"{}\">{}</deprecated>",
                encode(deprecation.package.as_bytes(), true),
                encode(deprecation.version.as_bytes(), true),
                encode(deprecation.message.as_bytes(), false)
            ));
        }

        if definition.is_autowired() {
            xml_attr.push_str(" autowire=\"true\"");
        }

        if definition.is_autoconfigured() {
            xml_attr.push_str(" autoconfigure=\"true\"");
        }

        if definition.is_abstract() {
            xml_attr.push_str(" abstract=\"true\"");
        }

        let configurator = definition.configurator();
        if configurator.is_truthy() {
            if let Value::Array(callable) = &configurator {
                let first = callable.get(&Key::Int(0)).cloned().unwrap_or_default();
                let method = callable_method(callable);
                if let Some(inner) = Definition::from_value(&first) {
                    xml.push(format!(
                        "  <configurator method=\"{}\">",
                        encode(method.as_bytes(), true)
                    ));
                    for line in self.add_service(&inner, None)? {
                        xml.push(format!("    {line}"));
                    }
                    xml.push("  </configurator>".to_string());
                } else {
                    xml.push(format!(
                        "  <configurator {}=\"{}\" method=\"{}\"/>",
                        if Reference::from_value(&first).is_some() {
                            "service"
                        } else {
                            "class"
                        },
                        encode(to_php_string(&first).as_bytes(), true),
                        encode(method.as_bytes(), true)
                    ));
                }
            } else {
                xml.push(format!(
                    "  <configurator function=\"{}\"/>",
                    encode(to_php_string(&configurator).as_bytes(), true)
                ));
            }
        }

        if xml.is_empty() {
            Ok(vec![format!("<service{xml_attr}/>")])
        } else {
            let mut lines = Vec::with_capacity(xml.len() + 2);
            lines.push(format!("<service{xml_attr}>"));
            lines.extend(xml);
            lines.push("</service>".to_string());
            Ok(lines)
        }
    }

    fn add_service_alias(alias: &str, id: &Alias) -> Vec<String> {
        let xml_attr = format!(
            " id=\"{}\" alias=\"{}\"{}",
            encode(alias.as_bytes(), true),
            encode(id.id().as_bytes(), true),
            if id.is_public() {
                " public=\"true\""
            } else {
                ""
            }
        );

        if let Some(deprecation) = id.deprecation("%alias_id%") {
            vec![
                format!("<service{xml_attr}>"),
                format!(
                    "  <deprecated package=\"{}\" version=\"{}\">{}</deprecated>",
                    encode(deprecation.package.as_bytes(), true),
                    encode(deprecation.version.as_bytes(), true),
                    encode(deprecation.message.as_bytes(), false)
                ),
                "</service>".to_string(),
            ]
        } else {
            vec![format!("<service{xml_attr}/>")]
        }
    }

    fn add_services(&mut self) -> Result<Vec<String>> {
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

        let mut lines = vec!["<services>".to_string()];
        for (id, definition) in &definitions {
            for line in self.add_service(definition, Some(id))? {
                lines.push(format!("  {line}"));
            }
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
            for line in Self::add_service_alias(alias, id) {
                lines.push(format!("  {line}"));
            }
        }
        lines.push("</services>".to_string());
        Ok(lines)
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn convert_parameters(
        &mut self,
        parameters: &PhpArray,
        type_: &str,
        key_attribute: &str,
    ) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        let with_keys = !parameters.is_list();
        for (key, value) in parameters.iter() {
            let mut xml_attr = if with_keys {
                format!(" {key_attribute}=\"{}\"", encode(&key.bytes(), true))
            } else {
                String::new()
            };

            if let Some(env_closure) = EnvClosureArgument::from_value(value) {
                xml_attr.push_str(" type=\"env_closure\"");
                if env_closure.is_stringable() {
                    xml_attr.push_str(" stringable=\"true\"");
                }
                let default = env_closure.default_value();
                if !default.is_null() {
                    let _ = write!(
                        xml_attr,
                        " default=\"{}\"",
                        encode(php_to_xml(default)?.as_bytes(), true)
                    );
                }
                let env_expr = self.container().resolve_env_placeholders(
                    Value::from(env_closure.value()),
                    &EnvFormat::Placeholder,
                    None,
                )?;
                lines.push(format!(
                    "<{type_}{xml_attr}>{}</{type_}>",
                    encode(to_php_string(&env_expr).as_bytes(), false)
                ));
                continue;
            }

            let iterator = IteratorArgument::from_value(value);
            let locator = ServiceLocatorArgument::from_value(value);
            let tagged = match (&iterator, &locator) {
                (Some(iterator), _) if iterator.is_tagged() => Some((iterator.clone(), true)),
                (_, Some(locator)) => locator.tagged_iterator_argument().map(|t| (t, false)),
                _ => None,
            };

            if let Some((tag, is_iterator)) = tagged {
                xml_attr.push_str(if is_iterator {
                    " type=\"tagged_iterator\""
                } else {
                    " type=\"tagged_locator\""
                });
                let _ = write!(
                    xml_attr,
                    " tag=\"{}\"",
                    encode(tag.tag().unwrap_or_default().as_bytes(), true)
                );

                if let Some(index_attribute) = tag.index_attribute() {
                    let _ = write!(
                        xml_attr,
                        " index-by=\"{}\"",
                        encode(index_attribute.as_bytes(), true)
                    );

                    let default_prefix = default_prefix(&index_attribute);
                    let index_method = tag.default_index_method(false);
                    if index_method.as_deref() != Some(&format!("{default_prefix}Name")) {
                        let _ = write!(
                            xml_attr,
                            " default-index-method=\"{}\"",
                            encode(index_method.unwrap_or_default().as_bytes(), true)
                        );
                    }
                    let priority_method = tag.default_priority_method(false);
                    if priority_method.as_deref() != Some(&format!("{default_prefix}Priority")) {
                        let _ = write!(
                            xml_attr,
                            " default-priority-method=\"{}\"",
                            encode(priority_method.unwrap_or_default().as_bytes(), true)
                        );
                    }
                }
                let excludes = tag.exclude();
                if excludes.len() == 1 {
                    let _ = write!(
                        xml_attr,
                        " exclude=\"{}\"",
                        encode(excludes[0].as_bytes(), true)
                    );
                }
                if !tag.exclude_self() {
                    xml_attr.push_str(" exclude-self=\"false\"");
                }

                if excludes.len() > 1 {
                    lines.push(format!("<{type_}{xml_attr}>"));
                    for exclude in &excludes {
                        lines.push(format!(
                            "  <exclude>{}</exclude>",
                            encode(exclude.as_bytes(), false)
                        ));
                    }
                    lines.push(format!("</{type_}>"));
                } else {
                    lines.push(format!("<{type_}{xml_attr}/>"));
                }
                continue;
            }

            let service_closure = ServiceClosureArgument::from_value(value);
            let collection = if let Value::Array(array) = value {
                xml_attr.push_str(" type=\"collection\"");
                Some(array.clone())
            } else if let Some(iterator) = &iterator {
                xml_attr.push_str(" type=\"iterator\"");
                Some(iterator.values())
            } else if let Some(locator) = &locator {
                xml_attr.push_str(" type=\"service_locator\"");
                Some(locator.values())
            } else if let Some(closure) = &service_closure
                && Reference::from_value(&closure.value()).is_none()
            {
                xml_attr.push_str(" type=\"service_closure\"");
                Some(closure.values())
            } else {
                None
            };

            if let Some(values) = collection {
                if values.is_empty() {
                    lines.push(format!("<{type_}{xml_attr}/>"));
                } else {
                    lines.push(format!("<{type_}{xml_attr}>"));
                    for line in self.convert_parameters(&values, type_, "key")? {
                        lines.push(format!("  {line}"));
                    }
                    lines.push(format!("</{type_}>"));
                }
                continue;
            }

            let reference = if let Some(closure) = &service_closure {
                xml_attr.push_str(" type=\"service_closure\"");
                Reference::from_value(&closure.value()).cloned()
            } else if let Some(reference) = Reference::from_value(value) {
                xml_attr.push_str(" type=\"service\"");
                Some(reference.clone())
            } else {
                None
            };
            if let Some(reference) = reference {
                let _ = write!(
                    xml_attr,
                    " id=\"{}\"",
                    encode(reference.id().as_bytes(), true)
                );
                xml_attr.push_str(match reference.invalid_behavior() {
                    InvalidBehavior::Null => " on-invalid=\"null\"",
                    InvalidBehavior::Ignore => " on-invalid=\"ignore\"",
                    InvalidBehavior::IgnoreUninitialized => " on-invalid=\"ignore_uninitialized\"",
                    _ => "",
                });
                lines.push(format!("<{type_}{xml_attr}/>"));
                continue;
            }

            if let Some(definition) = Definition::from_value(value) {
                xml_attr.push_str(" type=\"service\"");
                lines.push(format!("<{type_}{xml_attr}>"));
                for line in self.add_service(&definition, None)? {
                    lines.push(format!("  {line}"));
                }
                lines.push(format!("</{type_}>"));
                continue;
            }

            let mut value = value.clone();
            if let Some(text) = expression_text(&value) {
                xml_attr.push_str(" type=\"expression\"");
                value = Value::String(text);
            } else if let Value::String(s) = &value
                && !is_xml_text(s.as_bytes())
            {
                xml_attr.push_str(" type=\"binary\"");
                value = Value::from(php_value::base64::encode(s.as_bytes()));
            } else if downcast::<EnumCase>(&value).is_some() {
                xml_attr.push_str(" type=\"constant\"");
            } else if let Some(argument) = AbstractArgument::from_value(&value) {
                xml_attr.push_str(" type=\"abstract\"");
                value = Value::from(argument.text());
            } else if let Value::String(s) = &value {
                let bytes = s.as_bytes();
                if matches!(bytes, b"null" | b"true" | b"false")
                    || is_numeric(bytes)
                    || is_binary_number(bytes)
                    || is_hexadecimal_number(bytes)
                {
                    xml_attr.push_str(" type=\"string\"");
                }
            }

            let text = php_to_xml(&value)?;
            if text.is_empty() {
                lines.push(format!("<{type_}{xml_attr}/>"));
            } else {
                lines.push(format!(
                    "<{type_}{xml_attr}>{}</{type_}>",
                    encode(text.as_bytes(), false)
                ));
            }
        }
        Ok(lines)
    }

    /// `XmlDumper::phpToXml($value)`: converts PHP types to XML types.
    pub fn php_to_xml(value: &Value) -> Result<PhpString> {
        php_to_xml(value)
    }
}

impl DumperInterface for XmlDumper<'_> {
    fn dump(&mut self, _options: &PhpArray) -> Result<PhpString> {
        self.dump_xml()
    }
}

fn add_tag_recursive_attributes(attributes: &PhpArray) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for (name, value) in attributes.iter() {
        let name = name.bytes();
        if let Value::Array(value) = value {
            lines.push(format!("<attribute name=\"{}\">", encode(&name, true)));
            for line in add_tag_recursive_attributes(value)? {
                lines.push(format!("  {line}"));
            }
            lines.push("</attribute>".to_string());
        } else {
            let text = php_to_xml_or_empty(value)?;
            if !text.is_empty() {
                lines.push(format!(
                    "<attribute name=\"{}\">{}</attribute>",
                    encode(&name, true),
                    encode(text.as_bytes(), false)
                ));
            }
        }
    }
    Ok(lines)
}

/// `phpToXml($value ?? '')`.
fn php_to_xml_or_empty(value: &Value) -> Result<PhpString> {
    if value.is_null() {
        Ok(PhpString::default())
    } else {
        php_to_xml(value)
    }
}

/// `phpToXml($value)`.
fn php_to_xml(value: &Value) -> Result<PhpString> {
    Ok(match value {
        Value::Null => PhpString::from("null"),
        Value::Bool(true) => PhpString::from("true"),
        Value::Bool(false) => PhpString::from("false"),
        Value::Object(object) => {
            if let Some(parameter) = Parameter::from_value(value) {
                PhpString::from(format!("%{}%", parameter.id()))
            } else if let Some(case) = downcast::<EnumCase>(value) {
                PhpString::from(format!("{}::{}", object.class(), case.name()))
            } else {
                return Err(Error::runtime(format!(
                    "Unable to dump a service container if a parameter is an object or a resource, got \"{}\".",
                    value.debug_type()
                )));
            }
        }
        other => to_php_string(other),
    })
}

/// `['Closure', 'fromCallable'] === $callable`.
pub(crate) fn is_closure_from_callable(callable: &PhpArray) -> bool {
    callable.len() == 2
        && matches!(callable.get(&Key::Int(0)), Some(Value::String(s)) if s.as_bytes() == b"Closure")
        && matches!(callable.get(&Key::Int(1)), Some(Value::String(s)) if s.as_bytes() == b"fromCallable")
        && callable.keys().zip([0, 1]).all(|(k, i)| *k == Key::Int(i))
}

/// `$definition->getClass() === $callable[0]`.
pub(crate) fn class_identical(definition: &Definition, first: Option<&Value>) -> bool {
    match (definition.class(), first) {
        (None, None | Some(Value::Null)) => true,
        (Some(class), Some(Value::String(s))) => class.as_bytes() == s.as_bytes(),
        _ => false,
    }
}

/// `$callable[1]`.
fn callable_method(callable: &PhpArray) -> PhpString {
    callable
        .get(&Key::Int(1))
        .map(to_php_string)
        .unwrap_or_default()
}

/// PHP truthiness of a string.
fn truthy(s: &str) -> bool {
    !s.is_empty() && s != "0"
}

/// `'getDefault'.str_replace(' ', '', ucwords(preg_replace('/[^a-zA-Z0-9\x7f-\xff]++/', ' ', $attribute)))`.
pub(crate) fn default_prefix(attribute: &str) -> String {
    let mut out = b"getDefault".to_vec();
    let mut start = true;
    for &b in attribute.as_bytes() {
        if b.is_ascii_alphanumeric() || b >= 0x7f {
            out.push(if start { b.to_ascii_uppercase() } else { b });
            start = false;
        } else {
            start = true;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `$` without `D`: the subject, less one final `"\n"`.
fn without_final_newline(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

/// `preg_match('/^0b[01]*$/', $value)`.
fn is_binary_number(bytes: &[u8]) -> bool {
    without_final_newline(bytes)
        .strip_prefix(b"0b")
        .is_some_and(|digits| digits.iter().all(|b| matches!(b, b'0' | b'1')))
}

/// `preg_match('/^0x[0-9a-f]++$/i', $value)`.
fn is_hexadecimal_number(bytes: &[u8]) -> bool {
    let bytes = without_final_newline(bytes);
    bytes.len() > 2
        && bytes[0] == b'0'
        && matches!(bytes[1], b'x' | b'X')
        && bytes[2..].iter().all(u8::is_ascii_hexdigit)
}

/// `preg_match('/^[^\x00-\x08\x0B\x0C\x0E-\x1F\x7F]*+$/u', $value)`:
/// valid UTF-8 without control characters other than tab, LF and CR.
fn is_xml_text(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
        && !bytes
            .iter()
            .any(|&b| matches!(b, 0x00..=0x08 | 0x0B | 0x0C | 0x0E..=0x1F | 0x7F))
}

/// `escape($arguments)`: `%` doubled in the strings, nested arrays too.
fn escape(arguments: &PhpArray) -> PhpArray {
    let mut args = PhpArray::new();
    for (k, v) in arguments.iter() {
        let v = match v {
            Value::Array(array) => Value::Array(escape(array)),
            Value::String(s) => Value::String(PhpString::from(replace_percent(s.as_bytes()))),
            other => other.clone(),
        };
        args.insert(k.clone(), v);
    }
    args
}

/// `str_replace('%', '%%', $s)`.
pub(crate) fn replace_percent(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        out.push(b);
        if b == b'%' {
            out.push(b'%');
        }
    }
    out
}

/// `encode($value, $flags)`: `str_replace("\r", '&#13;',
/// htmlspecialchars($value, ENT_XML1 | ENT_SUBSTITUTE | $flags, 'UTF-8'))`,
/// with `compat` for `ENT_COMPAT` (the default flags).
fn encode(value: &[u8], compat: bool) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pos = 0;
    while pos < value.len() {
        match next_utf8_char(value, pos) {
            Ok((c, next)) => {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' if compat => out.push_str("&quot;"),
                    '\r' => out.push_str("&#13;"),
                    c => out.push(c),
                }
                pos = next;
            }
            Err(next) => {
                out.push('\u{FFFD}');
                pos = next;
            }
        }
    }
    out
}

/// PHP's `get_next_char()` for UTF-8 (ext/standard/html.c): the
/// character at `pos` and where the next one starts, or, for an invalid
/// sequence, where decoding resumes.
fn next_utf8_char(s: &[u8], pos: usize) -> std::result::Result<(char, usize), usize> {
    let lead = |b: u8| b < 0x80 || (0xC2..=0xF4).contains(&b);
    let trail = |b: u8| (0x80..=0xBF).contains(&b);
    let avail = s.len() - pos;
    let c = s[pos];
    let decode = |cp: u32, len: usize| {
        char::from_u32(cp)
            .map(|ch| (ch, pos + len))
            .ok_or(pos + len)
    };
    if c < 0x80 {
        return Ok((char::from(c), pos + 1));
    }
    if c < 0xC2 {
        return Err(pos + 1);
    }
    if c < 0xE0 {
        if avail < 2 {
            return Err(pos + 1);
        }
        if !trail(s[pos + 1]) {
            return Err(pos + if lead(s[pos + 1]) { 1 } else { 2 });
        }
        let cp = (u32::from(c & 0x1F) << 6) | u32::from(s[pos + 1] & 0x3F);
        if cp < 0x80 {
            return Err(pos + 2);
        }
        return decode(cp, 2);
    }
    if c < 0xF0 {
        if avail < 3 || !trail(s[pos + 1]) || !trail(s[pos + 2]) {
            return Err(if avail < 2 || lead(s[pos + 1]) {
                pos + 1
            } else if avail < 3 || lead(s[pos + 2]) {
                pos + 2
            } else {
                pos + 3
            });
        }
        let cp = (u32::from(c & 0x0F) << 12)
            | (u32::from(s[pos + 1] & 0x3F) << 6)
            | u32::from(s[pos + 2] & 0x3F);
        if cp < 0x800 || (0xD800..=0xDFFF).contains(&cp) {
            return Err(pos + 3);
        }
        return decode(cp, 3);
    }
    if c < 0xF5 {
        if avail < 4 || !trail(s[pos + 1]) || !trail(s[pos + 2]) || !trail(s[pos + 3]) {
            return Err(if avail < 2 || lead(s[pos + 1]) {
                pos + 1
            } else if avail < 3 || lead(s[pos + 2]) {
                pos + 2
            } else if avail < 4 || lead(s[pos + 3]) {
                pos + 3
            } else {
                pos + 4
            });
        }
        let cp = (u32::from(c & 0x07) << 18)
            | (u32::from(s[pos + 1] & 0x3F) << 12)
            | (u32::from(s[pos + 2] & 0x3F) << 6)
            | u32::from(s[pos + 3] & 0x3F);
        if !(0x1_0000..=0x10_FFFF).contains(&cp) {
            return Err(pos + 4);
        }
        return decode(cp, 4);
    }
    Err(pos + 1)
}

//! Reads the compiled container as Symfony's `XmlDumper` writes it
//! (`Dumper/XmlDumper.php`), after the compiler passes. Values are read as
//! the DI component's `XmlFileLoader` read them (`XmlUtils::phpize()` for
//! untyped text). Anything else is refused with its line.
//!
//! Upstream: symfony/dependency-injection Dumper/XmlDumper.php (the format)

use std::collections::HashMap;

use php_dom::{Document, LoadOptions, NodeId, NodeType, load_xml};
use php_value::{Key, PhpArray, Value};
use symfony_config::XmlUtils;

use crate::Error;

/// `on-invalid`: `ContainerInterface::*_REFERENCE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnInvalid {
    Exception,
    Null,
    Ignore,
    IgnoreUninitialized,
}

/// One part of a string that may hold `%env(...)%` placeholders.
#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Env(String),
}

#[derive(Clone, Debug)]
pub enum ArgDef {
    /// A scalar.
    Value(Value),
    /// A string holding at least one `%env(...)%`.
    Env(Vec<StrPart>),
    Collection(Vec<(Option<Key>, ArgDef)>),
    Ref {
        id: String,
        on_invalid: OnInvalid,
        line: u32,
    },
    Closure {
        id: String,
        on_invalid: OnInvalid,
        line: u32,
    },
    Iterator(Vec<(Option<Key>, ArgDef)>),
    /// An inline (anonymous) definition: index into `Graph::services`.
    Inline(usize),
    Constant(String),
}

#[derive(Clone, Debug)]
pub enum FactoryDef {
    /// A PHP function (`function="current"`).
    Function(String),
    Static {
        class: String,
        method: String,
    },
    Service {
        service: ArgDef,
        method: String,
    },
}

#[derive(Clone, Debug)]
pub struct CallDef {
    pub method: String,
    /// `returns-clone="true"`: a wither, `$service = $service->withX()`.
    pub returns_clone: bool,
    pub arguments: Vec<ArgDef>,
}

#[derive(Clone, Debug)]
pub struct Definition {
    /// The service id; for an inline definition, a generated
    /// `.inline.<n>` name (never visible to PHP).
    pub id: String,
    pub inline: bool,
    pub class: String,
    pub line: u32,
    pub public: bool,
    pub shared: bool,
    pub synthetic: bool,
    pub lazy: bool,
    pub factory: Option<FactoryDef>,
    pub arguments: Vec<ArgDef>,
    pub calls: Vec<CallDef>,
    pub properties: Vec<(String, ArgDef)>,
    pub configurator: Option<FactoryDef>,
    pub tags: Vec<(String, Vec<(String, String)>)>,
}

#[derive(Clone, Debug)]
pub struct Alias {
    pub id: String,
    pub target: String,
    pub public: bool,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub parameters: Vec<(String, Value)>,
    pub services: Vec<Definition>,
    pub aliases: Vec<Alias>,
}

struct Reader<'a> {
    doc: &'a Document,
    graph: Graph,
    parameters: HashMap<String, Value>,
    inline_count: usize,
}

fn refuse(doc: &Document, node: NodeId, message: impl Into<String>) -> Error {
    Error {
        line: doc.line(node),
        message: message.into(),
    }
}

/// Parses the dump. Every element and attribute the trial does not
/// understand is refused.
pub fn read(xml: &[u8], dumper_parameters: &[(String, Value)]) -> Result<Graph, Error> {
    let loaded = load_xml(xml, LoadOptions::default()).map_err(|e| Error {
        line: 0,
        message: format!("the file is not XML: {e:?}"),
    })?;
    let doc = loaded.document.ok_or_else(|| Error {
        line: loaded.errors.first().map_or(0, |e| e.line),
        message: format!(
            "the file is not well-formed XML: {:?}",
            loaded.errors.first()
        ),
    })?;
    let root = doc.document_element().ok_or_else(|| Error {
        line: 0,
        message: "the file has no root element".into(),
    })?;
    if doc.local_name(root) != Some("container") {
        return Err(refuse(&doc, root, "the root element is not <container>"));
    }
    let mut r = Reader {
        doc: &doc,
        graph: Graph::default(),
        parameters: dumper_parameters.iter().cloned().collect(),
        inline_count: 0,
    };
    r.graph.parameters.extend(dumper_parameters.iter().cloned());
    for child in r.elements(root) {
        match doc.local_name(child) {
            Some("parameters") => {
                for p in r.elements(child) {
                    r.expect(p, "parameter", &["key", "type"])?;
                    let key = doc.get_attribute(p, "key").to_owned();
                    let value = r.parameter_value(p)?;
                    r.parameters.insert(key.clone(), value.clone());
                    r.graph.parameters.push((key, value));
                }
            }
            Some("services") => {
                for s in r.elements(child) {
                    r.expect(s, "service", SERVICE_ATTRIBUTES)?;
                    if doc.has_attribute(s, "alias") {
                        r.graph.aliases.push(Alias {
                            id: doc.get_attribute(s, "id").to_owned(),
                            target: doc.get_attribute(s, "alias").to_owned(),
                            public: doc.get_attribute(s, "public") == "true",
                        });
                    } else {
                        let id = doc.get_attribute(s, "id").to_owned();
                        r.service(s, Some(id))?;
                    }
                }
            }
            _ => return Err(refuse(&doc, child, "unsupported element under <container>")),
        }
    }
    Ok(r.graph)
}

const SERVICE_ATTRIBUTES: &[&str] = &[
    "id",
    "class",
    "alias",
    "public",
    "shared",
    "synthetic",
    "lazy",
    "autowire",
    "autoconfigure",
    "constructor",
];

/// A string value as PhpDumper's `export()` reads it, with
/// `ParameterBag::resolveString()`'s pattern `/%%|%([^%\s]+)%/`: `%%` is
/// a literal `%`, `%env(...)%` is an env placeholder, `%name%` is a
/// parameter, and any other `%` is literal text (`"%d days"`).
fn split(s: &str, parameters: &HashMap<String, Value>, depth: u32) -> Result<Vec<StrPart>, String> {
    if depth > 32 {
        return Err(format!(
            "parameters refer to each other without end in \"{s}\""
        ));
    }
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        text.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("%%") {
            text.push('%');
            rest = r;
            continue;
        }
        let after = &rest[1..];
        let name = after
            .find('%')
            .map(|end| &after[..end])
            .filter(|n| !n.is_empty() && !n.chars().any(char::is_whitespace));
        let Some(name) = name else {
            text.push('%');
            rest = after;
            continue;
        };
        rest = &after[name.len() + 1..];
        if let Some(env) = name.strip_prefix("env(").and_then(|e| e.strip_suffix(')')) {
            if !text.is_empty() {
                parts.push(StrPart::Text(std::mem::take(&mut text)));
            }
            parts.push(StrPart::Env(env.to_owned()));
            continue;
        }
        match parameters.get(name) {
            Some(Value::String(v)) => {
                for p in split(
                    &String::from_utf8_lossy(v.as_bytes()),
                    parameters,
                    depth + 1,
                )? {
                    match p {
                        StrPart::Text(t) => text.push_str(&t),
                        env => {
                            if !text.is_empty() {
                                parts.push(StrPart::Text(std::mem::take(&mut text)));
                            }
                            parts.push(env);
                        }
                    }
                }
            }
            Some(Value::Int(n)) => text.push_str(&n.to_string()),
            Some(Value::Null) => {}
            Some(other) => {
                return Err(format!(
                    "A string value must be composed of strings and/or numbers, but found parameter \"{name}\" of type \"{}\" inside string value \"{s}\".",
                    other.debug_type()
                ));
            }
            None => {
                return Err(format!(
                    "You have requested a non-existent parameter \"{name}\"."
                ));
            }
        }
    }
    text.push_str(rest);
    if !text.is_empty() || parts.is_empty() {
        parts.push(StrPart::Text(text));
    }
    Ok(parts)
}

/// A string argument: `%name%` alone is the compiled parameter itself
/// (Symfony keeps whole-parameter references, so that `PhpDumper` can
/// share the parameter's value); otherwise text with `%%`, `%env()%` and
/// `%name%` parts.
fn string_arg(s: &str, parameters: &HashMap<String, Value>) -> Result<ArgDef, String> {
    if let Some(name) = s
        .strip_prefix('%')
        .and_then(|r| r.strip_suffix('%'))
        .filter(|n| {
            !n.is_empty()
                && !n.contains('%')
                && !n.chars().any(char::is_whitespace)
                && !n.starts_with("env(")
        })
    {
        let value = parameters
            .get(name)
            .ok_or_else(|| format!("You have requested a non-existent parameter \"{name}\"."))?;
        return value_arg(value, parameters);
    }
    let parts = split(s, parameters, 0)?;
    Ok(match parts.as_slice() {
        [StrPart::Text(t)] => ArgDef::Value(Value::string(t.as_str())),
        _ => ArgDef::Env(parts),
    })
}

/// A parameter's value as an argument. Parameter strings are escaped as
/// argument strings are (`%%`, `%env()%`).
fn value_arg(value: &Value, parameters: &HashMap<String, Value>) -> Result<ArgDef, String> {
    Ok(match value {
        Value::String(s) => string_arg(&String::from_utf8_lossy(s.as_bytes()), parameters)?,
        Value::Array(a) => ArgDef::Collection(
            a.iter()
                .map(|(k, v)| Ok((Some(k.clone()), value_arg(v, parameters)?)))
                .collect::<Result<_, String>>()?,
        ),
        v => ArgDef::Value(v.clone()),
    })
}

fn on_invalid(v: &str) -> Option<OnInvalid> {
    Some(match v {
        "" => OnInvalid::Exception,
        "null" => OnInvalid::Null,
        "ignore" => OnInvalid::Ignore,
        "ignore_uninitialized" => OnInvalid::IgnoreUninitialized,
        _ => return None,
    })
}

impl Reader<'_> {
    fn elements(&self, node: NodeId) -> Vec<NodeId> {
        self.doc
            .children(node)
            .iter()
            .copied()
            .filter(|&c| self.doc.node_type(c) == NodeType::Element)
            .collect()
    }

    fn expect(&self, node: NodeId, name: &str, attributes: &[&str]) -> Result<(), Error> {
        if self.doc.local_name(node) != Some(name) {
            return Err(refuse(
                self.doc,
                node,
                format!(
                    "unsupported element <{}>, expected <{name}>",
                    self.doc.node_name(node)
                ),
            ));
        }
        for a in self.doc.attributes(node) {
            if !attributes.contains(&a.local_name.as_str()) {
                return Err(refuse(
                    self.doc,
                    node,
                    format!(
                        "unsupported attribute {}=\"{}\" on <{name}>",
                        a.name, a.value
                    ),
                ));
            }
        }
        Ok(())
    }

    fn parameter_value(&self, p: NodeId) -> Result<Value, Error> {
        let doc = self.doc;
        Ok(match doc.get_attribute(p, "type") {
            "collection" => {
                let mut array = PhpArray::new();
                for c in self.elements(p) {
                    self.expect(c, "parameter", &["key", "type"])?;
                    let value = self.parameter_value(c)?;
                    if doc.has_attribute(c, "key") {
                        array.insert(
                            Key::from_bytes(doc.get_attribute(c, "key").as_bytes()),
                            value,
                        );
                    } else {
                        array
                            .push(value)
                            .map_err(|e| refuse(doc, c, e.to_string()))?;
                    }
                }
                Value::Array(array)
            }
            "string" => Value::string(doc.text_content(p)),
            "" => XmlUtils::phpize(doc.text_content(p).as_bytes()),
            other => {
                return Err(refuse(
                    doc,
                    p,
                    format!("unsupported parameter type \"{other}\""),
                ));
            }
        })
    }

    /// `<argument>` or `<property>` children of `node`, in order.
    fn arguments(&mut self, node: NodeId, element: &str) -> Result<Vec<(NodeId, ArgDef)>, Error> {
        let mut out = Vec::new();
        for a in self.elements(node) {
            if self.doc.local_name(a) != Some(element) {
                continue;
            }
            let value = self.argument(a)?;
            out.push((a, value));
        }
        Ok(out)
    }

    fn keyed(&mut self, node: NodeId) -> Result<Vec<(Option<Key>, ArgDef)>, Error> {
        let mut items = Vec::new();
        for (a, value) in self.arguments(node, "argument")? {
            let key = self
                .doc
                .has_attribute(a, "key")
                .then(|| Key::from_bytes(self.doc.get_attribute(a, "key").as_bytes()));
            items.push((key, value));
        }
        Ok(items)
    }

    fn argument(&mut self, a: NodeId) -> Result<ArgDef, Error> {
        let doc = self.doc;
        let element = doc.local_name(a).unwrap_or_default().to_owned();
        self.expect(a, &element, &["type", "id", "key", "name", "on-invalid"])?;
        let line = doc.line(a);
        let invalid = || {
            on_invalid(doc.get_attribute(a, "on-invalid")).ok_or_else(|| {
                refuse(
                    doc,
                    a,
                    format!(
                        "unsupported on-invalid=\"{}\"",
                        doc.get_attribute(a, "on-invalid")
                    ),
                )
            })
        };
        Ok(match doc.get_attribute(a, "type") {
            "service" if doc.has_attribute(a, "id") => ArgDef::Ref {
                id: doc.get_attribute(a, "id").to_owned(),
                on_invalid: invalid()?,
                line,
            },
            "service" => {
                let inner = self.elements(a);
                let [s] = inner.as_slice() else {
                    return Err(refuse(
                        doc,
                        a,
                        "an inline service argument needs one <service>",
                    ));
                };
                self.expect(*s, "service", SERVICE_ATTRIBUTES)?;
                ArgDef::Inline(self.service(*s, None)?)
            }
            "service_closure" => ArgDef::Closure {
                id: doc.get_attribute(a, "id").to_owned(),
                on_invalid: invalid()?,
                line,
            },
            "collection" => ArgDef::Collection(self.keyed(a)?),
            "iterator" => ArgDef::Iterator(self.keyed(a)?),
            "constant" => ArgDef::Constant(doc.text_content(a)),
            "string" => {
                string_arg(&doc.text_content(a), &self.parameters).map_err(|m| refuse(doc, a, m))?
            }
            "" => match XmlUtils::phpize(doc.text_content(a).as_bytes()) {
                Value::String(s) => {
                    string_arg(&String::from_utf8_lossy(s.as_bytes()), &self.parameters)
                        .map_err(|m| refuse(doc, a, m))?
                }
                v => ArgDef::Value(v),
            },
            // expression, binary, abstract, tagged_iterator,
            // tagged_locator, service_locator: not in the demo's
            // container; refused rather than guessed.
            other => {
                return Err(refuse(
                    doc,
                    a,
                    format!("unsupported argument type=\"{other}\""),
                ));
            }
        })
    }

    fn factory(&mut self, f: NodeId, own_class: &str) -> Result<FactoryDef, Error> {
        let doc = self.doc;
        let name = doc.local_name(f).unwrap_or_default().to_owned();
        self.expect(f, &name, &["class", "service", "method", "function"])?;
        if doc.has_attribute(f, "function") {
            return Ok(FactoryDef::Function(
                doc.get_attribute(f, "function").to_owned(),
            ));
        }
        let method = doc.get_attribute(f, "method").to_owned();
        if doc.has_attribute(f, "service") {
            return Ok(FactoryDef::Service {
                service: ArgDef::Ref {
                    id: doc.get_attribute(f, "service").to_owned(),
                    on_invalid: OnInvalid::Exception,
                    line: doc.line(f),
                },
                method,
            });
        }
        if doc.has_attribute(f, "class") {
            return Ok(FactoryDef::Static {
                class: doc.get_attribute(f, "class").to_owned(),
                method,
            });
        }
        match self.elements(f).as_slice() {
            [s] => {
                self.expect(*s, "service", SERVICE_ATTRIBUTES)?;
                let index = self.service(*s, None)?;
                Ok(FactoryDef::Service {
                    service: ArgDef::Inline(index),
                    method,
                })
            }
            // `[null, 'method']` on a configurator is not valid PHP; on a
            // factory XmlDumper writes the constructor attribute instead.
            [] => Ok(FactoryDef::Static {
                class: own_class.to_owned(),
                method,
            }),
            _ => Err(refuse(doc, f, "more than one inline service")),
        }
    }

    /// Reads one definition; returns its index in `Graph::services`.
    fn service(&mut self, s: NodeId, id: Option<String>) -> Result<usize, Error> {
        let doc = self.doc;
        let inline = id.is_none();
        let id = id.unwrap_or_else(|| {
            self.inline_count += 1;
            format!(".inline.{}", self.inline_count)
        });
        let mut class = doc.get_attribute(s, "class").to_owned();
        if class.is_empty() {
            // A factory result PHP does not type (a closure, a bool): the
            // placeholder type is "mixed".
            let has_factory = self
                .elements(s)
                .iter()
                .any(|&c| doc.local_name(c) == Some("factory"));
            if !has_factory {
                return Err(refuse(doc, s, format!("service \"{id}\" has no class")));
            }
            class = "mixed".to_owned();
        }
        let mut def = Definition {
            id,
            inline,
            class: class.clone(),
            line: doc.line(s),
            public: doc.get_attribute(s, "public") == "true",
            shared: doc.get_attribute(s, "shared") != "false",
            synthetic: doc.get_attribute(s, "synthetic") == "true",
            lazy: doc.get_attribute(s, "lazy") == "true",
            factory: None,
            arguments: Vec::new(),
            calls: Vec::new(),
            properties: Vec::new(),
            configurator: None,
            tags: Vec::new(),
        };
        if doc.has_attribute(s, "constructor") {
            def.factory = Some(FactoryDef::Static {
                class: class.clone(),
                method: doc.get_attribute(s, "constructor").to_owned(),
            });
        }
        // Reserve the slot first, so inline definitions read below get
        // later indexes and the parent keeps its own.
        let index = self.graph.services.len();
        self.graph.services.push(def.clone());
        for c in self.elements(s) {
            match doc.local_name(c) {
                Some("argument") => def.arguments.push(self.argument(c)?),
                Some("property") => {
                    let name = doc.get_attribute(c, "name").to_owned();
                    let value = self.argument(c)?;
                    def.properties.push((name, value));
                }
                Some("call") => {
                    self.expect(c, "call", &["method", "returns-clone"])?;
                    let arguments = self
                        .arguments(c, "argument")?
                        .into_iter()
                        .map(|(_, a)| a)
                        .collect();
                    def.calls.push(CallDef {
                        method: doc.get_attribute(c, "method").to_owned(),
                        returns_clone: doc.get_attribute(c, "returns-clone") == "true",
                        arguments,
                    });
                }
                Some("factory") => def.factory = Some(self.factory(c, &class)?),
                Some("configurator") => def.configurator = Some(self.factory(c, &class)?),
                Some("tag") => {
                    let attributes = doc
                        .attributes(c)
                        .iter()
                        .filter(|a| a.local_name != "name")
                        .map(|a| (a.local_name.clone(), a.value.clone()))
                        .collect();
                    def.tags
                        .push((doc.get_attribute(c, "name").to_owned(), attributes));
                }
                // A deprecation notice for users of the id; it changes
                // nothing in how the service is built.
                Some("deprecated") => {}
                _ => {
                    return Err(refuse(
                        doc,
                        c,
                        format!("unsupported element <{}> in a service", doc.node_name(c)),
                    ));
                }
            }
        }
        self.graph.services[index] = def;
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_placeholders_and_escaped_percent() {
        let mut parameters = HashMap::new();
        parameters.insert("p".to_owned(), Value::string("v%%w"));
        assert_eq!(
            split("a%%b%env(X)%c%p%%d days", &parameters, 0),
            Ok(vec![
                StrPart::Text("a%b".into()),
                StrPart::Env("X".into()),
                StrPart::Text("cv%w%d days".into())
            ])
        );
        assert!(split("%missing%", &parameters, 0).is_err());
    }
}

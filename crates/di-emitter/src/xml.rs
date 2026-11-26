//! Reads the compiled container as Symfony's `XmlDumper` writes it
//! (`Dumper/XmlDumper.php`), after the compiler passes: the input for
//! containers recorded from PHP. Values are read as the DI component's
//! `XmlFileLoader` reads them (`XmlUtils::phpize()` for untyped text).
//! Anything else is refused with its line.
//!
//! Upstream: none

use php_dom::{Document, LoadOptions, NodeId, NodeType, load_xml};
use php_value::{Key, PhpArray, Value};
use symfony_config::XmlUtils;

use crate::model::{
    Alias, ArgDef, CallDef, Definition, DumperParameters, FactoryDef, Graph, OnInvalid, Origin,
    Parameters, string_arg, unescape,
};
use crate::{Error, Location};

struct Reader<'a> {
    doc: &'a Document,
    graph: Graph,
    parameters: Parameters,
    inline_count: usize,
}

fn refuse(doc: &Document, node: NodeId, message: impl Into<String>) -> Error {
    Error::new(Location::Line(doc.line(node)), message)
}

fn origin(doc: &Document, node: NodeId) -> Origin {
    Origin(Location::Line(doc.line(node)))
}

/// Parses the dump. Every element and attribute the emitter does not
/// understand is refused.
pub fn read(xml: &[u8], dumper_parameters: &DumperParameters) -> Result<Graph, Error> {
    let loaded = load_xml(xml, LoadOptions::default())
        .map_err(|e| Error::new(Location::Unknown, format!("the file is not XML: {e:?}")))?;
    let doc = loaded.document.ok_or_else(|| {
        Error::new(
            Location::Line(loaded.errors.first().map_or(0, |e| e.line)),
            format!(
                "the file is not well-formed XML: {:?}",
                loaded.errors.first()
            ),
        )
    })?;
    let root = doc
        .document_element()
        .ok_or_else(|| Error::new(Location::Unknown, "the file has no root element"))?;
    if doc.local_name(root) != Some("container") {
        return Err(refuse(&doc, root, "the root element is not <container>"));
    }
    let mut r = Reader {
        doc: &doc,
        graph: Graph::default(),
        parameters: dumper_parameters.0.iter().cloned().collect(),
        inline_count: 0,
    };
    r.graph
        .parameters
        .extend(dumper_parameters.0.iter().cloned());
    for child in r.elements(root) {
        match doc.local_name(child) {
            Some("parameters") => {
                for p in r.elements(child) {
                    r.expect(p, "parameter", &["key", "type"])?;
                    let key = doc.get_attribute(p, "key").to_owned();
                    let value = r.parameter_value(p)?;
                    r.graph.parameters.push((key.clone(), unescape(&value)));
                    r.parameters.insert(key, value);
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
                            origin: origin(&doc, s),
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

    /// The items of a collection or iterator: `XmlDumper` writes them as
    /// elements of the collection's own name (`<property>` inside a
    /// property).
    fn keyed(&mut self, node: NodeId) -> Result<Vec<(Option<Key>, ArgDef)>, Error> {
        let element = self.doc.local_name(node).unwrap_or_default().to_owned();
        let mut items = Vec::new();
        for (a, value) in self.arguments(node, &element)? {
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
                origin: origin(doc, a),
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
            "service_closure" if !doc.has_attribute(a, "id") => {
                return Err(refuse(
                    doc,
                    a,
                    "unsupported argument type=\"service_closure\" of a value that is not a reference",
                ));
            }
            "service_closure" => ArgDef::Closure {
                id: doc.get_attribute(a, "id").to_owned(),
                on_invalid: invalid()?,
                origin: origin(doc, a),
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
            // tagged_locator, service_locator, env_closure: refused
            // rather than guessed.
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
                    origin: origin(doc, f),
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

    /// A `<tag>`'s name: its text when it has an attribute called `name`
    /// (`<tag name="attribute value">tag name</tag>`), else its `name`.
    fn tag_name(&self, t: NodeId) -> String {
        let doc = self.doc;
        if self.elements(t).is_empty() {
            let text = doc.text_content(t);
            if !text.is_empty() {
                return text;
            }
        }
        doc.get_attribute(t, "name").to_owned()
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
            "mixed".clone_into(&mut class);
        }
        let mut def = Definition {
            id,
            inline,
            class: class.clone(),
            origin: origin(doc, s),
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
                Some("tag") => def.tags.push(self.tag_name(c)),
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

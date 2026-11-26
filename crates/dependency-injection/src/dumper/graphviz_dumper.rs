//! Port of Symfony\Component\DependencyInjection\Dumper\GraphvizDumper.php.
//!
//! Upstream finds the nodes on a copy of the builder whose definitions
//! are all `stdClass`, and asks that copy for every other service id:
//! the copy has the definitions, the aliases and `service_container`,
//! so the only id it can be asked for is `service_container` when no
//! definition has that id, and the answer is the copy itself, a
//! `ContainerBuilder`. The port gives that node without the copy.
//!
//! Upstream: symfony/dependency-injection Dumper/GraphvizDumper.php

use std::fmt::Write as _;

use php_value::compare::loose_equals;
use php_value::{PhpArray, PhpString, Value, array::Key};

use super::{Dumper, DumperInterface, to_php_string};
use crate::argument::Argument;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::definition::Definition;
use crate::exception::{ErrorKind, Result};
use crate::id_map::IdMap;
use crate::reference::{Parameter, Reference};

const CONTAINER_BUILDER: &str = "Symfony\\Component\\DependencyInjection\\ContainerBuilder";

const OPTION_KEYS: [&str; 6] = [
    "graph",
    "node",
    "edge",
    "node.instance",
    "node.definition",
    "node.missing",
];

#[derive(Debug, Clone)]
struct Node {
    class: String,
    attributes: PhpArray,
}

#[derive(Debug, Clone)]
struct Edge {
    name: String,
    required: bool,
    to: String,
    lazy: bool,
}

/// `GraphvizDumper`: dumps a service container as a graphviz file.
///
/// The options of [`DumperInterface::dump`]: `graph`, `node`, `edge`
/// (the defaults for the whole graph, its nodes and edges),
/// `node.instance`, `node.definition` and `node.missing` (for services
/// defined by an object, by a definition, and missing services).
#[derive(Debug)]
pub struct GraphvizDumper<'a> {
    base: Dumper<'a>,
    nodes: IdMap<Node>,
    edges: IdMap<Vec<Edge>>,
    options: IdMap<PhpArray>,
}

fn options(pairs: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in pairs {
        array.insert(*k, Value::from(*v));
    }
    array
}

/// `array_merge($a, $b)`.
fn array_merge(a: &PhpArray, b: &PhpArray) -> PhpArray {
    let mut out = PhpArray::new();
    for (k, v) in a.iter().chain(b.iter()) {
        match k {
            Key::Int(_) => {
                let _ = out.push(v.clone());
            }
            Key::Str(_) => out.insert(k.clone(), v.clone()),
        }
    }
    out
}

impl<'a> GraphvizDumper<'a> {
    /// `new GraphvizDumper($container)`.
    pub fn new(container: &'a mut ContainerBuilder) -> Self {
        let mut defaults = IdMap::new();
        defaults.insert("graph", options(&[("ratio", "compress")]));
        defaults.insert(
            "node",
            options(&[
                ("fontsize", "11"),
                ("fontname", "Arial"),
                ("shape", "record"),
            ]),
        );
        defaults.insert(
            "edge",
            options(&[
                ("fontsize", "9"),
                ("fontname", "Arial"),
                ("color", "grey"),
                ("arrowhead", "open"),
                ("arrowsize", "0.5"),
            ]),
        );
        defaults.insert(
            "node.instance",
            options(&[("fillcolor", "#9999ff"), ("style", "filled")]),
        );
        defaults.insert("node.definition", options(&[("fillcolor", "#eeeeee")]));
        defaults.insert(
            "node.missing",
            options(&[("fillcolor", "#ff9999"), ("style", "filled")]),
        );
        Self {
            base: Dumper::new(container),
            nodes: IdMap::new(),
            edges: IdMap::new(),
            options: defaults,
        }
    }

    fn option(&self, key: &str) -> PhpArray {
        self.options.get(key).cloned().unwrap_or_default()
    }

    /// `dump($options)`: the container as a graphviz graph.
    pub fn dump_dot(&mut self, options: &PhpArray) -> Result<PhpString> {
        for key in OPTION_KEYS {
            if let Some(Value::Array(given)) = options.get(&Key::from(key)) {
                let merged = array_merge(&self.option(key), given);
                self.options.insert(key, merged);
            }
        }

        self.nodes = self.find_nodes()?;

        self.edges = IdMap::new();
        let definitions: Vec<(String, Definition)> = self
            .base
            .container()
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in &definitions {
            let mut edges = self.find_edges(&definition.arguments(), true, "", false)?;
            edges.extend(self.find_edges(&definition.properties(), false, "", false)?);

            for call in definition.method_calls().values() {
                let (method, arguments) = call_parts(call);
                edges.extend(self.find_edges(&arguments, false, &format!("{method}()"), false)?);
            }
            self.edges.insert(id.as_str(), edges);
        }

        let mut dot = self.start_dot();
        dot.extend(self.add_nodes());
        dot.extend(self.add_edges());
        dot.extend_from_slice(b"}\n");
        let resolved = self.base.container_mut().resolve_env_placeholders(
            Value::String(PhpString::from(dot)),
            &EnvFormat::Sprintf("__ENV_%s__".to_string()),
            None,
        )?;
        Ok(to_php_string(&resolved))
    }

    fn add_nodes(&self) -> Vec<u8> {
        let mut code = Vec::new();
        let shape = to_php_string(
            self.option("node")
                .get(&Key::from("shape"))
                .unwrap_or(&Value::Null),
        );
        for (id, node) in self.nodes.iter() {
            let aliases = self.get_aliases(id);
            let label = if aliases.is_empty() {
                id.to_string()
            } else {
                format!("{id} ({})", aliases.join(", "))
            };

            code.extend_from_slice(
                format!(
                    "  node_{} [label=\"{label}\\n{}\\n\", shape=",
                    dotize(id),
                    node.class
                )
                .as_bytes(),
            );
            code.extend_from_slice(shape.as_bytes());
            code.extend(add_attributes(&node.attributes));
            code.extend_from_slice(b"];\n");
        }
        code
    }

    fn add_edges(&self) -> Vec<u8> {
        let mut code = String::new();
        for (id, edges) in self.edges.iter() {
            for edge in edges {
                let _ = writeln!(
                    code,
                    "  node_{} -> node_{} [label=\"{}\" style=\"{}\"{}];",
                    dotize(id),
                    dotize(&edge.to),
                    edge.name,
                    if edge.required { "filled" } else { "dashed" },
                    if edge.lazy { " color=\"#9999ff\"" } else { "" }
                );
            }
        }
        code.into_bytes()
    }

    /// `findEdges()`: the edges of the service `id`.
    fn find_edges(
        &mut self,
        arguments: &PhpArray,
        required: bool,
        name: &str,
        lazy: bool,
    ) -> Result<Vec<Edge>> {
        let mut edges = Vec::new();
        for argument in arguments.values() {
            let mut argument = argument.clone();
            if let Some(parameter) = Parameter::from_value(&argument) {
                let name = parameter.id().to_string();
                argument = self.parameter_or_null(&name)?;
            } else if let Value::String(s) = &argument
                && let Some(name) = whole_placeholder(s.as_bytes())
            {
                argument = self.parameter_or_null(&name)?;
            }

            if let Some(reference) = Reference::from_value(&argument) {
                let to = reference.id().to_string();
                let mut lazy_edge = lazy;

                if !self.base.container().has(&to) {
                    self.nodes.insert(
                        to.as_str(),
                        Node {
                            class: String::new(),
                            attributes: self.option("node.missing"),
                        },
                    );
                } else if to != "service_container" {
                    lazy_edge = lazy || self.base.container().get_definition(&to)?.is_lazy();
                }

                edges.push(Edge {
                    name: name.to_string(),
                    required,
                    to,
                    lazy: lazy_edge,
                });
            } else if let Some(argument) = Argument::from_value(&argument) {
                edges.extend(self.find_edges(&argument.values(), required, name, true)?);
            } else if let Some(definition) = Definition::from_value(&argument) {
                edges.extend(self.find_edges(&definition.arguments(), required, "", false)?);
                edges.extend(self.find_edges(&definition.properties(), false, "", false)?);

                for call in definition.method_calls().values() {
                    let (method, arguments) = call_parts(call);
                    edges.extend(self.find_edges(
                        &arguments,
                        false,
                        &format!("{method}()"),
                        false,
                    )?);
                }
            } else if let Value::Array(array) = &argument {
                edges.extend(self.find_edges(array, required, name, lazy)?);
            }
        }

        Ok(edges)
    }

    /// `hasParameter($name) ? getParameter($name) : null`.
    fn parameter_or_null(&self, name: &str) -> Result<Value> {
        let container = self.base.container();
        if container.has_parameter(name) {
            container.get_parameter(name)
        } else {
            Ok(Value::Null)
        }
    }

    fn find_nodes(&self) -> Result<IdMap<Node>> {
        let mut nodes = IdMap::new();
        let container = self.base.container();

        for (id, definition) in container.get_definitions().iter() {
            let class = definition.class().unwrap_or_default();
            let class = class.strip_prefix('\\').unwrap_or(&class).to_string();

            let class = match container
                .get_parameter_bag()
                .resolve_value(&Value::from(class.as_str()), &[])
            {
                Ok(resolved) => to_php_string(&resolved).to_string_lossy().into_owned(),
                Err(e) if *e.kind() == ErrorKind::ParameterNotFound => class,
                Err(e) => return Err(e),
            };

            let mut style = PhpArray::new();
            style.insert(
                "style",
                Value::from(if definition.is_shared() {
                    "filled"
                } else {
                    "dotted"
                }),
            );
            nodes.insert(
                id,
                Node {
                    class: class.replace('\\', "\\\\"),
                    attributes: array_merge(&self.option("node.definition"), &style),
                },
            );
        }

        let id = "service_container";
        if !container.has_definition(id) && !container.get_aliases().contains_key(id) {
            nodes.insert(
                id,
                Node {
                    class: CONTAINER_BUILDER.replace('\\', "\\\\"),
                    attributes: self.option("node.instance"),
                },
            );
        }

        Ok(nodes)
    }

    fn start_dot(&self) -> Vec<u8> {
        let mut out = b"digraph sc {\n  ".to_vec();
        out.extend(add_options(&self.option("graph")));
        out.extend_from_slice(b"\n  node [");
        out.extend(add_options(&self.option("node")));
        out.extend_from_slice(b"];\n  edge [");
        out.extend(add_options(&self.option("edge")));
        out.extend_from_slice(b"];\n\n");
        out
    }

    fn get_aliases(&self, id: &str) -> Vec<String> {
        let mut aliases = Vec::new();
        for (alias, origin) in self.base.container().get_aliases().iter() {
            if loose_equals(&Value::from(id), &Value::from(origin.id())) {
                aliases.push(alias.to_string());
            }
        }
        aliases
    }
}

impl DumperInterface for GraphvizDumper<'_> {
    fn dump(&mut self, options: &PhpArray) -> Result<PhpString> {
        self.dump_dot(options)
    }
}

/// A method call's `[$method, $arguments]`.
fn call_parts(call: &Value) -> (String, PhpArray) {
    let Value::Array(call) = call else {
        return (String::new(), PhpArray::new());
    };
    let method = call
        .get(&Key::Int(0))
        .map(|m| to_php_string(m).to_string_lossy().into_owned())
        .unwrap_or_default();
    let arguments = match call.get(&Key::Int(1)) {
        Some(Value::Array(arguments)) => arguments.clone(),
        _ => PhpArray::new(),
    };
    (method, arguments)
}

/// `preg_match('/^%([^%]+)%$/', $argument, $match) ? $match[1] : null`.
fn whole_placeholder(bytes: &[u8]) -> Option<String> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let inner = bytes.strip_prefix(b"%")?.strip_suffix(b"%")?;
    if inner.is_empty() || inner.contains(&b'%') {
        return None;
    }
    Some(String::from_utf8_lossy(inner).into_owned())
}

fn add_attributes(attributes: &PhpArray) -> Vec<u8> {
    let code = key_values(attributes);
    if code.is_empty() {
        return Vec::new();
    }
    let mut out = b", ".to_vec();
    out.extend(code.join(&b", "[..]));
    out
}

fn add_options(options: &PhpArray) -> Vec<u8> {
    key_values(options).join(&b" "[..])
}

/// `sprintf('%s="%s"', $k, $v)` of each entry.
fn key_values(array: &PhpArray) -> Vec<Vec<u8>> {
    array
        .iter()
        .map(|(k, v)| {
            let mut entry = k.bytes().to_vec();
            entry.extend_from_slice(b"=\"");
            entry.extend_from_slice(to_php_string(v).as_bytes());
            entry.push(b'"');
            entry
        })
        .collect()
}

/// `preg_replace('/\W/i', '_', $id)`: every byte that is not an ASCII
/// letter, digit or `_` becomes `_`.
fn dotize(id: &str) -> String {
    id.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'_' {
                char::from(b)
            } else {
                '_'
            }
        })
        .collect()
}

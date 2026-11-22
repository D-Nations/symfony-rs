//! Port of Symfony\Component\Config\Definition\Dumper\XmlReferenceDumper.php:
//! the XML reference of a configuration tree (`config:dump-reference
//! --format=xml`).
//!
//! `PHP_EOL` is `"\n"`. Dumping renames a prototype that is itself a
//! PrototypedArrayNode to its parent's key attribute, as upstream's
//! `setName()` does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Dumper/XmlReferenceDumper.php

use php_value::{PhpString, Value};

use crate::{
    definition::{
        ConfigurationInterface, Node, base_node::undefined_method, boolean_node, enum_node,
        exception::Error, float_node, integer_node, scalar_node,
    },
    php,
};

const PHP_EOL: &[u8] = b"\n";

/// The value upstream marks an attribute without a default with.
const NOT_DEFINED: &[u8] = b"%%%%not_defined%%%%";

/// `class XmlReferenceDumper`.
#[derive(Debug, Default)]
pub struct XmlReferenceDumper {
    reference: Option<Vec<u8>>,
}

impl XmlReferenceDumper {
    pub fn new() -> Self {
        Self::default()
    }

    /// `dump($configuration, $namespace)`.
    pub fn dump(
        &mut self,
        configuration: &dyn ConfigurationInterface,
        namespace: Option<&[u8]>,
    ) -> Result<Vec<u8>, Error> {
        let tree = configuration.get_config_tree_builder()?.build_tree()?;
        self.dump_node(&tree, namespace)
    }

    /// `dumpNode($node, $namespace)`.
    pub fn dump_node(&mut self, node: &Node, namespace: Option<&[u8]>) -> Result<Vec<u8>, Error> {
        self.reference = Some(Vec::new());
        let result = self.write_node(node, 0, true, namespace);
        let reference = self.reference.take().unwrap_or_default();
        result.map(|()| reference)
    }

    fn write_node(
        &mut self,
        node: &Node,
        depth: usize,
        root: bool,
        namespace: Option<&[u8]>,
    ) -> Result<(), Error> {
        let mut root_name: Vec<u8> = if root {
            b"config".to_vec()
        } else {
            node.get_name().to_vec()
        };
        // `$namespace ?: (...)`: an empty or "0" namespace is not one.
        let root_namespace: Option<Vec<u8>> =
            match namespace.filter(|n| php::truthy(&Value::from(PhpString::from(*n)))) {
                Some(n) => Some(n.to_vec()),
                None if root => {
                    let mut ns = b"http://example.org/schema/dic/".to_vec();
                    ns.extend_from_slice(&node.get_name());
                    Some(ns)
                }
                None => None,
            };

        // xml remapping; `getParent()` is BaseNode's, not NodeInterface's
        if !node.is_base_node() {
            return Err(undefined_method(&node.class(), "getParent"));
        }
        if let Some(parent) = node.get_parent() {
            let remapping = parent
                .get_xml_remappings()?
                .into_iter()
                .find(|(_, plural)| plural.as_bytes() == root_name.as_slice());

            if let Some((singular, _)) = remapping {
                root_name = singular.to_vec();
            }
        }
        let root_name = replace_byte(&root_name, b'_', b'-');

        let mut root_attributes: Vec<(Vec<u8>, Value)> = Vec::new();
        let mut root_attribute_comments: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        let mut root_children: Vec<Node> = Vec::new();
        let mut root_comments: Vec<Vec<u8>> = Vec::new();
        // `isset($prototypeValue)`: set and not null.
        let mut prototype_value: Option<Value> = None;

        if node.is_array_node() {
            let mut children = node.get_children()?;

            // comments about the root node
            if let Some(root_info) = node.hint() {
                root_comments.push(root_info.to_vec());
            }

            if let Some(ns) = &root_namespace {
                let mut comment = b"Namespace: ".to_vec();
                comment.extend_from_slice(ns);
                root_comments.push(comment);
            }

            // render prototyped nodes
            if node.is_prototyped_array_node() {
                let prototype = node.get_prototype()?;

                let mut info = b"prototype".to_vec();
                if let Some(prototype_info) = prototype.get_info() {
                    info.extend_from_slice(b": ");
                    info.extend_from_slice(&prototype_info);
                }
                root_comments.insert(0, info);

                let key = node.get_key_attribute()?;
                if let Some(key) = key
                    .as_ref()
                    .filter(|k| php::truthy(&Value::String((*k).clone())))
                {
                    let mut value = replace_byte(&root_name, b'-', b' ');
                    value.push(b' ');
                    value.extend_from_slice(key);
                    set_attribute(
                        &mut root_attributes,
                        key.to_vec(),
                        Value::from(PhpString::from(value)),
                    );
                }

                if prototype.is_prototyped_array_node() {
                    let key = key.map(|k| k.to_vec()).unwrap_or_default();
                    prototype.set_name(&key)?;
                    children = vec![(php_value::Key::from_bytes(&key), prototype)];
                } else if prototype.is_array_node() {
                    children = prototype.get_children()?;
                } else {
                    let value = if prototype.has_default_value() {
                        prototype.get_default_value()?
                    } else {
                        let class = prototype.class();
                        let text: Vec<u8> = if class == scalar_node::CLASS {
                            b"scalar value".to_vec()
                        } else if class == float_node::CLASS || class == integer_node::CLASS {
                            b"numeric value".to_vec()
                        } else if class == boolean_node::CLASS {
                            b"true|false".to_vec()
                        } else if class == enum_node::CLASS {
                            prototype.get_permissible_values(b"|", true)?
                        } else {
                            b"value".to_vec()
                        };
                        Value::from(PhpString::from(text))
                    };
                    prototype_value = Some(value).filter(|v| !v.is_null());
                }
            }

            // get attributes and elements
            for (_, child) in &children {
                if child.is_array_node() {
                    // get elements
                    root_children.push(child.clone());

                    continue;
                }

                // get attributes

                // metadata
                let name = replace_byte(&child.get_name(), b'_', b'-');
                // use a string which isn't used in the normal world
                let mut value = Value::from(PhpString::from(NOT_DEFINED));

                // comments
                let mut comments: Vec<Vec<u8>> = Vec::new();
                if child.is_base_node()
                    && let Some(info) = child.hint()
                {
                    comments.push(info.to_vec());
                }

                if child.is_base_node() {
                    let example = child.get_example();
                    if php::truthy(&example) {
                        let mut comment = b"Example: ".to_vec();
                        match &example {
                            Value::Array(items) => {
                                let mut pieces = Vec::with_capacity(items.len());
                                for item in items.values() {
                                    pieces.push(php::string_value(item)?);
                                }
                                comment.extend(php::implode(b", ", pieces));
                            }
                            other => comment.extend(php::string_value(other)?),
                        }
                        comments.push(comment);
                    }
                }

                if child.is_required() {
                    comments.push(b"Required".to_vec());
                }

                if child.is_base_node() && child.is_deprecated() {
                    let mut comment = b"Deprecated (".to_vec();
                    comment.extend(child.get_deprecation_message(Some(node))?);
                    comment.push(b')');
                    comments.push(comment);
                }

                if child.is_instance_of(enum_node::CLASS) {
                    let mut comment = b"One of ".to_vec();
                    comment.extend(child.get_permissible_values(b"; ", true)?);
                    comments.push(comment);
                }

                if !comments.is_empty() {
                    let comment = php::implode(b";\n", comments);
                    set_attribute(&mut root_attribute_comments, name.clone(), comment);
                }

                // default values
                if child.has_default_value() {
                    value = child.get_default_value()?;
                }

                // append attribute
                set_attribute(&mut root_attributes, name, value);
            }
        }

        // render comments

        // root node comment
        for comment in &root_comments {
            let mut line = b"<!-- ".to_vec();
            line.extend_from_slice(comment);
            line.extend_from_slice(b" -->");
            self.write_line(&line, depth);
        }

        // attribute comments
        for (attr_name, comment) in &root_attribute_comments {
            let comment_depth = depth + 4 + attr_name.len() + 2;
            let comment_lines: Vec<&[u8]> = comment.split(|&b| b == b'\n').collect();
            let multiline = comment_lines.len() > 1;
            let mut glue = PHP_EOL.to_vec();
            glue.extend(std::iter::repeat_n(b' ', comment_depth));
            let comment = php::implode(&glue, comment_lines.iter().map(|l| l.to_vec()));

            let mut text = attr_name.clone();
            text.extend_from_slice(b": ");
            text.extend_from_slice(&comment);
            if multiline {
                self.write_line(b"<!--", depth);
                self.write_line(&text, depth + 4);
                self.write_line(b"-->", depth);
            } else {
                let mut line = b"<!-- ".to_vec();
                line.extend_from_slice(&text);
                line.extend_from_slice(b" -->");
                self.write_line(&line, depth);
            }
        }

        // render start tag + attributes
        let root_is_variable_prototype = prototype_value.is_some();
        let root_is_empty_tag = root_children.is_empty() && !root_is_variable_prototype;
        let mut root_open_tag = b"<".to_vec();
        root_open_tag.extend_from_slice(&root_name);
        let attributes_count = root_attributes.len();
        if attributes_count <= 1 {
            if let Some((attr_name, attr_value)) = root_attributes.first() {
                root_open_tag.push(b' ');
                root_open_tag.extend_from_slice(attr_name);
                root_open_tag.extend_from_slice(b"=\"");
                root_open_tag.extend(write_value(attr_value)?);
                root_open_tag.push(b'"');
            }

            root_open_tag.extend_from_slice(if root_is_empty_tag { b" />" } else { b">" });

            if let Some(value) = &prototype_value {
                root_open_tag.extend(php::string_value(value)?);
                root_open_tag.extend_from_slice(b"</");
                root_open_tag.extend_from_slice(&root_name);
                root_open_tag.push(b'>');
            }

            self.write_line(&root_open_tag, depth);
        } else {
            self.write_line(&root_open_tag, depth);

            for (i, (attr_name, attr_value)) in root_attributes.iter().enumerate() {
                let mut attr = attr_name.clone();
                attr.extend_from_slice(b"=\"");
                attr.extend(write_value(attr_value)?);
                attr.push(b'"');

                self.write_line(&attr, depth + 4);

                if attributes_count == i + 1 {
                    self.write_line(if root_is_empty_tag { b"/>" } else { b">" }, depth);

                    // Upstream appends the value to a tag it has already
                    // written, so it is never output; the conversion
                    // still runs.
                    if let Some(value) = &prototype_value {
                        php::string_value(value)?;
                    }
                }
            }
        }

        // render children tags
        for child in &root_children {
            self.write_line(b"", 0);
            self.write_node(child, depth + 4, false, None)?;
        }

        // render end tag
        if !root_is_empty_tag && !root_is_variable_prototype {
            self.write_line(b"", 0);

            let mut root_end_tag = b"</".to_vec();
            root_end_tag.extend_from_slice(&root_name);
            root_end_tag.push(b'>');
            self.write_line(&root_end_tag, depth);
        }
        Ok(())
    }

    /// Outputs a single config reference line.
    fn write_line(&mut self, text: &[u8], indent: usize) {
        // sprintf('%'.(strlen($text) + $indent).'s', $text)
        let reference = self.reference.get_or_insert_with(Vec::new);
        reference.extend(std::iter::repeat_n(b' ', indent));
        reference.extend_from_slice(text);
        reference.extend_from_slice(PHP_EOL);
    }
}

/// `$array[$key] = $value`: replaces an existing key in place.
fn set_attribute<V>(list: &mut Vec<(Vec<u8>, V)>, key: Vec<u8>, value: V) {
    match list.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => list.push((key, value)),
    }
}

/// `str_replace($from, $to, $s)` for one byte.
fn replace_byte(s: &[u8], from: u8, to: u8) -> Vec<u8> {
    s.iter().map(|&b| if b == from { to } else { b }).collect()
}

/// Renders the string conversion of the value.
fn write_value(value: &Value) -> Result<Vec<u8>, Error> {
    if let Value::String(s) = value
        && s.as_bytes() == NOT_DEFINED
    {
        return Ok(Vec::new());
    }

    // `is_string($value) || is_numeric($value)`, returned as a string.
    match value {
        Value::String(s) => return Ok(s.to_vec()),
        Value::Int(_) | Value::Float(_) => return Ok(php::to_bytes(value)),
        _ => {}
    }

    match value {
        Value::Bool(false) => return Ok(b"false".to_vec()),
        Value::Bool(true) => return Ok(b"true".to_vec()),
        Value::Null => return Ok(b"null".to_vec()),
        _ => {}
    }

    if !php::truthy(value) {
        return Ok(Vec::new());
    }

    if let Value::Array(items) = value {
        let mut pieces = Vec::with_capacity(items.len());
        for item in items.values() {
            pieces.push(php::string_value(item)?);
        }
        return Ok(php::implode(b",", pieces));
    }

    Ok(Vec::new())
}

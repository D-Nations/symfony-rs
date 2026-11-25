//! Port of Symfony\Component\Config\Definition\Dumper\YamlReferenceDumper.php:
//! the YAML reference of a configuration tree (`config:dump-reference`),
//! with values written by symfony-yaml's `Inline::dump()`. Behind the
//! `yaml` feature, as upstream needs symfony/yaml installed.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Dumper/YamlReferenceDumper.php

use php_value::{Key, PhpArray, PhpString, Value};
use symfony_yaml::{Flags, Inline};

use crate::{
    definition::{
        ArrayNode, ConfigurationInterface, Node, ScalarNode, enum_node,
        exception::{Class, Error},
    },
    php,
};

/// `class YamlReferenceDumper`.
#[derive(Debug, Default)]
pub struct YamlReferenceDumper {
    reference: Option<Vec<u8>>,
    inline: Inline,
}

impl YamlReferenceDumper {
    pub fn new() -> Self {
        Self::default()
    }

    /// `dump($configuration)`.
    pub fn dump(&mut self, configuration: &dyn ConfigurationInterface) -> Result<Vec<u8>, Error> {
        let tree = configuration.get_config_tree_builder()?.build_tree()?;
        self.dump_node(&tree)
    }

    /// `dumpAtPath($configuration, $path)`: the reference of the node at a
    /// dot-separated path below the root.
    pub fn dump_at_path(
        &mut self,
        configuration: &dyn ConfigurationInterface,
        path: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let root_node = configuration.get_config_tree_builder()?.build_tree()?;
        self.dump_node_at_path(&root_node, path)
    }

    /// `dumpAtPath()` on a tree already built: the steps after
    /// `buildTree()`.
    pub fn dump_node_at_path(&mut self, root_node: &Node, path: &[u8]) -> Result<Vec<u8>, Error> {
        let mut node = root_node.clone();

        let not_found = || {
            let mut message = b"Unable to find node at path \"".to_vec();
            message.extend_from_slice(&root_node.get_name());
            message.push(b'.');
            message.extend_from_slice(path);
            message.extend_from_slice(b"\".");
            Error::new(Class::UnexpectedValue, message)
        };

        'steps: for step in path.split(|&b| b == b'.') {
            if !node.is_array_node() {
                return Err(not_found());
            }

            let children = if node.is_prototyped_array_node() {
                self.get_prototype_children(&node)?
            } else {
                node.get_children()?
            };

            for (_, child) in children {
                if child.get_name().as_bytes() == step {
                    node = child;

                    continue 'steps;
                }
            }

            return Err(not_found());
        }

        self.dump_node(&node)
    }

    /// `dumpNode($node)`.
    pub fn dump_node(&mut self, node: &Node) -> Result<Vec<u8>, Error> {
        self.reference = Some(Vec::new());
        let result = self.write_node(node, None, 0, false);
        let reference = self.reference.take().unwrap_or_default();
        result.map(|()| reference)
    }

    /// `Inline::dump($value)`.
    fn inline_dump(&self, value: &Value) -> Result<Vec<u8>, Error> {
        self.inline
            .dump(value, Flags::NONE, false)
            .map(|s| s.to_vec())
            .map_err(|e| {
                Error::new(
                    Class::Other {
                        class: "Symfony\\Component\\Yaml\\Exception\\DumpException".to_owned(),
                        exception: true,
                    },
                    e.message,
                )
            })
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn write_node(
        &mut self,
        node: &Node,
        parent_node: Option<&Node>,
        depth: usize,
        prototyped_array: bool,
    ) -> Result<(), Error> {
        let mut comments: Vec<Vec<u8>> = Vec::new();
        let mut default: Vec<u8> = Vec::new();
        let mut default_array: Option<PhpArray> = None;
        let mut children: Option<Vec<(Key, Node)>> = None;
        let example = if node.is_base_node() {
            node.get_example()
        } else {
            Value::Null
        };

        // defaults
        if node.is_array_node() {
            let mut own = node.get_children()?;

            if node.is_prototyped_array_node() {
                own = self.get_prototype_children(node)?;
            }

            if own.is_empty() {
                // `$node->hasDefaultValue() && $defaultArray = $node->getDefaultValue()`
                let has_default = node.has_default_value();
                let value = if has_default {
                    Some(node.get_default_value()?)
                } else {
                    None
                };
                let truthy = value.as_ref().is_some_and(php::truthy);
                if truthy {
                    default_array = value.and_then(|v| match v {
                        Value::Array(a) => Some(a),
                        _ => None,
                    });
                } else {
                    default = if has_default && value.as_ref().is_some_and(Value::is_null) {
                        b"~".to_vec()
                    } else {
                        b"[]".to_vec()
                    };
                }
            }
            children = Some(own);
        } else if node.is_instance_of(enum_node::CLASS) {
            let mut comment = b"One of ".to_vec();
            comment.extend(node.get_permissible_values(b"; ", true)?);
            comments.push(comment);
            default = if node.has_default_value() {
                self.inline_dump(&node.get_default_value()?)?
            } else {
                b"~".to_vec()
            };
        } else {
            default = b"~".to_vec();

            if node.has_default_value() {
                let value = node.get_default_value()?;

                if let Value::Array(_) = value {
                    // Upstream reads the default a second time here.
                    let second = node.get_default_value()?;
                    let count = match &second {
                        Value::Array(a) => a.len(),
                        other => {
                            return Err(Error::new(
                                Class::TypeError,
                                format!(
                                    "count(): Argument #1 ($value) must be of type Countable|array, {} given",
                                    other.debug_type()
                                ),
                            ));
                        }
                    };
                    if count > 0 {
                        default = Vec::new();
                        if let Value::Array(a) = second {
                            default_array = Some(a);
                        }
                    } else if !matches!(example, Value::Array(_)) {
                        default = b"[]".to_vec();
                    } else {
                        // The empty array stays, and `(string)` makes it
                        // "Array" (PHP also warns).
                        default = b"Array".to_vec();
                    }
                } else {
                    default = self.inline_dump(&value)?;
                }
            }
        }

        // required?
        if node.is_required() {
            comments.push(b"Required".to_vec());
        }

        // deprecated?
        if node.is_base_node() && node.is_deprecated() {
            let mut comment = b"Deprecated (".to_vec();
            comment.extend(node.get_deprecation_message(parent_node)?);
            comment.push(b')');
            comments.push(comment);
        }

        // example
        if php::truthy(&example) && !matches!(example, Value::Array(_)) {
            let mut comment = b"Example: ".to_vec();
            comment.extend(self.inline_dump(&example)?);
            comments.push(comment);
        }

        let default = if default.is_empty() {
            Vec::new()
        } else {
            let mut d = b" ".to_vec();
            d.extend(default);
            d
        };
        let comments = if comments.is_empty() {
            Vec::new()
        } else {
            let mut c = b"# ".to_vec();
            c.extend(php::implode(b", ", comments));
            c
        };

        let key: Vec<u8> = if prototyped_array {
            b"-".to_vec()
        } else {
            let mut k = node.get_name().to_vec();
            k.push(b':');
            k
        };
        // rtrim(sprintf('%-21s%s %s', $key, $default, $comments), ' ')
        let mut text = key;
        if text.len() < 21 {
            text.resize(21, b' ');
        }
        text.extend(default);
        text.push(b' ');
        text.extend(comments);
        while text.last() == Some(&b' ') {
            text.pop();
        }

        if node.is_base_node()
            && let Some(info) = node.hint()
        {
            self.write_line(b"", 0);
            // indenting multi-line info
            let mut replacement = b"\n".to_vec();
            replacement.extend(sprintf_s_width(depth * 4, b" "));
            replacement.extend_from_slice(b"# ");
            let info = replace(&info, b"\n", &replacement);
            let mut line = b"# ".to_vec();
            line.extend(info);
            self.write_line(&line, depth * 4);
        }

        self.write_line(&text, depth * 4);

        // output defaults
        if let Some(default_array) = default_array.filter(|a| !a.is_empty()) {
            self.write_line(b"", 0);

            let message: &[u8] = if default_array.len() > 1 {
                b"Defaults"
            } else {
                b"Default"
            };

            let mut line = b"# ".to_vec();
            line.extend_from_slice(message);
            line.push(b':');
            self.write_line(&line, depth * 4 + 4);

            let walked = self.dump_leaves(&default_array)?;
            self.write_array(&walked, depth + 1, false)?;
        }

        if let Value::Array(example) = &example {
            self.write_line(b"", 0);

            let message: &[u8] = if example.len() > 1 {
                b"Examples"
            } else {
                b"Example"
            };

            let mut line = b"# ".to_vec();
            line.extend_from_slice(message);
            line.push(b':');
            self.write_line(&line, depth * 4 + 4);

            // array_map(Inline::dump(...), $example)
            let mut mapped = PhpArray::new();
            for (k, v) in example.iter() {
                mapped.insert(
                    k.clone(),
                    Value::from(PhpString::from(self.inline_dump(v)?)),
                );
            }
            self.write_array(&mapped, depth + 1, true)?;
        }

        if let Some(children) = children.filter(|c| !c.is_empty()) {
            let prototyped = node.is_prototyped_array_node()
                && !node
                    .get_key_attribute()?
                    .is_some_and(|k| php::truthy(&Value::String(k)));
            for (_, child_node) in &children {
                self.write_node(child_node, Some(node), depth + 1, prototyped)?;
            }
        }
        Ok(())
    }

    /// `array_walk_recursive()` with `Inline::dump()` on every leaf.
    fn dump_leaves(&self, array: &PhpArray) -> Result<PhpArray, Error> {
        let mut out = PhpArray::new();
        for (k, v) in array.iter() {
            let v = match v {
                Value::Array(inner) => Value::Array(self.dump_leaves(inner)?),
                leaf => Value::from(PhpString::from(self.inline_dump(leaf)?)),
            };
            out.insert(k.clone(), v);
        }
        Ok(out)
    }

    /// Outputs a single config reference line.
    fn write_line(&mut self, text: &[u8], indent: usize) {
        // sprintf('%'.(strlen($text) + $indent).'s', $text)
        let reference = self.reference.get_or_insert_with(Vec::new);
        reference.extend(std::iter::repeat_n(b' ', indent));
        reference.extend_from_slice(text);
        reference.push(b'\n');
    }

    fn write_array(
        &mut self,
        array: &PhpArray,
        depth: usize,
        as_comment: bool,
    ) -> Result<(), Error> {
        let is_indexed = array.is_list();

        for (key, value) in array.iter() {
            let val: Vec<u8> = match value {
                Value::Array(a) if a.is_empty() => b"[]".to_vec(),
                Value::Array(_) => Vec::new(),
                other => php::string_value(other)?,
            };

            let prefix: &[u8] = if as_comment { b"# " } else { b"" };

            let mut line = prefix.to_vec();
            if is_indexed {
                line.extend_from_slice(b"- ");
                line.extend(val);
            } else {
                // sprintf('%s%-20s %s', $prefix, Inline::dump($key).':', $val)
                let mut k = self.inline_dump(&key.to_value())?;
                k.push(b':');
                if k.len() < 20 {
                    k.resize(20, b' ');
                }
                line.extend(k);
                line.push(b' ');
                line.extend(val);
            }
            self.write_line(&line, depth * 4);

            if let Value::Array(inner) = value {
                self.write_array(inner, depth + 1, as_comment)?;
            }
        }
        Ok(())
    }

    #[expect(
        clippy::self_only_used_in_recursion,
        reason = "a method, as upstream's"
    )]
    fn get_prototype_children(&self, node: &Node) -> Result<Vec<(Key, Node)>, Error> {
        let prototype = node.get_prototype()?;
        let key = node.get_key_attribute()?;
        let key_truthy = key
            .as_ref()
            .is_some_and(|k| php::truthy(&Value::String(k.clone())));

        // Do not expand prototype if it isn't an array node nor uses attribute as key
        if !key_truthy && !prototype.is_array_node() {
            return node.get_children();
        }

        let key_name = key.map(|k| k.to_vec()).unwrap_or_default();
        let key_node = if prototype.is_array_node() {
            let key_node = ArrayNode::new(&key_name, Some(node))?;
            let mut children = prototype.get_children()?;

            if prototype.is_prototyped_array_node()
                && prototype
                    .get_key_attribute()?
                    .is_some_and(|k| php::truthy(&Value::String(k)))
            {
                children = self.get_prototype_children(&prototype)?;
            }

            // add children
            for (_, child_node) in &children {
                key_node.add_child(child_node)?;
            }
            key_node
        } else {
            ScalarNode::new(&key_name, Some(node))?
        };

        let mut info = b"Prototype".to_vec();
        if let Some(prototype_info) = prototype.get_info() {
            info.extend_from_slice(b": ");
            info.extend_from_slice(&prototype_info);
        }
        key_node.set_info(&info);

        Ok(vec![(Key::from_bytes(&key_name), key_node)])
    }
}

/// `sprintf('%'.$width.'s', $s)`: a width of 0 reads as the `0` padding
/// flag with no width, which leaves `$s` as it is.
fn sprintf_s_width(width: usize, s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(width.max(s.len()));
    out.extend(std::iter::repeat_n(b' ', width.saturating_sub(s.len())));
    out.extend_from_slice(s);
    out
}

/// `str_replace($search, $replace, $subject)` for one pair.
fn replace(subject: &[u8], search: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(search) {
            out.extend_from_slice(with);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

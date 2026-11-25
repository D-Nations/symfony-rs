//! Port of Symfony\Component\ExpressionLanguage\Node\ArrayNode.php.
//!
//! [`ArrayNode`] also stands for `ArgumentsNode` (its subclass), which
//! compiles and dumps without keys and brackets
//! (`arguments_node.rs`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/ArrayNode.php

use php_value::{PhpArray, Value};

use super::{ConstantNode, Node, is_hash};
use crate::{compiler::Compiler, expression_function::Functions, php, syntax_error::Error};

/// An array literal (`[a, b]`, `{k: v}`), or a list of arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayNode {
    /// Keys and values, alternating.
    nodes: Vec<Node>,
    index: i64,
    arguments: bool,
}

impl Default for ArrayNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ArrayNode {
    /// `new ArrayNode()`.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            index: -1,
            arguments: false,
        }
    }

    /// `new ArgumentsNode()`.
    pub fn arguments() -> Self {
        Self {
            arguments: true,
            ..Self::new()
        }
    }

    /// Whether this is an `ArgumentsNode`.
    pub fn is_arguments(&self) -> bool {
        self.arguments
    }

    /// `addElement($value, $key)`: without a key, the next int.
    pub fn add_element(&mut self, value: impl Into<Node>, key: Option<Node>) {
        let key = key.unwrap_or_else(|| {
            self.index += 1;
            Node::Constant(ConstantNode::new(Value::Int(self.index)))
        });
        self.nodes.push(key);
        self.nodes.push(value.into());
    }

    /// `$node->nodes`: keys and values, alternating.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// `getKeyValuePairs()`.
    pub fn pairs(&self) -> impl Iterator<Item = (&Node, &Node)> {
        self.nodes.chunks(2).filter_map(|pair| match pair {
            [key, value] => Some((key, value)),
            _ => None,
        })
    }

    /// The values (`$node->nodes` of the arguments `Parser::parseArguments()`
    /// collects).
    pub fn values(&self) -> impl Iterator<Item = &Node> {
        self.pairs().map(|(_, value)| value)
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        if self.arguments {
            return self.compile_arguments(compiler, false);
        }
        compiler.raw("[");
        self.compile_arguments(compiler, true)?;
        compiler.raw("]");
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let mut result = PhpArray::new();
        for (key, value) in self.pairs() {
            let key = php::write_key(&key.evaluate(functions, values)?)?;
            result.insert(key, value.evaluate(functions, values)?);
        }
        Ok(Value::Array(result))
    }

    pub(crate) fn dump_into(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        if self.arguments {
            return self.dump_arguments(out);
        }
        // $value[$key->attributes['value']] = $value node
        let mut value = PhpArray::new();
        let pairs: Vec<(&Node, &Node)> = self.pairs().collect();
        for (i, (key, _)) in pairs.iter().enumerate() {
            let key = php::write_key(&key.constant_value())?;
            value.insert(key, Value::Int(i64::try_from(i).unwrap_or(i64::MAX)));
        }
        let node_at = |v: &Value| match v {
            Value::Int(i) => usize::try_from(*i)
                .ok()
                .and_then(|i| pairs.get(i))
                .map(|p| p.1),
            _ => None,
        };
        let hash = is_hash(&value);
        out.push(if hash { b'{' } else { b'[' });
        for (n, (k, v)) in value.iter().enumerate() {
            if n > 0 {
                out.extend_from_slice(b", ");
            }
            if hash {
                ConstantNode::new(k.to_value()).dump_into(out)?;
                out.extend_from_slice(b": ");
            }
            if let Some(node) = node_at(v) {
                node.dump_into(out)?;
            }
        }
        out.push(if hash { b'}' } else { b']' });
        Ok(())
    }

    /// `compileArguments($compiler, $withKeys)`.
    pub(crate) fn compile_arguments(
        &self,
        compiler: &mut Compiler,
        with_keys: bool,
    ) -> Result<(), Error> {
        let mut first = true;
        for (key, value) in self.pairs() {
            if !first {
                compiler.raw(", ");
            }
            first = false;
            if with_keys {
                compiler.compile(key)?.raw(" => ");
            }
            compiler.compile(value)?;
        }
        Ok(())
    }
}

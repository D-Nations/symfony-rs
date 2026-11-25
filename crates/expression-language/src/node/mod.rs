//! Port of Symfony\Component\ExpressionLanguage\Node\Node.php: the AST.
//!
//! Upstream's nodes are classes that share `$nodes` and `$attributes`
//! arrays. Here [`Node`] is a closed enum of the node classes, each
//! variant a struct with its children and attributes as fields;
//! [`Node::Node`] is the plain `Node` class (a list of nodes, as
//! `Parser::parseArguments()` returns it). [`Node::attributes`] and
//! [`Node::children`] give upstream's two arrays back, in upstream's
//! order, for `__toString()` and for the depth check.
//!
//! `GetAttrNode` changes two of its attributes while it evaluates
//! (`is_null_coalesce`, `is_short_circuited`); they are atomic flags, so
//! evaluation takes `&self` and a parsed expression can be shared.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/Node.php

pub mod arguments_node;
pub mod array_node;
pub mod binary_node;
pub mod conditional_node;
pub mod constant_node;
pub mod function_node;
pub mod get_attr_node;
pub mod name_node;
pub mod null_coalesce_node;
pub mod null_coalesced_name_node;
pub mod unary_node;

use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use php_value::{PhpArray, PhpString, Value};

pub use self::{
    array_node::ArrayNode, binary_node::BinaryNode, conditional_node::ConditionalNode,
    constant_node::ConstantNode, function_node::FunctionNode, get_attr_node::GetAttrNode,
    get_attr_node::GetAttrType, name_node::NameNode, null_coalesce_node::NullCoalesceNode,
    null_coalesced_name_node::NullCoalescedNameNode, unary_node::UnaryNode,
};
use crate::{
    compiler::Compiler,
    expression_function::Functions,
    php,
    syntax_error::{Class, Error},
};

/// A node of the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// The `Node` class itself: a list of nodes.
    Node(Vec<Node>),
    /// `ArrayNode` and `ArgumentsNode`.
    Array(ArrayNode),
    Constant(ConstantNode),
    Name(NameNode),
    NullCoalescedName(NullCoalescedNameNode),
    Unary(UnaryNode),
    Binary(BinaryNode),
    Conditional(ConditionalNode),
    NullCoalesce(NullCoalesceNode),
    Function(FunctionNode),
    GetAttr(GetAttrNode),
}

/// An attribute that `evaluate()` changes in place.
#[derive(Debug, Default)]
pub struct Flag(AtomicBool);

impl Flag {
    pub fn new(value: bool) -> Self {
        Self(AtomicBool::new(value))
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set(&self, value: bool) {
        self.0.store(value, Ordering::Relaxed);
    }
}

impl Clone for Flag {
    fn clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl PartialEq for Flag {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

/// The class names PHP prints, without the namespace.
pub const NAMESPACE: &str = "Symfony\\Component\\ExpressionLanguage\\Node\\";

impl Node {
    /// `new Node($nodes)`.
    pub fn list(nodes: Vec<Node>) -> Self {
        Self::Node(nodes)
    }

    /// The class name without the namespace (`static::class` with the
    /// namespace removed).
    pub fn short_class(&self) -> &'static str {
        match self {
            Self::Node(_) => "Node",
            Self::Array(a) => {
                if a.is_arguments() {
                    "ArgumentsNode"
                } else {
                    "ArrayNode"
                }
            }
            Self::Constant(_) => "ConstantNode",
            Self::Name(_) => "NameNode",
            Self::NullCoalescedName(_) => "NullCoalescedNameNode",
            Self::Unary(_) => "UnaryNode",
            Self::Binary(_) => "BinaryNode",
            Self::Conditional(_) => "ConditionalNode",
            Self::NullCoalesce(_) => "NullCoalesceNode",
            Self::Function(_) => "FunctionNode",
            Self::GetAttr(_) => "GetAttrNode",
        }
    }

    /// `static::class`.
    pub fn class(&self) -> String {
        format!("{NAMESPACE}{}", self.short_class())
    }

    /// `$node->nodes`, in upstream's order.
    pub fn children(&self) -> Vec<&Node> {
        match self {
            Self::Node(nodes) => nodes.iter().collect(),
            Self::Array(a) => a.nodes().iter().collect(),
            Self::Constant(_) | Self::Name(_) | Self::NullCoalescedName(_) => Vec::new(),
            Self::Unary(u) => vec![&u.node],
            Self::Binary(b) => vec![&b.left, &b.right],
            Self::Conditional(c) => vec![&c.expr1, &c.expr2, &c.expr3],
            Self::NullCoalesce(n) => vec![&n.expr1, &n.expr2],
            Self::Function(f) => vec![&f.arguments],
            Self::GetAttr(g) => vec![&g.node, &g.attribute, &g.arguments],
        }
    }

    /// `$node->attributes`, in upstream's order.
    pub fn attributes(&self) -> Vec<(&'static str, Value)> {
        match self {
            Self::Node(_) | Self::Array(_) | Self::Conditional(_) | Self::NullCoalesce(_) => {
                Vec::new()
            }
            Self::Constant(c) => vec![("value", c.value.clone())],
            Self::Name(n) => vec![("name", Value::String(n.name.clone()))],
            Self::NullCoalescedName(n) => vec![("name", Value::String(n.name.clone()))],
            Self::Unary(u) => vec![("operator", Value::from(u.operator.as_str()))],
            Self::Binary(b) => vec![("operator", Value::from(b.operator.as_str()))],
            Self::Function(f) => vec![("name", Value::String(f.name.clone()))],
            Self::GetAttr(g) => vec![
                ("type", Value::Int(g.kind.value())),
                ("is_null_coalesce", Value::Bool(g.is_null_coalesce.get())),
                (
                    "is_short_circuited",
                    Value::Bool(g.is_short_circuited.get()),
                ),
                ("is_null_safe", Value::Bool(g.is_null_safe)),
            ],
        }
    }

    /// `__toString()`: the class, its attributes (`var_export()`ed, new
    /// lines removed) and its children, indented.
    pub fn to_php_string(&self) -> PhpString {
        let mut attributes = Vec::new();
        for (name, value) in self.attributes() {
            let exported: Vec<u8> = php::var_export(&value)
                .into_iter()
                .filter(|&b| b != b'\n')
                .collect();
            attributes.push([name.as_bytes(), b": ", &exported].concat());
        }
        let mut first = self.short_class().as_bytes().to_vec();
        first.push(b'(');
        first.extend_from_slice(&attributes.join(b", ".as_slice()));
        let children = self.children();
        let mut repr = vec![first];
        if children.is_empty() {
            repr[0].push(b')');
        } else {
            for node in children {
                let text = node.to_php_string();
                for line in text.as_bytes().split(|&b| b == b'\n') {
                    repr.push([b"    ".as_slice(), line].concat());
                }
            }
            repr.push(b")".to_vec());
        }
        PhpString::from(repr.join(b"\n".as_slice()))
    }

    /// `compile($compiler)`.
    pub fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        match self {
            Self::Node(nodes) => {
                for node in nodes {
                    node.compile(compiler)?;
                }
                Ok(())
            }
            Self::Array(a) => a.compile(compiler),
            Self::Constant(c) => c.compile(compiler),
            Self::Name(n) => {
                n.compile(compiler);
                Ok(())
            }
            Self::NullCoalescedName(n) => {
                n.compile(compiler);
                Ok(())
            }
            Self::Unary(u) => u.compile(compiler),
            Self::Binary(b) => b.compile(compiler),
            Self::Conditional(c) => c.compile(compiler),
            Self::NullCoalesce(n) => n.compile(compiler),
            Self::Function(f) => f.compile(compiler),
            Self::GetAttr(g) => g.compile(compiler),
        }
    }

    /// `evaluate($functions, $values)`.
    pub fn evaluate(&self, functions: &Functions, values: &PhpArray) -> Result<Value, Error> {
        match self {
            Self::Node(nodes) => {
                let mut results = Vec::with_capacity(nodes.len());
                for node in nodes {
                    results.push(node.evaluate(functions, values)?);
                }
                Ok(Value::Array(PhpArray::list(results)))
            }
            Self::Array(a) => a.evaluate(functions, values),
            Self::Constant(c) => Ok(c.value.clone()),
            Self::Name(n) => Ok(n.evaluate(values)),
            Self::NullCoalescedName(_) => Ok(Value::Null),
            Self::Unary(u) => u.evaluate(functions, values),
            Self::Binary(b) => b.evaluate(functions, values),
            Self::Conditional(c) => c.evaluate(functions, values),
            Self::NullCoalesce(n) => n.evaluate(functions, values),
            Self::Function(f) => f.evaluate(functions, values),
            Self::GetAttr(g) => g.evaluate(functions, values),
        }
    }

    /// `dump()`: the node as expression text.
    pub fn dump(&self) -> Result<PhpString, Error> {
        let mut out = Vec::new();
        self.dump_into(&mut out)?;
        Ok(PhpString::from(out))
    }

    pub(crate) fn dump_into(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Node(_) => Err(Error::new(
                Class::BadMethodCallException,
                format!(
                    "Dumping a \"{}\" instance is not supported yet.",
                    self.class()
                ),
            )),
            Self::Array(a) => a.dump_into(out),
            Self::Constant(c) => c.dump_into(out),
            Self::Name(n) => {
                out.extend_from_slice(n.name.as_bytes());
                Ok(())
            }
            Self::NullCoalescedName(n) => {
                out.extend_from_slice(n.name.as_bytes());
                out.extend_from_slice(b" ?? null");
                Ok(())
            }
            Self::Unary(u) => {
                out.push(b'(');
                out.extend_from_slice(u.operator.as_bytes());
                out.push(b' ');
                u.node.dump_into(out)?;
                out.push(b')');
                Ok(())
            }
            Self::Binary(b) => {
                out.push(b'(');
                b.left.dump_into(out)?;
                out.push(b' ');
                out.extend_from_slice(b.operator.as_bytes());
                out.push(b' ');
                b.right.dump_into(out)?;
                out.push(b')');
                Ok(())
            }
            Self::Conditional(c) => {
                out.push(b'(');
                c.expr1.dump_into(out)?;
                out.extend_from_slice(b" ? ");
                c.expr2.dump_into(out)?;
                out.extend_from_slice(b" : ");
                c.expr3.dump_into(out)?;
                out.push(b')');
                Ok(())
            }
            Self::NullCoalesce(n) => {
                out.push(b'(');
                n.expr1.dump_into(out)?;
                out.extend_from_slice(b") ?? (");
                n.expr2.dump_into(out)?;
                out.push(b')');
                Ok(())
            }
            Self::Function(f) => f.dump_into(out),
            Self::GetAttr(g) => g.dump_into(out),
        }
    }

    /// The `ConstantNode`, if this is one.
    pub fn as_constant(&self) -> Option<&ConstantNode> {
        match self {
            Self::Constant(c) => Some(c),
            _ => None,
        }
    }

    /// `$node->attributes['value']` of a `ConstantNode`; `null` for any
    /// other node (PHP warns of an undefined index).
    pub(crate) fn constant_value(&self) -> Value {
        self.as_constant().map_or(Value::Null, |c| c.value.clone())
    }
}

/// `addcslashes($value, "\0\t\"\\")` in double quotes (`dumpString()`).
pub(crate) fn dump_string(value: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    out.extend_from_slice(&php::addcslashes(value, b"\0\t\"\\"));
    out.push(b'"');
}

/// `isHash($value)`: keys other than 0, 1, 2, ... in order.
pub(crate) fn is_hash(value: &PhpArray) -> bool {
    value
        .keys()
        .zip(0i64..)
        .any(|(key, expected)| key.as_int() != Some(expected))
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_php_string().to_string_lossy())
    }
}

impl From<ConstantNode> for Node {
    fn from(n: ConstantNode) -> Self {
        Self::Constant(n)
    }
}

impl From<ArrayNode> for Node {
    fn from(n: ArrayNode) -> Self {
        Self::Array(n)
    }
}

impl From<NameNode> for Node {
    fn from(n: NameNode) -> Self {
        Self::Name(n)
    }
}

impl From<NullCoalescedNameNode> for Node {
    fn from(n: NullCoalescedNameNode) -> Self {
        Self::NullCoalescedName(n)
    }
}

impl From<UnaryNode> for Node {
    fn from(n: UnaryNode) -> Self {
        Self::Unary(n)
    }
}

impl From<BinaryNode> for Node {
    fn from(n: BinaryNode) -> Self {
        Self::Binary(n)
    }
}

impl From<ConditionalNode> for Node {
    fn from(n: ConditionalNode) -> Self {
        Self::Conditional(n)
    }
}

impl From<NullCoalesceNode> for Node {
    fn from(n: NullCoalesceNode) -> Self {
        Self::NullCoalesce(n)
    }
}

impl From<FunctionNode> for Node {
    fn from(n: FunctionNode) -> Self {
        Self::Function(n)
    }
}

impl From<GetAttrNode> for Node {
    fn from(n: GetAttrNode) -> Self {
        Self::GetAttr(n)
    }
}

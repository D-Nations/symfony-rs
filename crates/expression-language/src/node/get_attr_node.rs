//! Port of Symfony\Component\ExpressionLanguage\Node\GetAttrNode.php.
//!
//! Objects are php-value objects: a property is read from
//! `PhpObject::properties()`; a method is called when the object is a
//! [`crate::object::MethodObject`] that has it (or `__invoke` of a
//! `Closure`); `ArrayAccess` is php-value's `ArrayObject`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/GetAttrNode.php

use php_value::{Key, PhpArray, Value, object::ArrayObject, object::Closure};

use super::{ArrayNode, Flag, Node};
use crate::{
    compiler::Compiler,
    expression_function::Functions,
    object::MethodObject,
    php,
    syntax_error::{Class, Error},
};

/// How a [`GetAttrNode`] reaches its attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetAttrType {
    /// `PROPERTY_CALL` (1): `a.b`.
    Property,
    /// `METHOD_CALL` (2): `a.b()`.
    Method,
    /// `ARRAY_CALL` (3): `a[b]`.
    Array,
}

impl GetAttrType {
    /// The constant's value.
    pub fn value(self) -> i64 {
        match self {
            Self::Property => 1,
            Self::Method => 2,
            Self::Array => 3,
        }
    }
}

/// `a.b`, `a.b()`, `a[b]`, and their null-safe forms.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAttrNode {
    pub node: Box<Node>,
    pub attribute: Box<Node>,
    /// An `ArrayNode` (or `ArgumentsNode`).
    pub arguments: Box<Node>,
    pub kind: GetAttrType,
    pub is_null_coalesce: Flag,
    pub is_short_circuited: Flag,
    /// `?.[]`; only kept for [`GetAttrType::Array`]. Property and method
    /// calls are null-safe through their `ConstantNode`.
    pub is_null_safe: bool,
}

/// The class `convertToArrayAccess()` belongs to, as compiled code
/// names it.
pub const CLASS: &str = "Symfony\\Component\\ExpressionLanguage\\Node\\GetAttrNode";

impl GetAttrNode {
    /// `new GetAttrNode($node, $attribute, $arguments, $type)`.
    pub fn new(
        node: impl Into<Node>,
        attribute: impl Into<Node>,
        arguments: ArrayNode,
        kind: GetAttrType,
    ) -> Self {
        Self::with_null_safe(node, attribute, arguments, kind, false)
    }

    /// `new GetAttrNode($node, $attribute, $arguments, $type,
    /// $isNullSafe)`.
    pub fn with_null_safe(
        node: impl Into<Node>,
        attribute: impl Into<Node>,
        arguments: ArrayNode,
        kind: GetAttrType,
        is_null_safe: bool,
    ) -> Self {
        Self {
            node: Box::new(node.into()),
            attribute: Box::new(attribute.into()),
            arguments: Box::new(Node::Array(arguments)),
            kind,
            is_null_coalesce: Flag::new(false),
            is_short_circuited: Flag::new(false),
            is_null_safe: kind == GetAttrType::Array && is_null_safe,
        }
    }

    /// `$this->nodes['attribute']->isNullSafe`.
    fn attribute_is_null_safe(&self) -> bool {
        self.attribute.as_constant().is_some_and(|c| c.is_null_safe)
    }

    /// `$this->nodes['attribute']->attributes['value']` as a name.
    fn attribute_name(&self) -> Result<Vec<u8>, Error> {
        match self.attribute.constant_value() {
            Value::Null => Err(Error::type_error(
                "Symfony\\Component\\ExpressionLanguage\\Compiler::raw(): Argument #1 ($string) must be of type string, null given",
            )),
            value => Ok(php::to_string(&value)?.as_bytes().to_vec()),
        }
    }

    pub(crate) fn compile(&self, compiler: &mut Compiler) -> Result<(), Error> {
        let null_safe = self.attribute_is_null_safe() || self.is_null_safe;
        let arrow = if null_safe { "?->" } else { "->" };
        match self.kind {
            GetAttrType::Property => {
                let name = self.attribute_name()?;
                compiler.compile(&self.node)?.raw(arrow).raw_bytes(&name);
            }
            GetAttrType::Method => {
                let name = self.attribute_name()?;
                compiler
                    .compile(&self.node)?
                    .raw(arrow)
                    .raw_bytes(&name)
                    .raw("(")
                    .compile(&self.arguments)?
                    .raw(")");
            }
            GetAttrType::Array => {
                if null_safe {
                    let dump = self.node.dump()?;
                    compiler
                        .raw("\\")
                        .raw(CLASS)
                        .raw("::convertToArrayAccess(")
                        .compile(&self.node)?
                        .raw(", ")
                        .string(dump.as_bytes())
                        .raw(")?->offsetGet(")
                        .compile(&self.attribute)?
                        .raw(")");
                } else {
                    compiler
                        .compile(&self.node)?
                        .raw("[")
                        .compile(&self.attribute)?
                        .raw("]");
                }
            }
        }
        Ok(())
    }

    fn is_short_circuited(&self) -> bool {
        self.is_short_circuited.get()
            || matches!(&*self.node, Node::GetAttr(node) if node.is_short_circuited())
    }

    fn property_error(&self, what: &str, kind: &str) -> Error {
        let attribute = self.attribute.dump();
        let node = self.node.dump();
        match (attribute, node) {
            (Ok(a), Ok(n)) => {
                let mut text = format!("Unable to {what} \"").into_bytes();
                text.extend_from_slice(a.as_bytes());
                text.extend_from_slice(format!("\" of {kind} \"").as_bytes());
                text.extend_from_slice(n.as_bytes());
                text.extend_from_slice(b"\".");
                Error::runtime(text)
            }
            (Err(e), _) | (_, Err(e)) => e,
        }
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    pub(crate) fn evaluate(
        &self,
        functions: &Functions,
        values: &PhpArray,
    ) -> Result<Value, Error> {
        let null_safe = self.is_null_safe;
        self.is_short_circuited.set(false);
        let coalesce = self.is_null_coalesce.get();

        match self.kind {
            GetAttrType::Property => {
                let obj = self.node.evaluate(functions, values)?;
                if obj.is_null() && (self.attribute_is_null_safe() || coalesce) {
                    self.is_short_circuited.set(true);
                    return Ok(Value::Null);
                }
                if obj.is_null() && self.is_short_circuited() {
                    return Ok(Value::Null);
                }
                if !matches!(obj, Value::Object(_)) && !coalesce {
                    return Err(self.property_error("get property", "non-object"));
                }
                let property = php::to_string(&self.attribute.constant_value())?;
                Ok(match &obj {
                    Value::Object(o) => o
                        .properties()
                        .get(&Key::from_bytes(property.as_bytes()))
                        .cloned()
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                })
            }
            GetAttrType::Method => {
                let obj = self.node.evaluate(functions, values)?;
                if obj.is_null() && self.attribute_is_null_safe() {
                    self.is_short_circuited.set(true);
                    return Ok(Value::Null);
                }
                if obj.is_null() && self.is_short_circuited() {
                    return Ok(Value::Null);
                }
                let Value::Object(o) = &obj else {
                    return Err(self.property_error("call method", "non-object"));
                };
                let name = php::to_string(&self.attribute.constant_value())?;
                let name = name.as_bytes();
                let call = |arguments: &[Value]| -> Option<Result<Value, Error>> {
                    if let Some(object) = o.downcast_ref::<MethodObject>() {
                        return object.call(name, arguments);
                    }
                    if let Some(closure) = o.downcast_ref::<Closure>()
                        && name.eq_ignore_ascii_case(b"__invoke")
                    {
                        return Some(Ok(closure.call(arguments)));
                    }
                    None
                };
                let callable = o
                    .downcast_ref::<MethodObject>()
                    .is_some_and(|m| m.has_method(name))
                    || (o.downcast_ref::<Closure>().is_some()
                        && name.eq_ignore_ascii_case(b"__invoke"));
                if !callable {
                    let mut text = b"Unable to call method \"".to_vec();
                    text.extend_from_slice(name);
                    text.extend_from_slice(b"\" of object \"");
                    text.extend_from_slice(obj.debug_type().as_bytes());
                    text.extend_from_slice(b"\".");
                    return Err(Error::runtime(text));
                }
                let arguments = match self.arguments.evaluate(functions, values)? {
                    Value::Array(a) => a.values().cloned().collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                call(&arguments).unwrap_or(Ok(Value::Null))
            }
            GetAttrType::Array => {
                let array = self.node.evaluate(functions, values)?;
                if array.is_null() && (null_safe || self.is_short_circuited()) {
                    self.is_short_circuited
                        .set(null_safe || self.is_short_circuited.get());
                    return Ok(Value::Null);
                }
                let array_object = match &array {
                    Value::Object(o) => o.downcast_ref::<ArrayObject>(),
                    _ => None,
                };
                let is_array = matches!(array, Value::Array(_));
                if !is_array
                    && array_object.is_none()
                    && (!coalesce || matches!(array, Value::Object(_)))
                {
                    let mut text = b"Unable to get an item of non-array \"".to_vec();
                    text.extend_from_slice(self.node.dump()?.as_bytes());
                    text.extend_from_slice(b"\".");
                    return Err(Error::runtime(text));
                }
                let key = self.attribute.evaluate(functions, values)?;
                if coalesce {
                    return Ok(match &array {
                        Value::Array(a) => {
                            let key = php::isset_key(&key)?;
                            a.get(&key).cloned().unwrap_or(Value::Null)
                        }
                        Value::String(s) => php::string_offset_or_null(s.as_bytes(), &key),
                        Value::Object(_) => match array_object {
                            Some(ao) => {
                                let key = php::isset_key(&key)?;
                                ao.storage().get(&key).cloned().unwrap_or(Value::Null)
                            }
                            None => Value::Null,
                        },
                        _ => Value::Null,
                    });
                }
                match (&array, array_object) {
                    (Value::Array(a), _) => {
                        let key = php::read_key(&key, "array")?;
                        Ok(a.get(&key).cloned().unwrap_or(Value::Null))
                    }
                    (_, Some(ao)) => {
                        let key = php::read_key(&key, "ArrayObject")?;
                        Ok(ao.storage().get(&key).cloned().unwrap_or(Value::Null))
                    }
                    _ => Ok(Value::Null),
                }
            }
        }
    }

    pub(crate) fn dump_into(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        let null_safe = self.attribute_is_null_safe();
        self.node.dump_into(out)?;
        match self.kind {
            GetAttrType::Property => {
                out.extend_from_slice(if null_safe { b"?." } else { b"." });
                self.attribute.dump_into(out)?;
            }
            GetAttrType::Method => {
                out.extend_from_slice(if null_safe { b"?." } else { b"." });
                self.attribute.dump_into(out)?;
                out.push(b'(');
                self.arguments.dump_into(out)?;
                out.push(b')');
            }
            GetAttrType::Array => {
                out.extend_from_slice(if self.is_null_safe { b"?.[" } else { b"[" });
                self.attribute.dump_into(out)?;
                out.push(b']');
            }
        }
        Ok(())
    }
}

/// `GetAttrNode::convertToArrayAccess($value, $nodeDump)`, which compiled
/// null-safe array accesses call: `null` stays `null`, an array becomes
/// an `ArrayObject`, an `ArrayObject` is kept; anything else throws.
pub fn convert_to_array_access(value: &Value, node_dump: &[u8]) -> Result<Value, Error> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Array(a) => Ok(Value::Object(std::sync::Arc::new(ArrayObject::new(
            a.clone(),
        )))),
        Value::Object(o) if o.downcast_ref::<ArrayObject>().is_some() => Ok(value.clone()),
        _ => {
            let mut text = b"Unable to get an item of non-array \"".to_vec();
            text.extend_from_slice(node_dump);
            text.extend_from_slice(b"\".");
            Err(Error::new(Class::RuntimeException, text))
        }
    }
}

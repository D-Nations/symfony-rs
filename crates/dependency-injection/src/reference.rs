//! Port of Symfony\Component\DependencyInjection\Reference.php and
//! TypedReference.php, and the two small value classes Parameter.php and
//! Variable.php.
//!
//! All four are immutable PHP objects. A `TypedReference` is a
//! [`Reference`] with a type ([`Reference::typed`]); its class name is
//! `TypedReference`, and it is still `instanceof Reference`.
//!
//! Upstream: symfony/dependency-injection Reference.php TypedReference.php Parameter.php Variable.php

use std::fmt;

use php_value::{PhpArray, PhpString, Value, value::PhpObject};

use crate::container_interface::InvalidBehavior;
use crate::object::{downcast, object_value};

const REFERENCE: &str = "Symfony\\Component\\DependencyInjection\\Reference";
const TYPED_REFERENCE: &str = "Symfony\\Component\\DependencyInjection\\TypedReference";

#[derive(Debug, Clone, PartialEq)]
struct Typed {
    type_: String,
    name: Option<String>,
    attributes: PhpArray,
}

/// `Reference` (and `TypedReference`): a reference to a service.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    id: String,
    invalid_behavior: InvalidBehavior,
    typed: Option<Box<Typed>>,
}

impl Reference {
    pub const CLASS: &'static str = REFERENCE;
    pub const TYPED_CLASS: &'static str = TYPED_REFERENCE;

    /// `new Reference($id)`.
    pub fn new(id: impl Into<String>) -> Self {
        Self::with_behavior(id, InvalidBehavior::Exception)
    }

    /// `new Reference($id, $invalidBehavior)`.
    pub fn with_behavior(id: impl Into<String>, invalid_behavior: InvalidBehavior) -> Self {
        Self {
            id: id.into(),
            invalid_behavior,
            typed: None,
        }
    }

    /// `new TypedReference($id, $type, $invalidBehavior, $name,
    /// $attributes)`. The name is kept only when the id is the type.
    pub fn typed(
        id: impl Into<String>,
        type_: impl Into<String>,
        invalid_behavior: InvalidBehavior,
        name: Option<String>,
        attributes: PhpArray,
    ) -> Self {
        let id = id.into();
        let type_ = type_.into();
        let name = if type_ == id { name } else { None };
        Self {
            id,
            invalid_behavior,
            typed: Some(Box::new(Typed {
                type_,
                name,
                attributes,
            })),
        }
    }

    /// `(string) $reference`: the service id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// `getInvalidBehavior()`.
    pub fn invalid_behavior(&self) -> InvalidBehavior {
        self.invalid_behavior
    }

    /// `$reference instanceof TypedReference`.
    pub fn is_typed(&self) -> bool {
        self.typed.is_some()
    }

    /// `TypedReference::getType()`.
    pub fn type_(&self) -> Option<&str> {
        self.typed.as_ref().map(|t| t.type_.as_str())
    }

    /// `TypedReference::getName()`.
    pub fn name(&self) -> Option<&str> {
        self.typed.as_ref().and_then(|t| t.name.as_deref())
    }

    /// `TypedReference::getAttributes()`.
    pub fn attributes(&self) -> Option<&PhpArray> {
        self.typed.as_ref().map(|t| &t.attributes)
    }

    /// The reference, when `value` is a `Reference` object.
    pub fn from_value(value: &Value) -> Option<&Self> {
        downcast::<Self>(value)
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

impl PhpObject for Reference {
    fn class(&self) -> &str {
        if self.typed.is_some() {
            TYPED_REFERENCE
        } else {
            REFERENCE
        }
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        if let Some(typed) = &self.typed {
            properties.insert(
                "name",
                typed.name.as_deref().map_or(Value::Null, Value::from),
            );
            properties.insert("type", Value::from(typed.type_.as_str()));
            properties.insert("attributes", Value::Array(typed.attributes.clone()));
        }
        properties.insert("id", Value::from(self.id.as_str()));
        properties.insert(
            "invalidBehavior",
            Value::Int(self.invalid_behavior.as_int()),
        );
        properties
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(PhpString::from(self.id.as_str()))
    }
}

impl From<Reference> for Value {
    fn from(reference: Reference) -> Self {
        object_value(reference)
    }
}

/// `Parameter`: a reference to a parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    id: String,
}

impl Parameter {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Parameter";

    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    /// `(string) $parameter`.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn from_value(value: &Value) -> Option<&Self> {
        downcast::<Self>(value)
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)
    }
}

impl PhpObject for Parameter {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("id", Value::from(self.id.as_str()));
        properties
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(PhpString::from(self.id.as_str()))
    }
}

impl From<Parameter> for Value {
    fn from(parameter: Parameter) -> Self {
        object_value(parameter)
    }
}

/// `Variable`: a PHP variable name in generated code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    name: String,
}

impl Variable {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Variable";

    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// `(string) $variable`.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn from_value(value: &Value) -> Option<&Self> {
        downcast::<Self>(value)
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

impl PhpObject for Variable {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("name", Value::from(self.name.as_str()));
        properties
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(PhpString::from(self.name.as_str()))
    }
}

impl From<Variable> for Value {
    fn from(variable: Variable) -> Self {
        object_value(variable)
    }
}

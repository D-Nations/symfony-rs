//! Port of Symfony\Component\HttpKernel\ControllerMetadata\ArgumentMetadata.php,
//! with ArgumentMetadataFactory.php and ArgumentMetadataFactoryInterface.php:
//! what the argument resolver knows of one controller argument.
//!
//! PHP's factory reflects the controller; here the controller declares
//! what reflection would report ([`crate::controller::Parameter`]: name,
//! type, variadic, default, nullability, attributes) and its name, and the
//! factory turns that into [`ArgumentMetadata`]. `self` and `parent` types
//! are resolved by the controller's author, as there is no declaring class
//! to read.
//!
//! Attributes are PHP objects. `getAttributesOfType()` compares class
//! names exactly, and with `IS_INSTANCEOF` also matches subclasses of
//! `ValueResolver` (an [`crate::argument_resolver::ValueResolver`] of any
//! class), the one attribute hierarchy the resolver reads.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel ControllerMetadata/ArgumentMetadata.php ControllerMetadata/ArgumentMetadataFactory.php ControllerMetadata/ArgumentMetadataFactoryInterface.php

use php_value::Value;

use crate::{
    argument_resolver::{VALUE_RESOLVER_CLASS, ValueResolver},
    controller::{Controller, Parameter},
    exception::{PhpClass, PhpException, Throwable},
};

/// `ArgumentMetadata`'s class name.
pub const CLASS: &str = "Symfony\\Component\\HttpKernel\\ControllerMetadata\\ArgumentMetadata";

/// Responsible for storing metadata of an argument.
#[derive(Debug, Clone)]
pub struct ArgumentMetadata {
    name: String,
    type_name: Option<String>,
    is_variadic: bool,
    has_default_value: bool,
    default_value: Value,
    is_nullable: bool,
    attributes: Vec<Value>,
    controller_name: String,
}

impl ArgumentMetadata {
    /// `ArgumentMetadata::IS_INSTANCEOF`.
    pub const IS_INSTANCEOF: i64 = 2;

    /// `new ArgumentMetadata($name, $type, $isVariadic, $hasDefaultValue,
    /// $defaultValue, $isNullable = false, $attributes = [],
    /// $controllerName = 'n/a')`: nullable also without a type or with a
    /// `null` default.
    #[expect(clippy::too_many_arguments, reason = "upstream's constructor")]
    pub fn new(
        name: impl Into<String>,
        type_name: Option<&str>,
        is_variadic: bool,
        has_default_value: bool,
        default_value: Value,
        is_nullable: bool,
        attributes: Vec<Value>,
        controller_name: impl Into<String>,
    ) -> Self {
        let is_nullable = is_nullable
            || type_name.is_none()
            || (has_default_value && matches!(default_value, Value::Null));
        Self {
            name: name.into(),
            type_name: type_name.map(str::to_owned),
            is_variadic,
            has_default_value,
            default_value,
            is_nullable,
            attributes,
            controller_name: controller_name.into(),
        }
    }

    /// The five required arguments, with upstream's defaults for the rest.
    pub fn simple(
        name: impl Into<String>,
        type_name: Option<&str>,
        is_variadic: bool,
        has_default_value: bool,
        default_value: Value,
    ) -> Self {
        Self::new(
            name,
            type_name,
            is_variadic,
            has_default_value,
            default_value,
            false,
            Vec::new(),
            "n/a",
        )
    }

    /// `getName()`: the name as given in the code.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// `getType()`: the type as given in the code (`array`, `string`, a
    /// class name); `None` without one.
    pub fn get_type(&self) -> Option<&str> {
        self.type_name.as_deref()
    }

    /// `isVariadic()`.
    pub fn is_variadic(&self) -> bool {
        self.is_variadic
    }

    /// `hasDefaultValue()`.
    pub fn has_default_value(&self) -> bool {
        self.has_default_value
    }

    /// `isNullable()`: whether the argument accepts null values.
    pub fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    /// `getDefaultValue()`: `\LogicException` when there is none.
    pub fn get_default_value(&self) -> Result<Value, Throwable> {
        if !self.has_default_value {
            return Err(PhpException::new(
                PhpClass::LogicException,
                format!(
                    "Argument ${} does not have a default value. Use \"{CLASS}::hasDefaultValue()\" to avoid this exception.",
                    self.name
                ),
            )
            .throwable());
        }
        Ok(self.default_value.clone())
    }

    /// `getAttributes($name, $flags)`: all of them without a name.
    pub fn get_attributes(&self, name: Option<&str>, flags: i64) -> Vec<Value> {
        match name {
            None | Some("") => self.attributes.clone(),
            Some(name) => self.get_attributes_of_type(name, flags),
        }
    }

    /// `getAttributesOfType($name, $flags)`.
    pub fn get_attributes_of_type(&self, name: &str, flags: i64) -> Vec<Value> {
        let instance_of = flags & Self::IS_INSTANCEOF != 0;
        self.attributes
            .iter()
            .filter(|attribute| {
                let Value::Object(object) = attribute else {
                    return false;
                };
                object.class() == name
                    || (instance_of
                        && name == VALUE_RESOLVER_CLASS
                        && object.downcast_ref::<ValueResolver>().is_some())
            })
            .cloned()
            .collect()
    }

    /// `getControllerName()`.
    pub fn get_controller_name(&self) -> &str {
        &self.controller_name
    }
}

/// `ArgumentMetadataFactoryInterface`: builds method argument data.
pub trait ArgumentMetadataFactoryInterface: Send + Sync {
    /// `createArgumentMetadata($controller, $reflector)`; `reflector` is
    /// the controller's parameters, its own when `None`.
    fn create_argument_metadata(
        &self,
        controller: &Controller,
        reflector: Option<&[Parameter]>,
    ) -> Vec<ArgumentMetadata>;
}

/// `ArgumentMetadataFactory`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArgumentMetadataFactory;

impl ArgumentMetadataFactoryInterface for ArgumentMetadataFactory {
    fn create_argument_metadata(
        &self,
        controller: &Controller,
        reflector: Option<&[Parameter]>,
    ) -> Vec<ArgumentMetadata> {
        let parameters = reflector.unwrap_or_else(|| controller.parameters());
        parameters
            .iter()
            .map(|param| {
                ArgumentMetadata::new(
                    param.name.clone(),
                    param.type_name.as_deref(),
                    param.variadic,
                    param.default.is_some(),
                    param.default.clone().unwrap_or(Value::Null),
                    param.allows_null(),
                    param.attributes.clone(),
                    controller.name(),
                )
            })
            .collect()
    }
}

/// PHPUnit's `assertEquals()` on two values: objects by class and
/// properties, arrays by pairs, anything else by `===`.
fn equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            a.class() == b.class()
                && equals(&Value::Array(a.properties()), &Value::Array(b.properties()))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|w| equals(v, w)))
        }
        _ => a == b,
    }
}

impl PartialEq for ArgumentMetadata {
    /// `assertEquals()` on two metadata objects.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.type_name == other.type_name
            && self.is_variadic == other.is_variadic
            && self.has_default_value == other.has_default_value
            && equals(&self.default_value, &other.default_value)
            && self.is_nullable == other.is_nullable
            && self.attributes.len() == other.attributes.len()
            && self
                .attributes
                .iter()
                .zip(&other.attributes)
                .all(|(a, b)| equals(a, b))
            && self.controller_name == other.controller_name
    }
}

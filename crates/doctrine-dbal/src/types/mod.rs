//! Port of Doctrine\DBAL\Types\Type.php: the base of the so-called Doctrine
//! mapping types, and the static registry functions (`Type::getType()`,
//! `Type::addType()`, ...), here free functions of this module.
//!
//! A type is an `Arc<dyn Type>`: PHP's type objects are flyweights, and
//! the registry tells them apart by identity (`Arc::ptr_eq`), as PHP does
//! with `spl_object_id()`. Values cross a type as `php_value::Value`, the
//! PHP side of a conversion being any PHP value (`mixed`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/Type.php

use std::{
    fmt,
    sync::{Arc, OnceLock},
};

use php_value::{PhpArray, PhpString, Thrown, Value};

use crate::{ParameterType, exception::Exception, platforms::AbstractPlatform};

mod binary_type;
mod boolean_type;
mod conversion_exception;
mod date_interval_type;
mod date_time_type;
mod date_type;
mod enum_type;
mod exception;
mod float_type;
mod integer_type;
mod json_type;
mod names;
mod php_mapping_type;
mod simple_array_type;
mod string_type;
mod type_registry;

pub use binary_type::{BinaryType, BlobType};
pub use boolean_type::BooleanType;
pub use conversion_exception::{ConversionException, ConversionExceptionKind, Previous};
pub use date_interval_type::DateIntervalType;
pub use date_time_type::{
    DateTimeImmutableType, DateTimeType, DateTimeTzImmutableType, DateTimeTzType,
    DateTimeUtcImmutableType, DateTimeUtcType, VarDateTimeImmutableType, VarDateTimeType,
};
pub use date_type::{DateImmutableType, DateType, TimeImmutableType, TimeType};
pub use enum_type::EnumType;
pub use exception::TypesException;
pub use float_type::{DecimalType, FloatType, NumberType, SmallFloatType};
pub use integer_type::{BigIntType, IntegerType, SmallIntType};
pub use json_type::{JsonObjectType, JsonType, JsonbObjectType, JsonbType};
pub use names::Types;
pub use php_mapping_type::PhpMappingType;
pub use simple_array_type::SimpleArrayType;
pub use string_type::{AsciiStringType, GuidType, StringType, TextType};
pub use type_registry::TypeRegistry;

/// What a conversion throws.
#[derive(Debug, Clone)]
pub enum ConversionError {
    /// A `Types\ConversionException`.
    Conversion(ConversionException),
    /// A PHP error raised on the way: a `TypeError` for an argument of the
    /// wrong type (`strict_types`), an `Error` for an object without
    /// `__toString()` in a string, an `AssertionError` (assertions on, as
    /// under PHPUnit), or what a `jsonSerialize()` threw.
    Thrown(Thrown),
    /// Not PHP: a value this port does not read (a date or interval text
    /// outside what `php_value` parses, a stream). PHP gives an answer for
    /// it that is not reproduced.
    Unsupported(String),
}

impl ConversionError {
    /// The class of what PHP throws (`None` for [`Self::Unsupported`]).
    pub fn class(&self) -> Option<&str> {
        match self {
            Self::Conversion(e) => Some(e.kind().class()),
            Self::Thrown(t) => Some(&t.class),
            Self::Unsupported(_) => None,
        }
    }

    /// The conversion exception, when that is what was thrown.
    pub fn as_conversion(&self) -> Option<&ConversionException> {
        match self {
            Self::Conversion(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ConversionException> for ConversionError {
    fn from(e: ConversionException) -> Self {
        Self::Conversion(e)
    }
}

impl From<Thrown> for ConversionError {
    fn from(e: Thrown) -> Self {
        Self::Thrown(e)
    }
}

impl From<php_value::dateparse::Unsupported> for ConversionError {
    fn from(e: php_value::dateparse::Unsupported) -> Self {
        Self::Unsupported(e.to_string())
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(e) => write!(f, "{e}"),
            Self::Thrown(t) => write!(f, "{t}"),
            Self::Unsupported(text) => write!(f, "not supported by this port: {text}"),
        }
    }
}

impl std::error::Error for ConversionError {}

/// A Doctrine mapping type (`Doctrine\DBAL\Types\Type`).
pub trait Type: Send + Sync + fmt::Debug {
    /// `$type::class`, the class name messages and `getTypesMap()` show.
    fn class(&self) -> &str;

    /// Converts a value from its PHP representation to its database
    /// representation of this type.
    fn convert_to_database_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(value.clone())
    }

    /// Converts a value from its database representation to its PHP
    /// representation of this type.
    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(value.clone())
    }

    /// Gets the SQL declaration snippet for a column of this type.
    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception>;

    /// Gets the (preferred) binding type for values of this type that can
    /// be used when binding parameters to prepared statements.
    fn get_binding_type(&self) -> ParameterType {
        ParameterType::String
    }

    /// Modifies the SQL expression (identifier, parameter) to convert to a
    /// database value.
    fn convert_to_database_value_sql(
        &self,
        sql_expr: &[u8],
        _platform: &dyn AbstractPlatform,
    ) -> PhpString {
        PhpString::from(sql_expr)
    }

    /// Modifies the SQL expression (identifier, parameter) to convert to a
    /// PHP value.
    fn convert_to_php_value_sql(
        &self,
        sql_expr: &[u8],
        _platform: &dyn AbstractPlatform,
    ) -> PhpString {
        PhpString::from(sql_expr)
    }

    /// Gets an array of database types that map to this Doctrine type.
    fn get_mapped_database_types(&self, _platform: &dyn AbstractPlatform) -> Vec<PhpString> {
        Vec::new()
    }

    /// The `Php*MappingType` marker interface the class implements.
    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        None
    }
}

/// A class name given to `Type::addType()` / `Type::overrideType()`
/// (`class-string<Type>`): how `new $class()` builds it, or the
/// `ArgumentCountError` its constructor throws without arguments.
#[derive(Clone, Copy)]
pub struct TypeClass {
    pub name: &'static str,
    pub construct: fn() -> Result<Arc<dyn Type>, Thrown>,
}

impl fmt::Debug for TypeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TypeClass").field(&self.name).finish()
    }
}

/// `class-string<Type>|Type`.
#[derive(Debug, Clone)]
pub enum TypeArgument {
    Class(TypeClass),
    Instance(Arc<dyn Type>),
}

impl TypeArgument {
    fn instantiate(self, name: &str) -> Result<Arc<dyn Type>, TypesException> {
        match self {
            Self::Instance(type_) => Ok(type_),
            Self::Class(class) => (class.construct)()
                .map_err(|previous| TypesException::type_argument_count_error(name, previous)),
        }
    }
}

impl From<Arc<dyn Type>> for TypeArgument {
    fn from(type_: Arc<dyn Type>) -> Self {
        Self::Instance(type_)
    }
}

/// The map of supported doctrine mapping types.
fn builtin_types() -> Vec<(&'static str, Arc<dyn Type>)> {
    vec![
        (Types::ASCII_STRING, Arc::new(AsciiStringType)),
        (Types::BIGINT, Arc::new(BigIntType)),
        (Types::BINARY, Arc::new(BinaryType)),
        (Types::BLOB, Arc::new(BlobType)),
        (Types::BOOLEAN, Arc::new(BooleanType)),
        (Types::DATE_MUTABLE, Arc::new(DateType)),
        (Types::DATE_IMMUTABLE, Arc::new(DateImmutableType)),
        (Types::DATEINTERVAL, Arc::new(DateIntervalType)),
        (Types::DATETIME_MUTABLE, Arc::new(DateTimeType)),
        (Types::DATETIME_IMMUTABLE, Arc::new(DateTimeImmutableType)),
        (Types::DATETIME_UTC_MUTABLE, Arc::new(DateTimeUtcType)),
        (
            Types::DATETIME_UTC_IMMUTABLE,
            Arc::new(DateTimeUtcImmutableType),
        ),
        (Types::DATETIMETZ_MUTABLE, Arc::new(DateTimeTzType)),
        (
            Types::DATETIMETZ_IMMUTABLE,
            Arc::new(DateTimeTzImmutableType),
        ),
        (Types::DECIMAL, Arc::new(DecimalType)),
        (Types::NUMBER, Arc::new(NumberType)),
        (Types::ENUM, Arc::new(EnumType)),
        (Types::FLOAT, Arc::new(FloatType)),
        (Types::GUID, Arc::new(GuidType)),
        (Types::INTEGER, Arc::new(IntegerType)),
        (Types::JSON, Arc::new(JsonType)),
        (Types::JSON_OBJECT, Arc::new(JsonObjectType)),
        (Types::JSONB, Arc::new(JsonbType)),
        (Types::JSONB_OBJECT, Arc::new(JsonbObjectType)),
        (Types::SIMPLE_ARRAY, Arc::new(SimpleArrayType)),
        (Types::SMALLFLOAT, Arc::new(SmallFloatType)),
        (Types::SMALLINT, Arc::new(SmallIntType)),
        (Types::STRING, Arc::new(StringType)),
        (Types::TEXT, Arc::new(TextType)),
        (Types::TIME_MUTABLE, Arc::new(TimeType)),
        (Types::TIME_IMMUTABLE, Arc::new(TimeImmutableType)),
    ]
}

/// `Type::getTypeRegistry()`: the process-wide registry, created with the
/// built-in types on first use.
pub fn get_type_registry() -> &'static TypeRegistry {
    static REGISTRY: OnceLock<TypeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let registry = TypeRegistry::new();
        for (name, type_) in builtin_types() {
            // The built-in names and instances are distinct.
            let _ = registry.register(name, type_);
        }
        registry
    })
}

/// `Type::getType($name)`: factory method to create type instances.
pub fn get_type(name: &str) -> Result<Arc<dyn Type>, TypesException> {
    get_type_registry().get(name)
}

/// `Type::lookupName($type)`: finds a name for the given type.
pub fn lookup_name(type_: &Arc<dyn Type>) -> Result<String, TypesException> {
    get_type_registry().lookup_name(type_)
}

/// `Type::addType($name, $type)`: adds a custom type to the type map.
pub fn add_type(name: &str, type_: TypeArgument) -> Result<(), TypesException> {
    let type_ = type_.instantiate(name)?;
    get_type_registry().register(name, type_)
}

/// `Type::hasType($name)`: checks if exists support for a type.
pub fn has_type(name: &str) -> bool {
    get_type_registry().has(name)
}

/// `Type::overrideType($name, $type)`: overrides an already defined type
/// to use a different implementation.
pub fn override_type(name: &str, type_: TypeArgument) -> Result<(), TypesException> {
    let type_ = type_.instantiate(name)?;
    get_type_registry().override_type(name, type_)
}

/// `Type::getTypesMap()`: the registered type names and their classes.
pub fn get_types_map() -> Vec<(String, String)> {
    get_type_registry()
        .get_map()
        .into_iter()
        .map(|(name, type_)| (name, type_.class().to_owned()))
        .collect()
}

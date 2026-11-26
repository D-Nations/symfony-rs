//! Port of Doctrine\DBAL\Schema\Column.php: object representation of a
//! database column.
//!
//! Options keep PHP's types (`php_value::Value`); `setOptions()` calls
//! the setter each option names, with `strict_types` argument checks.
//! A `Type` instance in a column definition array (the legacy `type`
//! key, `Column::toArray()` without `$skipType`) is a [`TypeObject`].
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Column.php

use std::ops::Deref;
use std::sync::Arc;

use php_value::{Key, PhpArray, PhpObject, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::types::{self, Type};

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser};
use super::column_editor::ColumnEditor;
use super::exception::{self, invalid_state};
use super::name::{Name, UnqualifiedName};

/// A `Doctrine\DBAL\Types\Type` instance as a PHP value.
#[derive(Debug, Clone)]
pub struct TypeObject(pub Arc<dyn Type>);

impl TypeObject {
    /// The type as a PHP value.
    pub fn value(type_: Arc<dyn Type>) -> Value {
        Value::Object(Arc::new(Self(type_)))
    }

    /// The type a value holds (`$value instanceof Type`).
    pub fn of(value: &Value) -> Option<&Arc<dyn Type>> {
        Some(&value.as_object()?.downcast_ref::<Self>()?.0)
    }
}

impl PhpObject for TypeObject {
    fn class(&self) -> &str {
        self.0.class()
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

const LINK_TYPE_INSTANCE: &str = "https://github.com/doctrine/dbal/pull/7490";
const LINK_JSONB: &str = "https://github.com/doctrine/dbal/pull/6939";

/// Object representation of a database column.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the properties of Column.php"
)]
pub struct Column {
    asset: AbstractAsset,
    type_name: String,
    length: Option<i64>,
    precision: Option<i64>,
    scale: i64,
    unsigned: bool,
    fixed: bool,
    notnull: bool,
    default: Value,
    autoincrement: bool,
    values: PhpArray,
    platform_options: PhpArray,
    #[allow(clippy::struct_field_names, reason = "Column::$_columnDefinition")]
    column_definition: Option<String>,
    comment: String,
}

/// `Type|string`: what `new Column()` takes as its type.
#[derive(Debug, Clone)]
pub enum ColumnType {
    /// A type name (`Types::STRING`, ...).
    Name(String),
    /// A type instance (deprecated upstream).
    Type(Arc<dyn Type>),
}

impl From<&str> for ColumnType {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<Arc<dyn Type>> for ColumnType {
    fn from(type_: Arc<dyn Type>) -> Self {
        Self::Type(type_)
    }
}

fn type_error(method: &str, parameter: &str, expected: &str, given: &Value) -> Exception {
    Exception::new(
        Exception::TYPE_ERROR,
        format!(
            "Doctrine\\DBAL\\Schema\\Column::{method}(): Argument #1 (${parameter}) must be of type {expected}, {} given",
            given.value_name()
        ),
    )
}

fn optional_int(method: &str, parameter: &str, value: &Value) -> Result<Option<i64>, Exception> {
    match value {
        Value::Null => Ok(None),
        Value::Int(i) => Ok(Some(*i)),
        other => Err(type_error(method, parameter, "?int", other)),
    }
}

fn boolean(method: &str, parameter: &str, value: &Value) -> Result<bool, Exception> {
    match value {
        Value::Bool(b) => Ok(*b),
        other => Err(type_error(method, parameter, "bool", other)),
    }
}

fn utf8(value: &php_value::PhpString) -> Result<String, Exception> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| Exception::new(Exception::UNSUPPORTED, "a column option that is not UTF-8"))
}

impl Column {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Column";

    /// `new Column($name, $type, $options)`: creates a new column.
    pub fn new(
        name: &str,
        type_: impl Into<ColumnType>,
        options: &PhpArray,
    ) -> Result<Self, Exception> {
        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::Named,
            NameParser::Unqualified,
            Some(name),
        );
        let mut column = Self {
            asset,
            type_name: String::new(),
            length: None,
            precision: None,
            scale: 0,
            unsigned: false,
            fixed: false,
            notnull: true,
            default: Value::Null,
            autoincrement: false,
            values: PhpArray::new(),
            platform_options: PhpArray::new(),
            column_definition: None,
            comment: String::new(),
        };
        match type_.into() {
            ColumnType::Type(type_) => {
                deprecations::trigger(LINK_TYPE_INSTANCE, || {
                    "Passing a Doctrine\\DBAL\\Types\\Type instance to Doctrine\\DBAL\\Schema\\Column::__construct() is deprecated, pass the type name instead.".to_owned()
                });
                column.set_type_inner(&type_)?;
            }
            ColumnType::Name(name) => column.type_name = name,
        }
        column.set_options_inner(options)?;
        Ok(column)
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<UnqualifiedName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::Unqualified(name)) => Ok(name.clone()),
            _ => Err(invalid_state::object_name_not_initialized()),
        }
    }

    /// `setOptions($options)` (deprecated upstream; use `Column::editor()`).
    pub fn set_options(&mut self, options: &PhpArray) -> Result<&mut Self, Exception> {
        self.set_options_inner(options)?;
        Ok(self)
    }

    fn set_options_inner(&mut self, options: &PhpArray) -> Result<(), Exception> {
        for (name, value) in options.iter() {
            let name = match name {
                Key::Str(name) => utf8(name)?,
                Key::Int(i) => {
                    // 'set' . $name names no method; UnknownColumnOption::new()
                    // then refuses the int (strict_types).
                    return Err(Exception::new(
                        Exception::TYPE_ERROR,
                        format!(
                            "Doctrine\\DBAL\\Schema\\Exception\\UnknownColumnOption::new(): Argument #1 ($name) must be of type string, int given (option {i})"
                        ),
                    ));
                }
            };
            self.set_option(&name, value)?;
        }
        Ok(())
    }

    /// `$this->{'set' . $name}($value)` (PHP method names are
    /// case-insensitive).
    #[allow(clippy::too_many_lines, reason = "one arm per setter of Column.php")]
    fn set_option(&mut self, name: &str, value: &Value) -> Result<(), Exception> {
        match name.to_ascii_lowercase().as_str() {
            "options" => match value {
                Value::Array(options) => self.set_options_inner(options),
                other => Err(type_error("setOptions", "options", "array", other)),
            },
            "type" => match TypeObject::of(value) {
                Some(type_) => {
                    let type_ = Arc::clone(type_);
                    self.set_type_inner(&type_)
                }
                None => Err(type_error(
                    "setType",
                    "type",
                    "Doctrine\\DBAL\\Types\\Type",
                    value,
                )),
            },
            "typename" => match value {
                Value::String(name) => {
                    self.type_name = utf8(name)?;
                    Ok(())
                }
                other => Err(type_error("setTypeName", "typeName", "string", other)),
            },
            "length" => {
                self.length = optional_int("setLength", "length", value)?;
                Ok(())
            }
            "precision" => {
                self.precision = optional_int("setPrecision", "precision", value)?;
                Ok(())
            }
            "scale" => match value {
                Value::Int(i) => {
                    self.scale = *i;
                    Ok(())
                }
                other => Err(type_error("setScale", "scale", "int", other)),
            },
            "unsigned" => {
                self.unsigned = boolean("setUnsigned", "unsigned", value)?;
                Ok(())
            }
            "fixed" => {
                self.fixed = boolean("setFixed", "fixed", value)?;
                Ok(())
            }
            "notnull" => {
                self.notnull = boolean("setNotnull", "notnull", value)?;
                Ok(())
            }
            "default" => {
                self.default = value.clone();
                Ok(())
            }
            "platformoptions" => match value {
                Value::Array(options) => {
                    self.set_platform_options_inner(options.clone());
                    Ok(())
                }
                other => Err(type_error(
                    "setPlatformOptions",
                    "platformOptions",
                    "array",
                    other,
                )),
            },
            "columndefinition" => match value {
                Value::Null => {
                    self.column_definition = None;
                    Ok(())
                }
                Value::String(definition) => {
                    self.column_definition = Some(utf8(definition)?);
                    Ok(())
                }
                other => Err(type_error("setColumnDefinition", "value", "?string", other)),
            },
            "autoincrement" => {
                self.autoincrement = boolean("setAutoincrement", "flag", value)?;
                Ok(())
            }
            "comment" => match value {
                Value::String(comment) => {
                    self.comment = utf8(comment)?;
                    Ok(())
                }
                other => Err(type_error("setComment", "comment", "string", other)),
            },
            "values" => match value {
                Value::Array(values) => {
                    self.values = values.clone();
                    Ok(())
                }
                other => Err(type_error("setValues", "values", "array", other)),
            },
            "platformoption" | "name" => Err(Exception::new(
                Exception::UNSUPPORTED,
                format!("the column option \"{name}\", whose setter does not take one value"),
            )),
            _ => Err(exception::unknown_column_option(name)),
        }
    }

    /// `setType($type)` (deprecated upstream).
    pub fn set_type(&mut self, type_: &Arc<dyn Type>) -> Result<&mut Self, Exception> {
        self.set_type_inner(type_)?;
        Ok(self)
    }

    fn set_type_inner(&mut self, type_: &Arc<dyn Type>) -> Result<(), Exception> {
        self.type_name = types::get_type_registry()
            .lookup_name(type_)
            .map_err(Exception::from)?;
        Ok(())
    }

    pub fn set_type_name(&mut self, type_name: &str) -> &mut Self {
        type_name.clone_into(&mut self.type_name);
        self
    }

    pub fn set_length(&mut self, length: Option<i64>) -> &mut Self {
        self.length = length;
        self
    }

    pub fn set_precision(&mut self, precision: Option<i64>) -> &mut Self {
        self.precision = precision;
        self
    }

    pub fn set_scale(&mut self, scale: i64) -> &mut Self {
        self.scale = scale;
        self
    }

    pub fn set_unsigned(&mut self, unsigned: bool) -> &mut Self {
        self.unsigned = unsigned;
        self
    }

    pub fn set_fixed(&mut self, fixed: bool) -> &mut Self {
        self.fixed = fixed;
        self
    }

    pub fn set_notnull(&mut self, notnull: bool) -> &mut Self {
        self.notnull = notnull;
        self
    }

    pub fn set_default(&mut self, default: Value) -> &mut Self {
        self.default = default;
        self
    }

    /// `setPlatformOptions($platformOptions)`.
    pub fn set_platform_options(&mut self, platform_options: PhpArray) -> &mut Self {
        if platform_options
            .get(&Key::from("jsonb"))
            .is_some_and(Value::is_truthy)
        {
            jsonb_deprecation();
        }
        self.set_platform_options_inner(platform_options);
        self
    }

    fn set_platform_options_inner(&mut self, platform_options: PhpArray) {
        self.platform_options = platform_options;
    }

    /// `setPlatformOption($name, $value)`.
    pub fn set_platform_option(&mut self, name: &str, value: Value) -> &mut Self {
        if name == "jsonb" && value.is_truthy() {
            jsonb_deprecation();
        }
        self.platform_options.insert(name, value);
        self
    }

    pub fn set_column_definition(&mut self, value: Option<&str>) -> &mut Self {
        self.column_definition = value.map(str::to_owned);
        self
    }

    /// `getType()` (deprecated upstream: use `getTypeName()`).
    pub fn get_type(&self) -> Result<Arc<dyn Type>, Exception> {
        deprecations::trigger_if_called_from_outside(LINK_TYPE_INSTANCE, || {
            "Doctrine\\DBAL\\Schema\\Column::getType is deprecated. Use Column::getTypeName() instead."
                .to_owned()
        });
        self.get_type_inner()
    }

    fn get_type_inner(&self) -> Result<Arc<dyn Type>, Exception> {
        types::get_type(&self.type_name).map_err(Exception::from)
    }

    pub fn get_type_name(&self) -> &str {
        &self.type_name
    }

    pub fn get_length(&self) -> Option<i64> {
        self.length
    }

    pub fn get_precision(&self) -> Option<i64> {
        self.precision
    }

    pub fn get_scale(&self) -> i64 {
        self.scale
    }

    pub fn get_unsigned(&self) -> bool {
        self.unsigned
    }

    pub fn get_fixed(&self) -> bool {
        self.fixed
    }

    pub fn get_notnull(&self) -> bool {
        self.notnull
    }

    pub fn get_default(&self) -> &Value {
        &self.default
    }

    fn platform_string(&self, name: &str) -> Option<String> {
        match self.platform_options.get(&Key::from(name)) {
            Some(Value::String(s)) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        }
    }

    /// The charset platform option.
    pub fn get_charset(&self) -> Option<String> {
        self.platform_string("charset")
    }

    /// The collation platform option.
    pub fn get_collation(&self) -> Option<String> {
        self.platform_string("collation")
    }

    /// The `min` platform option.
    pub fn get_minimum_value(&self) -> &Value {
        self.platform_options
            .get(&Key::from("min"))
            .unwrap_or(&Value::Null)
    }

    /// The `max` platform option.
    pub fn get_maximum_value(&self) -> &Value {
        self.platform_options
            .get(&Key::from("max"))
            .unwrap_or(&Value::Null)
    }

    /// The `enumType` platform option.
    pub fn get_enum_type(&self) -> Option<String> {
        self.platform_string("enumType")
    }

    /// The SQL Server default constraint name platform option.
    pub fn get_default_constraint_name(&self) -> Option<String> {
        self.platform_string("default_constraint_name")
    }

    pub fn get_platform_options(&self) -> &PhpArray {
        &self.platform_options
    }

    /// `isset($this->_platformOptions[$name])`.
    pub fn has_platform_option(&self, name: &str) -> bool {
        self.platform_options
            .get(&Key::from(name))
            .is_some_and(|v| !v.is_null())
    }

    /// `$this->_platformOptions[$name]` (`None` where PHP warns of an
    /// undefined key and gives `null`).
    pub fn get_platform_option(&self, name: &str) -> Option<&Value> {
        self.platform_options.get(&Key::from(name))
    }

    pub fn get_column_definition(&self) -> Option<&str> {
        self.column_definition.as_deref()
    }

    pub fn get_autoincrement(&self) -> bool {
        self.autoincrement
    }

    pub fn set_autoincrement(&mut self, flag: bool) -> &mut Self {
        self.autoincrement = flag;
        self
    }

    pub fn set_comment(&mut self, comment: &str) -> &mut Self {
        comment.clone_into(&mut self.comment);
        self
    }

    pub fn get_comment(&self) -> &str {
        &self.comment
    }

    pub fn set_values(&mut self, values: PhpArray) -> &mut Self {
        self.values = values;
        self
    }

    pub fn get_values(&self) -> &PhpArray {
        &self.values
    }

    /// `toArray($skipType)`: the column definition array. Without
    /// `$skipType` (deprecated) it holds the `Type` instance under `type`.
    pub fn to_array(&self, skip_type: bool) -> Result<PhpArray, Exception> {
        if !skip_type {
            deprecations::trigger(LINK_TYPE_INSTANCE, || {
                "Calling Doctrine\\DBAL\\Schema\\Column::toArray() without the $skipType argument is deprecated. Pass true to omit the Type instance from the returned array and read the \"typeName\" key instead.".to_owned()
            });
        }
        let optional_int = |v: Option<i64>| v.map_or(Value::Null, Value::Int);
        let mut array = PhpArray::new();
        array.insert("name", Value::string(self.asset.name.as_str()));
        array.insert("typeName", Value::string(self.type_name.as_str()));
        array.insert("default", self.default.clone());
        array.insert("notnull", Value::Bool(self.notnull));
        array.insert("length", optional_int(self.length));
        array.insert("precision", optional_int(self.precision));
        array.insert("scale", Value::Int(self.scale));
        array.insert("fixed", Value::Bool(self.fixed));
        array.insert("unsigned", Value::Bool(self.unsigned));
        array.insert("autoincrement", Value::Bool(self.autoincrement));
        array.insert(
            "columnDefinition",
            self.column_definition
                .as_deref()
                .map_or(Value::Null, Value::string),
        );
        array.insert("comment", Value::string(self.comment.as_str()));
        array.insert("values", Value::Array(self.values.clone()));
        if !skip_type {
            array.insert("type", TypeObject::value(self.get_type_inner()?));
        }
        array_merge_into(&mut array, &self.platform_options);
        Ok(array)
    }

    /// `Column::editor()`.
    pub fn editor() -> ColumnEditor {
        ColumnEditor::new()
    }

    /// `edit()`: an editor holding this column's definition.
    pub fn edit(&self) -> Result<ColumnEditor, Exception> {
        Ok(Self::editor()
            .set_name(self.get_object_name()?)
            .set_type_name(&self.type_name)
            .set_length(self.length)
            .set_precision(self.precision)
            .set_scale(self.scale)
            .set_unsigned(self.unsigned)
            .set_fixed(self.fixed)
            .set_not_null(self.notnull)
            .set_default_value(self.default.clone())
            .set_autoincrement(self.autoincrement)
            .set_comment(&self.comment)
            .set_values(self.values.clone())
            .set_column_definition(self.column_definition.as_deref())
            .set_charset(self.get_charset().as_deref())
            .set_collation(self.get_collation().as_deref())
            .set_minimum_value(self.get_minimum_value().clone())
            .set_maximum_value(self.get_maximum_value().clone())
            .set_enum_type(self.get_enum_type().as_deref())
            .set_default_constraint_name(self.get_default_constraint_name().as_deref()))
    }

    /// The asset state, for the platform's quoting.
    pub(crate) fn asset_mut(&mut self) -> &mut AbstractAsset {
        &mut self.asset
    }
}

impl Deref for Column {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}

fn jsonb_deprecation() {
    deprecations::trigger_if_called_from_outside(LINK_JSONB, || {
        "The \"jsonb\" column platform option is deprecated. Use the \"JSONB\" type instead."
            .to_owned()
    });
}

/// `array_merge($array, $other)` in place: string keys replace, int keys
/// are appended.
pub(crate) fn array_merge_into(array: &mut PhpArray, other: &PhpArray) {
    for (key, value) in other.iter() {
        match key {
            Key::Str(_) => array.insert(key.clone(), value.clone()),
            Key::Int(_) => {
                let _ = array.push(value.clone());
            }
        }
    }
}

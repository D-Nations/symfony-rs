//! What the Types tests share: a stand-in for PHPUnit's
//! `createStub(AbstractPlatform::class)` / `createMock(...)`, and readers
//! for recorded provider values.
//!
//! A stub answers every method with PHPUnit's default for its return type
//! (`''` for `string`, `null` for `mixed` and `?bool`) unless a return was
//! configured (`->method(...)->willReturn(...)`), and records each call,
//! so a test checks `expects(self::once())` and `->with(...)` on the
//! recorded calls.
//!
//! Upstream: none
#![allow(dead_code)]

use std::{cell::RefCell, collections::HashMap};

use doctrine_dbal::{exception::Exception, platforms::AbstractPlatform};
use php_value::{
    PhpArray, PhpString, Value,
    datetime::{DateTime, DateTimeImmutable},
    vectors::decode,
};
use serde_json::Value as Json;

/// `createStub(AbstractPlatform::class)`.
#[derive(Debug, Default)]
pub struct StubPlatform {
    returns: RefCell<HashMap<&'static str, Value>>,
    calls: RefCell<Vec<(&'static str, Vec<Value>)>>,
}

impl StubPlatform {
    pub fn new() -> Self {
        Self::default()
    }

    /// `->method($name)->willReturn($value)`.
    pub fn will_return(&self, method: &'static str, value: impl Into<Value>) -> &Self {
        self.returns.borrow_mut().insert(method, value.into());
        self
    }

    /// How many times `method` was called.
    pub fn calls(&self, method: &str) -> usize {
        self.calls
            .borrow()
            .iter()
            .filter(|(name, _)| *name == method)
            .count()
    }

    /// The arguments of each call of `method`.
    pub fn arguments(&self, method: &str) -> Vec<Vec<Value>> {
        self.calls
            .borrow()
            .iter()
            .filter(|(name, _)| *name == method)
            .map(|(_, arguments)| arguments.clone())
            .collect()
    }

    fn call(&self, method: &'static str, arguments: Vec<Value>) -> Option<Value> {
        self.calls.borrow_mut().push((method, arguments));
        self.returns.borrow().get(method).cloned()
    }

    fn string(&self, method: &'static str, arguments: Vec<Value>) -> PhpString {
        match self.call(method, arguments) {
            Some(Value::String(s)) => s,
            Some(other) => panic!("{method} configured to return {other:?}"),
            None => PhpString::default(),
        }
    }

    #[allow(clippy::unnecessary_wraps, reason = "the trait's signature")]
    fn declaration(&self, method: &'static str, column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(self.string(method, vec![Value::Array(column.clone())]))
    }

    fn format(&self, method: &'static str) -> String {
        self.string(method, Vec::new())
            .to_string_lossy()
            .into_owned()
    }
}

impl AbstractPlatform for StubPlatform {
    fn class(&self) -> &'static str {
        "MockObject_AbstractPlatform"
    }

    fn get_current_database_expression(&self) -> Result<String, doctrine_dbal::Exception> {
        Ok(String::new())
    }

    fn get_set_transaction_isolation_sql(
        &self,
        _level: doctrine_dbal::TransactionIsolationLevel,
    ) -> Result<String, doctrine_dbal::Exception> {
        Ok(String::new())
    }

    fn get_boolean_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getBooleanTypeDeclarationSQL", column)
    }

    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getIntegerTypeDeclarationSQL", column)
    }

    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getBigIntTypeDeclarationSQL", column)
    }

    fn get_small_int_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.declaration("getSmallIntTypeDeclarationSQL", column)
    }

    fn get_ascii_string_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.declaration("getAsciiStringTypeDeclarationSQL", column)
    }

    fn get_string_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getStringTypeDeclarationSQL", column)
    }

    fn get_binary_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getBinaryTypeDeclarationSQL", column)
    }

    fn get_enum_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getEnumDeclarationSQL", column)
    }

    fn get_guid_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getGuidTypeDeclarationSQL", column)
    }

    fn get_json_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getJsonTypeDeclarationSQL", column)
    }

    fn get_jsonb_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getJsonbTypeDeclarationSQL", column)
    }

    fn get_clob_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getClobTypeDeclarationSQL", column)
    }

    fn get_blob_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getBlobTypeDeclarationSQL", column)
    }

    fn get_decimal_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getDecimalTypeDeclarationSQL", column)
    }

    fn convert_booleans(&self, item: &Value) -> Value {
        self.call("convertBooleans", vec![item.clone()])
            .unwrap_or_default()
    }

    fn convert_from_boolean(&self, item: &Value) -> Option<bool> {
        match self.call("convertFromBoolean", vec![item.clone()]) {
            Some(Value::Bool(b)) => Some(b),
            Some(other) => panic!("convertFromBoolean configured to return {other:?}"),
            None => None,
        }
    }

    fn convert_booleans_to_database_value(&self, item: &Value) -> Value {
        self.call("convertBooleansToDatabaseValue", vec![item.clone()])
            .unwrap_or_default()
    }

    fn get_date_time_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.declaration("getDateTimeTypeDeclarationSQL", column)
    }

    fn get_date_time_tz_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.declaration("getDateTimeTzTypeDeclarationSQL", column)
    }

    fn get_date_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getDateTypeDeclarationSQL", column)
    }

    fn get_time_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getTimeTypeDeclarationSQL", column)
    }

    fn get_float_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getFloatDeclarationSQL", column)
    }

    fn get_small_float_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.declaration("getSmallFloatDeclarationSQL", column)
    }

    fn get_date_time_format_string(&self) -> String {
        self.format("getDateTimeFormatString")
    }

    fn get_date_time_tz_format_string(&self) -> String {
        self.format("getDateTimeTzFormatString")
    }

    fn get_date_format_string(&self) -> String {
        self.format("getDateFormatString")
    }

    fn get_time_format_string(&self) -> String {
        self.format("getTimeFormatString")
    }
}

/// A recorded provider value: `php_value::vectors::decode()`, with a
/// mutable `DateTime` (`["datetime_mutable", ...]`) as a `DateTime`
/// object, in arrays too.
pub fn provider_value(json: &Json) -> Value {
    match json[0].as_str() {
        Some("datetime_mutable") => {
            let Value::Object(object) = decode(&json[1]).expect("typed date") else {
                panic!("not a date: {json}");
            };
            let date = *object
                .downcast_ref::<DateTimeImmutable>()
                .expect("a DateTimeImmutable");
            DateTime::object(date)
        }
        Some("array") => {
            let mut array = PhpArray::new();
            for pair in json[1].as_array().expect("pairs") {
                let key = match decode(&pair[0]).expect("key") {
                    Value::Int(i) => php_value::Key::Int(i),
                    Value::String(s) => php_value::Key::Str(s),
                    other => panic!("bad key {other:?}"),
                };
                array.insert(key, provider_value(&pair[1]));
            }
            Value::Array(array)
        }
        _ => decode(json).expect("typed value"),
    }
}

/// `new DateTime($text)` / `new DateTimeImmutable($text)`.
pub fn date(text: &str) -> DateTimeImmutable {
    php_value::date_create::date_create(text.as_bytes(), None)
        .expect("a text php_value reads")
        .expect("a valid date")
}

/// `new DateTime()` as a value.
pub fn new_date_time() -> Value {
    DateTime::object(date("now"))
}

/// `new DateTimeImmutable()` as a value.
pub fn new_date_time_immutable() -> Value {
    date("now").object()
}

/// `$date->format($format)` of a `DateTime` or `DateTimeImmutable`.
pub fn format(value: &Value, format: &str) -> String {
    let object = value.as_object().expect("a date object");
    if let Some(date) = object.downcast_ref::<DateTimeImmutable>() {
        return date.format(format);
    }
    object
        .downcast_ref::<DateTime>()
        .expect("a date object")
        .format(format)
}

/// `$value instanceof DateTime`.
pub fn is_date_time(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.downcast_ref::<DateTime>().is_some())
}

/// `$value instanceof DateTimeImmutable`.
pub fn is_date_time_immutable(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.downcast_ref::<DateTimeImmutable>().is_some())
}

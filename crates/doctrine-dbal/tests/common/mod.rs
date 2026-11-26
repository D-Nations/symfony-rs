//! Reads tests/data/php.json (`scripts/record.sh doctrine-dbal`): the
//! upstream data providers, and the parser, expansion and query vectors;
//! and the test doubles of the query tests.
//!
//! Upstream: none
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use doctrine_dbal::platforms::AbstractPlatform;
use doctrine_dbal::query::expression::{CompositeExpression, CompositeType, ExpressionPart};
use doctrine_dbal::sql::builder::{
    DefaultSelectSQLBuilder, DefaultUnionSQLBuilder, SelectSQLBuilder, UnionSQLBuilder,
    WithSQLBuilder,
};
use doctrine_dbal::{
    ArrayParameterType, ConnectionStandIn, Exception, ParameterType, WrapperParameterType,
};
use php_value::vectors::{base64, decode};
use php_value::{Key, PhpArray, PhpString, Value};
use serde_json::Value as Json;

pub fn data() -> &'static Json {
    static DATA: OnceLock<Json> = OnceLock::new();
    DATA.get_or_init(|| {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/php.json"))
                .expect("tests/data/php.json (scripts/record.sh doctrine-dbal)");
        serde_json::from_str(&text).expect("valid JSON")
    })
}

/// The rows of a data provider: (row name, typed JSON arguments).
pub fn provider(class: &str, method: &str) -> Vec<(String, Vec<Json>)> {
    let rows = data()["providers"][class][method]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no provider for {class}::{method}"));
    rows.iter()
        .map(|row| {
            let name = row[0].as_str().expect("row name").to_owned();
            let args = row[1].as_array().expect("arguments").clone();
            (name, args)
        })
        .collect()
}

pub fn bytes(json: &Json) -> Vec<u8> {
    assert_eq!(json[0], "string", "not a string: {json}");
    base64(json[1].as_str().expect("base64")).expect("base64")
}

pub fn bool(json: &Json) -> bool {
    assert_eq!(json[0], "bool", "not a bool: {json}");
    json[1].as_bool().expect("bool")
}

pub fn array(json: &Json) -> PhpArray {
    match decode(json).expect("typed value") {
        Value::Array(array) => array,
        other => panic!("not an array: {other:?}"),
    }
}

/// A recorded `string|ParameterType|ArrayParameterType`.
pub fn wrapper_type(json: &Json) -> WrapperParameterType {
    match json[0].as_str() {
        Some("string") => WrapperParameterType::Name(bytes(json).into()),
        Some("enum") => {
            let case = json[2].as_str().expect("case");
            match json[1].as_str() {
                Some("Doctrine\\DBAL\\ParameterType") => {
                    WrapperParameterType::Parameter(ParameterType::from_name(case).expect("case"))
                }
                Some("Doctrine\\DBAL\\ArrayParameterType") => {
                    WrapperParameterType::Array(ArrayParameterType::from_name(case).expect("case"))
                }
                other => panic!("unknown enum {other:?}"),
            }
        }
        _ => panic!("not a parameter type: {json}"),
    }
}

/// A recorded array of types, by key.
pub fn types(json: &Json) -> BTreeMap<Key, WrapperParameterType> {
    assert_eq!(json[0], "array", "not an array: {json}");
    json[1]
        .as_array()
        .expect("pairs")
        .iter()
        .map(|pair| {
            let key = match decode(&pair[0]).expect("key") {
                Value::Int(i) => Key::Int(i),
                Value::String(s) => Key::Str(s),
                other => panic!("bad key {other:?}"),
            };
            (key, wrapper_type(&pair[1]))
        })
        .collect()
}

/// A recorded array of types, by key, in the array's order.
pub fn ordered_types(json: &Json) -> Vec<(Key, WrapperParameterType)> {
    assert_eq!(json[0], "array", "not an array: {json}");
    json[1]
        .as_array()
        .expect("pairs")
        .iter()
        .map(|pair| {
            let key = match decode(&pair[0]).expect("key") {
                Value::Int(i) => Key::Int(i),
                Value::String(s) => Key::Str(s),
                other => panic!("bad key {other:?}"),
            };
            (key, wrapper_type(&pair[1]))
        })
        .collect()
}

/// A recorded string as UTF-8 text.
pub fn text(json: &Json) -> String {
    String::from_utf8(bytes(json)).expect("UTF-8")
}

/// A recorded `string|CompositeExpression` (["composite", type, parts]).
pub fn expression_part(json: &Json) -> ExpressionPart {
    match json[0].as_str() {
        Some("string") => ExpressionPart::Sql(text(json)),
        Some("composite") => {
            let r#type = match json[1].as_str() {
                Some("AND") => CompositeType::And,
                Some("OR") => CompositeType::Or,
                other => panic!("composite type {other:?}"),
            };
            let mut parts = expression_parts(&json[2]).into_iter();
            let first = parts.next().expect("a part");
            CompositeExpression::new(r#type, first, parts).into()
        }
        _ => panic!("not an expression part: {json}"),
    }
}

/// A recorded list of `string|CompositeExpression`: a typed array, or
/// the parts list of a composite.
pub fn expression_parts(json: &Json) -> Vec<ExpressionPart> {
    if json[0] == "array" {
        json[1]
            .as_array()
            .expect("pairs")
            .iter()
            .map(|pair| expression_part(&pair[1]))
            .collect()
    } else {
        json.as_array()
            .expect("parts")
            .iter()
            .map(expression_part)
            .collect()
    }
}

/// `QueryBuilderTest`'s platform stub: `createStub(AbstractPlatform::class)`
/// with union parts as given, `UNION ALL` / `UNION`, and a select builder
/// without FOR UPDATE. Its other methods are `AbstractPlatform`'s own
/// (PHPUnit doubles only public methods; the limit methods it calls are
/// final or protected).
pub struct StubPlatform;

/// The methods `AbstractPlatform` declares abstract, answered as a
/// PHPUnit stub answers an unconfigured call (an empty string). The Query
/// tests never call them.
macro_rules! abstract_platform_stub_methods {
    () => {
        fn class(&self) -> &str {
            "Mock_AbstractPlatform"
        }
        abstract_platform_stub_methods!(
            get_boolean_type_declaration_sql,
            get_integer_type_declaration_sql,
            get_big_int_type_declaration_sql,
            get_small_int_type_declaration_sql,
            get_clob_type_declaration_sql,
            get_blob_type_declaration_sql,
            get_date_time_type_declaration_sql,
            get_date_type_declaration_sql,
            get_time_type_declaration_sql
        );
        fn get_current_database_expression(&self) -> Result<String, doctrine_dbal::Exception> {
            Ok(String::new())
        }
        fn get_set_transaction_isolation_sql(
            &self,
            _level: doctrine_dbal::TransactionIsolationLevel,
        ) -> Result<String, doctrine_dbal::Exception> {
            Ok(String::new())
        }
    };
    ($($name:ident),*) => {
        $(
            fn $name(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
                Ok(PhpString::from(""))
            }
        )*
    };
}

impl AbstractPlatform for StubPlatform {
    abstract_platform_stub_methods!();

    fn get_union_select_part_sql(&self, sub_query: &str) -> String {
        sub_query.to_owned()
    }

    fn get_union_all_sql(&self) -> String {
        "UNION ALL".to_owned()
    }

    fn get_union_distinct_sql(&self) -> String {
        "UNION".to_owned()
    }

    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, None, None))
    }

    fn create_union_sql_builder(&self) -> Box<dyn UnionSQLBuilder + '_> {
        Box::new(DefaultUnionSQLBuilder::new(self))
    }

    fn create_with_sql_builder(&self) -> WithSQLBuilder {
        WithSQLBuilder::new()
    }
}

/// A platform with `AbstractPlatform`'s own methods (as `PostgreSQLPlatform`
/// keeps them, which the recorder uses).
pub struct DefaultPlatform;

impl AbstractPlatform for DefaultPlatform {
    abstract_platform_stub_methods!();
}

/// `createMock(Connection::class)` with `getDatabasePlatform()` set; a
/// mocked `quote()` gives its return type's default, `''`.
pub struct TestConnection<P> {
    pub platform: P,
}

impl<P: AbstractPlatform> ConnectionStandIn for TestConnection<P> {
    type Platform = P;

    fn get_database_platform(&self) -> Result<&P, Exception> {
        Ok(&self.platform)
    }

    fn quote(&self, _value: &str) -> Result<String, Exception> {
        Ok(String::new())
    }
}

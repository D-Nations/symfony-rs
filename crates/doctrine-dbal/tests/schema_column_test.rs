//! Port of tests/Schema/ColumnTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `testQuotedColumnName` needs the `MySQL` and SQL Server
//! platforms, which are not ported (pending).
//!
//! Upstream: doctrine/dbal tests/Schema/ColumnTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use std::sync::Arc;

use doctrine_dbal::schema::name::{Identifier, UnqualifiedName};
use doctrine_dbal::schema::{Column, TypeObject};
use doctrine_dbal::types::{self, Type, Types};
use php_value::{Key, PhpArray, Value, php_array};
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

const SELF_CLASS: &str = "Doctrine\\DBAL\\Tests\\Schema\\ColumnTest";

fn create_column() -> Column {
    Column::editor()
        .set_unquoted_name("foo")
        .set_type_name(Types::STRING)
        .set_length(Some(200))
        .set_precision(Some(5))
        .set_scale(2)
        .set_unsigned(true)
        .set_not_null(false)
        .set_fixed(true)
        .set_default_value(Value::string("baz"))
        .set_charset(Some("utf8"))
        .set_enum_type(Some(SELF_CLASS))
        .create()
        .unwrap()
}

fn same_type(a: &Arc<dyn Type>, b: &Arc<dyn Type>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

fn expected_array() -> PhpArray {
    php_array![
        "name" => "foo",
        "typeName" => Types::STRING,
        "default" => "baz",
        "notnull" => false,
        "length" => 200,
        "precision" => 5,
        "scale" => 2,
        "fixed" => true,
        "unsigned" => true,
        "autoincrement" => false,
        "columnDefinition" => Value::Null,
        "comment" => "",
        "values" => PhpArray::new(),
    ]
}

#[test]
fn testGet() {
    let column = create_column();

    assert_eq!(
        column.get_object_name().unwrap(),
        UnqualifiedName::unquoted("foo").unwrap()
    );
    assert!(same_type(
        &column.get_type().unwrap(),
        &types::get_type(Types::STRING).unwrap()
    ));

    assert_eq!(column.get_length(), Some(200));
    assert_eq!(column.get_precision(), Some(5));
    assert_eq!(column.get_scale(), 2);
    assert!(column.get_unsigned());
    assert!(!column.get_notnull());
    assert!(column.get_fixed());
    assert_eq!(column.get_default(), &Value::string("baz"));

    assert_eq!(
        column.get_platform_options(),
        &php_array!["charset" => "utf8", "enumType" => SELF_CLASS]
    );
    assert!(column.has_platform_option("charset"));
    assert_eq!(
        column.get_platform_option("charset"),
        Some(&Value::string("utf8"))
    );
    assert!(!column.has_platform_option("collation"));
    assert!(column.has_platform_option("enumType"));
    assert_eq!(
        column.get_platform_option("enumType"),
        Some(&Value::string(SELF_CLASS))
    );
}

#[test]
fn testToArrayWithType() {
    let column = create_column();
    let array = expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        column.to_array(false).unwrap()
    });

    let mut expected = expected_array();
    expected.insert("type", Value::Null);
    expected.insert("charset", Value::string("utf8"));
    expected.insert("enumType", Value::string(SELF_CLASS));

    // The keys in order, then the values; `type` is the registered
    // instance (`assertSame()`).
    let keys = |a: &PhpArray| a.iter().map(|(k, _)| k.clone()).collect::<Vec<Key>>();
    assert_eq!(keys(&array), keys(&expected));
    for (key, value) in expected.iter() {
        if *key == Key::from("type") {
            let type_ = TypeObject::of(array.get(key).unwrap()).expect("a Type");
            assert!(same_type(type_, &types::get_type(Types::STRING).unwrap()));
        } else {
            assert_eq!(array.get(key), Some(value), "{key:?}");
        }
    }
}

#[test]
fn testToArray() {
    let column = create_column();
    let array = expect_no_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        column.to_array(true).unwrap()
    });

    let mut expected = expected_array();
    expected.insert("charset", Value::string("utf8"));
    expected.insert("enumType", Value::string(SELF_CLASS));
    assert_eq!(array, expected);
}

#[test]
fn testSettingUnknownOptionIsStillSupported() {
    let mut column = Column::editor()
        .set_unquoted_name("foo")
        .set_type_name(Types::STRING)
        .create()
        .unwrap();

    let e = expect_exception(
        "UnknownColumnOption",
        column
            .set_options(&php_array!["unknown_option" => "bar"])
            .map(|_| ()),
    );
    assert_eq!(
        e.message().to_string_lossy(),
        "The \"unknown_option\" column option is not supported."
    );
}

#[test]
fn testOptionsShouldNotBeIgnored() {
    let e = expect_exception(
        "UnknownColumnOption",
        Column::new(
            "bar",
            types::get_type(Types::INTEGER).unwrap(),
            &php_array!["unknown_option" => "bar", "notnull" => true],
        ),
    );
    assert_eq!(
        e.message().to_string_lossy(),
        "The \"unknown_option\" column option is not supported."
    );
}

#[test]
fn testIsQuoted() {
    for (column_name, is_quoted) in [
        ("bar", false),
        ("`bar`", true),
        ("\"bar\"", true),
        ("[bar]", true),
    ] {
        let column = Column::new(
            column_name,
            types::get_type(Types::STRING).unwrap(),
            &PhpArray::new(),
        )
        .unwrap();
        assert_eq!(column.is_quoted(), is_quoted, "{column_name}");
    }
}

#[test]
fn testColumnComment() {
    let column = Column::editor()
        .set_unquoted_name("bar")
        .set_type_name(Types::STRING)
        .create()
        .unwrap();
    assert_eq!(column.get_comment(), "");

    let column = column.edit().unwrap().set_comment("foo").create().unwrap();
    assert_eq!(column.get_comment(), "foo");

    let column_array = column.to_array(false).unwrap();
    assert_eq!(
        column_array.get(&Key::from("comment")),
        Some(&Value::string("foo"))
    );
}

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        Column::new(
            "",
            types::get_type(Types::INTEGER).unwrap(),
            &PhpArray::new(),
        )
        .unwrap();
    });
}

#[test]
fn testGetObjectName() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap();
    assert_eq!(
        column.get_object_name().unwrap().get_identifier(),
        &Identifier::unquoted("id").unwrap()
    );
}

#[test]
fn testPassingTypeInstanceToConstructorIsDeprecated() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        Column::new(
            "foo",
            types::get_type(Types::STRING).unwrap(),
            &PhpArray::new(),
        )
        .unwrap();
    });
}

#[test]
fn testGetTypeIsDeprecated() {
    let column = Column::new("foo", Types::STRING, &PhpArray::new()).unwrap();
    let type_ = expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        column.get_type().unwrap()
    });
    assert_eq!(type_.class(), "Doctrine\\DBAL\\Types\\StringType");
}

#[test]
fn testGetTypeNameIsNotDeprecated() {
    let column = Column::new("foo", Types::STRING, &PhpArray::new()).unwrap();
    let name = expect_no_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        column.get_type_name().to_owned()
    });
    assert_eq!(name, Types::STRING);
}

#[test]
fn testSetPlatformOptionJsonb() {
    let mut column = Column::new(
        "jsonb",
        types::get_type(Types::JSON).unwrap(),
        &PhpArray::new(),
    )
    .unwrap();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6939", || {
        column.set_platform_option("jsonb", Value::Bool(true));
    });
}

#[test]
fn testSetPlatformOptionsJsonb() {
    let mut column = Column::new(
        "jsonb",
        types::get_type(Types::JSON).unwrap(),
        &PhpArray::new(),
    )
    .unwrap();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6939", || {
        column.set_platform_options(php_array!["jsonb" => true]);
    });
}

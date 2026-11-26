//! Port of tests/Schema/ColumnEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/ColumnEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::Column;
use doctrine_dbal::schema::name::UnqualifiedName;
use doctrine_dbal::types::{self, Types};
use php_value::Value;
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

#[test]
fn testSetUnquotedName() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap();
    assert_eq!(
        column.get_object_name().unwrap(),
        UnqualifiedName::unquoted("id").unwrap()
    );
}

#[test]
fn testSetQuotedName() {
    let column = Column::editor()
        .set_quoted_name("id")
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap();
    assert_eq!(
        column.get_object_name().unwrap(),
        UnqualifiedName::quoted("id").unwrap()
    );
}

#[test]
fn testSetType() {
    let type_ = types::get_type(Types::INTEGER).unwrap();
    let column = expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        Column::editor()
            .set_unquoted_name("id")
            .set_type(&type_)
            .create()
            .unwrap()
    });
    assert_eq!(column.get_type_name(), Types::INTEGER);
}

#[test]
fn testSetTypeName() {
    let column = expect_no_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        Column::editor()
            .set_unquoted_name("id")
            .set_type_name(Types::INTEGER)
            .create()
            .unwrap()
    });
    assert_eq!(column.get_type_name(), Types::INTEGER);
}

#[test]
fn testSetCollation() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::STRING)
        .set_collation(Some("utf8_bin"))
        .create()
        .unwrap();
    assert_eq!(column.get_collation().as_deref(), Some("utf8_bin"));
}

#[test]
fn testSetMinimumValue() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .set_minimum_value(Value::Int(1))
        .create()
        .unwrap();
    assert_eq!(column.get_minimum_value(), &Value::Int(1));
}

#[test]
fn testSetMaximumValue() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .set_maximum_value(Value::Int(10))
        .create()
        .unwrap();
    assert_eq!(column.get_maximum_value(), &Value::Int(10));
}

#[test]
fn testSetDefaultConstraintName() {
    let column = Column::editor()
        .set_unquoted_name("id")
        .set_type_name(Types::INTEGER)
        .set_default_constraint_name(Some("df_id"))
        .create()
        .unwrap();
    assert_eq!(
        column.get_default_constraint_name().as_deref(),
        Some("df_id")
    );
}

#[test]
fn testNameNotSet() {
    expect_exception("InvalidColumnDefinition", Column::editor().create());
}

#[test]
fn testTypeNotSet() {
    let editor = Column::editor().set_unquoted_name("id");
    expect_exception("InvalidColumnDefinition", editor.create());
}

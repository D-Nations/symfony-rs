//! Port of tests/Schema/PrimaryKeyConstraintEditorTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/PrimaryKeyConstraintEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::PrimaryKeyConstraint;
use doctrine_dbal::schema::name::UnqualifiedName;
use schema_support::expect_exception;

fn create_column_name() -> UnqualifiedName {
    UnqualifiedName::unquoted("id").unwrap()
}

fn u(name: &str) -> UnqualifiedName {
    UnqualifiedName::unquoted(name).unwrap()
}

fn q(name: &str) -> UnqualifiedName {
    UnqualifiedName::quoted(name).unwrap()
}

#[test]
fn testColumnNamesNotSet() {
    let editor = PrimaryKeyConstraint::editor();
    expect_exception("InvalidPrimaryKeyConstraintDefinition", editor.create());
}

#[test]
fn testSetUnquotedName() {
    let constraint = PrimaryKeyConstraint::editor()
        .set_unquoted_name("pk_users")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(constraint.get_object_name(), Some(&u("pk_users")));
}

#[test]
fn testSetQuotedName() {
    let constraint = PrimaryKeyConstraint::editor()
        .set_quoted_name("pk_users")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(constraint.get_object_name(), Some(&q("pk_users")));
}

#[test]
fn testSetUnquotedColumnNames() {
    let constraint = PrimaryKeyConstraint::editor()
        .set_unquoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names(),
        [u("account_id"), u("user_id")]
    );
}

#[test]
fn testSetQuotedColumnNames() {
    let constraint = PrimaryKeyConstraint::editor()
        .set_quoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names(),
        [q("account_id"), q("user_id")]
    );
}

#[test]
fn testAddColumnName() {
    let constraint = PrimaryKeyConstraint::editor()
        .add_unquoted_column_name("account_id")
        .add_quoted_column_name("user_id")
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names(),
        [u("account_id"), q("user_id")]
    );
}

#[test]
fn testSetIsClustered() {
    let constraint = PrimaryKeyConstraint::editor()
        .set_unquoted_column_names(&["id"])
        .create()
        .unwrap();
    assert!(constraint.is_clustered());

    let constraint = constraint.edit().set_is_clustered(false).create().unwrap();
    assert!(!constraint.is_clustered());
}

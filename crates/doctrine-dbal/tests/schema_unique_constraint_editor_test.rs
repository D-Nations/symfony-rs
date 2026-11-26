//! Port of tests/Schema/UniqueConstraintEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the editor is a by-value builder, so an editor used twice
//! is cloned.
//!
//! Upstream: doctrine/dbal tests/Schema/UniqueConstraintEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::UniqueConstraint;
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
fn testEmptyColumnNames() {
    expect_exception(
        "InvalidUniqueConstraintDefinition",
        UniqueConstraint::editor().create(),
    );
}

#[test]
fn testSetUnquotedName() {
    let constraint = UniqueConstraint::editor()
        .set_unquoted_name("uq_id")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(constraint.get_object_name().unwrap(), Some(u("uq_id")));
}

#[test]
fn testSetQuotedName() {
    let constraint = UniqueConstraint::editor()
        .set_quoted_name("uq_id")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(constraint.get_object_name().unwrap(), Some(q("uq_id")));
}

#[test]
fn testSetUnquotedColumnNames() {
    let constraint = UniqueConstraint::editor()
        .set_unquoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names().unwrap(),
        [u("account_id"), u("user_id")]
    );
}

#[test]
fn testSetQuotedColumnNames() {
    let constraint = UniqueConstraint::editor()
        .set_quoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names().unwrap(),
        [q("account_id"), q("user_id")]
    );
}

#[test]
fn testAddColumnNames() {
    let constraint = UniqueConstraint::editor()
        .add_unquoted_column_name("account_id")
        .add_quoted_column_name("user_id")
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_column_names().unwrap(),
        [u("account_id"), q("user_id")]
    );
}

#[test]
fn testSetIsClustered() {
    let editor = UniqueConstraint::editor().set_unquoted_column_names(&["user_id"]);
    let constraint = editor.clone().create().unwrap();
    assert!(!constraint.is_clustered());

    let constraint = editor.set_is_clustered(true).create().unwrap();
    assert!(constraint.is_clustered());
}

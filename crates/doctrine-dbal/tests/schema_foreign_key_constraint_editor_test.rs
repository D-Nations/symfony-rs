//! Port of tests/Schema/ForeignKeyConstraintEditorTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; the editor is a by-value builder, so an
//! editor used twice is cloned.
//!
//! Upstream: doctrine/dbal tests/Schema/ForeignKeyConstraintEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::name::{OptionallyQualifiedName, UnqualifiedName};
use doctrine_dbal::schema::{
    Deferrability, ForeignKeyConstraint, ForeignKeyConstraintEditor, MatchType, ReferentialAction,
};
use schema_support::expect_exception;

fn create_minimal_valid_editor() -> ForeignKeyConstraintEditor {
    ForeignKeyConstraint::editor()
        .set_referenced_table_name(create_table_name())
        .set_referencing_column_names(vec![create_column_name()])
        .set_referenced_column_names(vec![create_column_name()])
}

fn create_table_name() -> OptionallyQualifiedName {
    OptionallyQualifiedName::unquoted("users", None).unwrap()
}

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
fn testReferencedTableNameNotSet() {
    let editor = ForeignKeyConstraint::editor()
        .set_referencing_column_names(vec![create_column_name()])
        .set_referenced_column_names(vec![create_column_name()]);
    expect_exception("InvalidForeignKeyConstraintDefinition", editor.create());
}

#[test]
fn testReferencingColumnNamesNotSet() {
    let editor = ForeignKeyConstraint::editor()
        .set_referenced_table_name(create_table_name())
        .set_referenced_column_names(vec![create_column_name()]);
    expect_exception("InvalidForeignKeyConstraintDefinition", editor.create());
}

#[test]
fn testReferencedColumnNamesNotSet() {
    let editor = ForeignKeyConstraint::editor()
        .set_referenced_table_name(create_table_name())
        .set_referencing_column_names(vec![create_column_name()]);
    expect_exception("InvalidForeignKeyConstraintDefinition", editor.create());
}

#[test]
fn testSetName() {
    let editor = create_minimal_valid_editor();
    let constraint = editor.clone().create().unwrap();
    assert!(constraint.get_object_name().unwrap().is_none());

    let name = u("fk_users_id");
    let constraint = editor.set_name(Some(name.clone())).create().unwrap();
    assert_eq!(constraint.get_object_name().unwrap(), Some(name));
}

#[test]
fn testSetUnquotedName() {
    let constraint = create_minimal_valid_editor()
        .set_unquoted_name("fk_users_id")
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_object_name().unwrap(),
        Some(u("fk_users_id"))
    );
}

#[test]
fn testSetQuotedName() {
    let constraint = create_minimal_valid_editor()
        .set_quoted_name("fk_users_id")
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_object_name().unwrap(),
        Some(q("fk_users_id"))
    );
}

#[test]
fn testSetUnquotedReferencingColumnNames() {
    let constraint = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["account_id", "user_id"])
        .set_referenced_table_name(create_table_name())
        .set_unquoted_referenced_column_names(&["unused1", "unused2"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referencing_column_names().unwrap(),
        [u("account_id"), u("user_id")]
    );
}

#[test]
fn testSetQuotedReferencingColumnNames() {
    let constraint = ForeignKeyConstraint::editor()
        .set_quoted_referencing_column_names(&["account_id", "user_id"])
        .set_referenced_table_name(create_table_name())
        .set_quoted_referenced_column_names(&["unused1", "unused2"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referencing_column_names().unwrap(),
        [q("account_id"), q("user_id")]
    );
}

#[test]
fn testSetUnquotedReferencedTableName() {
    let constraint = ForeignKeyConstraint::editor()
        .set_referencing_column_names(vec![create_column_name()])
        .set_unquoted_referenced_table_name("users", Some("public"))
        .set_referenced_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referenced_table_name().unwrap(),
        &OptionallyQualifiedName::unquoted("users", Some("public")).unwrap()
    );
}

#[test]
fn testSetQuotedReferencedTableName() {
    let constraint = ForeignKeyConstraint::editor()
        .set_referencing_column_names(vec![create_column_name()])
        .set_quoted_referenced_table_name("users", Some("public"))
        .set_referenced_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referenced_table_name().unwrap(),
        &OptionallyQualifiedName::quoted("users", Some("public")).unwrap()
    );
}

#[test]
fn testSetUnquotedReferencedColumnNames() {
    let constraint = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["unused1", "unused2"])
        .set_referenced_table_name(create_table_name())
        .set_unquoted_referenced_column_names(&["account_id", "id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referenced_column_names().unwrap(),
        [u("account_id"), u("id")]
    );
}

#[test]
fn testSetQuotedReferencedColumnNames() {
    let constraint = ForeignKeyConstraint::editor()
        .set_quoted_referencing_column_names(&["unused1", "unused2"])
        .set_referenced_table_name(create_table_name())
        .set_quoted_referenced_column_names(&["account_id", "id"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referenced_column_names().unwrap(),
        [q("account_id"), q("id")]
    );
}

#[test]
fn testAddReferencingColumnName() {
    let constraint = ForeignKeyConstraint::editor()
        .add_unquoted_referencing_column_name("account_id")
        .add_quoted_referencing_column_name("user_id")
        .set_referenced_table_name(create_table_name())
        .set_unquoted_referenced_column_names(&["unused1", "unused2"])
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referencing_column_names().unwrap(),
        [u("account_id"), q("user_id")]
    );
}

#[test]
fn testAddReferencedColumnName() {
    let constraint = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["unused1", "unused2"])
        .set_referenced_table_name(create_table_name())
        .add_unquoted_referenced_column_name("account_id")
        .add_quoted_referenced_column_name("id")
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_referenced_column_names().unwrap(),
        [u("account_id"), q("id")]
    );
}

#[test]
fn testSetMatchType() {
    let editor = create_minimal_valid_editor();
    let constraint = editor.clone().create().unwrap();
    assert_eq!(constraint.get_match_type().unwrap(), MatchType::Simple);

    let constraint = editor.set_match_type(MatchType::Full).create().unwrap();
    assert_eq!(constraint.get_match_type().unwrap(), MatchType::Full);
}

#[test]
fn testSetOnUpdateAction() {
    let editor = create_minimal_valid_editor();
    let constraint = editor.clone().create().unwrap();
    assert_eq!(
        constraint.get_on_update_action().unwrap(),
        ReferentialAction::NoAction
    );

    let constraint = editor
        .set_on_update_action(ReferentialAction::Cascade)
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_on_update_action().unwrap(),
        ReferentialAction::Cascade
    );
}

#[test]
fn testSetOnDeleteAction() {
    let editor = create_minimal_valid_editor();
    let constraint = editor.clone().create().unwrap();
    assert_eq!(
        constraint.get_on_delete_action().unwrap(),
        ReferentialAction::NoAction
    );

    let constraint = editor
        .set_on_delete_action(ReferentialAction::Cascade)
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_on_delete_action().unwrap(),
        ReferentialAction::Cascade
    );
}

#[test]
fn testSetDeferrability() {
    let editor = create_minimal_valid_editor();
    let constraint = editor.clone().create().unwrap();
    assert_eq!(
        constraint.get_deferrability().unwrap(),
        Deferrability::NotDeferrable
    );

    let constraint = editor
        .set_deferrability(Deferrability::Deferrable)
        .create()
        .unwrap();
    assert_eq!(
        constraint.get_deferrability().unwrap(),
        Deferrability::Deferrable
    );
}

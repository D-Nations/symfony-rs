//! Port of tests/Schema/TableEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `TableEditor` is a by-value builder that keeps the first
//! error of a modification and returns it from `create()` (PHP throws
//! from the modifying call), so `expectException()` before a modifying
//! call is checked on the `create()` that follows it. `assertSame()` on
//! objects compares them by value.
//!
//! Upstream: doctrine/dbal tests/Schema/TableEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::name::OptionallyQualifiedName;
use doctrine_dbal::schema::{
    Column, ForeignKeyConstraint, Index, PrimaryKeyConstraint, Table, TableEditor, UniqueConstraint,
};
use doctrine_dbal::types::Types;
use schema_support::expect_exception;

fn create_column(name: &str, type_name: &str) -> Column {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
        .create()
        .unwrap()
}

fn accounts(columns: Vec<Column>) -> TableEditor {
    Table::editor()
        .set_unquoted_name("accounts", None)
        .set_columns(columns)
}

fn index(name: &str, columns: &[&str]) -> Index {
    Index::editor()
        .set_unquoted_name(name)
        .set_unquoted_column_names(columns)
        .create()
        .unwrap()
}

fn columns(table: &Table) -> Vec<Column> {
    table.get_columns().into_iter().cloned().collect()
}

fn invalid_modification(editor: TableEditor) {
    expect_exception("InvalidTableModification", editor.create());
}

#[test]
fn testSetUnquotedName() {
    let table = Table::editor()
        .set_unquoted_name("accounts", Some("public"))
        .set_columns(vec![create_column("id", Types::INTEGER)])
        .create()
        .unwrap();
    assert_eq!(
        table.get_object_name().unwrap(),
        OptionallyQualifiedName::unquoted("accounts", Some("public")).unwrap()
    );
}

#[test]
fn testSetQuotedName() {
    let table = Table::editor()
        .set_quoted_name("contacts", Some("dbo"))
        .set_columns(vec![create_column("id", Types::INTEGER)])
        .create()
        .unwrap();
    assert_eq!(
        table.get_object_name().unwrap(),
        OptionallyQualifiedName::quoted("contacts", Some("dbo")).unwrap()
    );
}

#[test]
fn testSetName() {
    let name = OptionallyQualifiedName::unquoted("contacts", None).unwrap();
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .create()
        .unwrap();
    let table = table
        .edit()
        .unwrap()
        .set_name(name.clone())
        .create()
        .unwrap();
    assert_eq!(table.get_object_name().unwrap(), name);
}

#[test]
fn testNameNotSet() {
    expect_exception("InvalidTableDefinition", Table::editor().create());
}

#[test]
fn testColumnsNotSet() {
    let editor = Table::editor().set_unquoted_name("accounts", None);
    expect_exception("InvalidTableDefinition", editor.create());
}

#[test]
fn testAddExistingColumn() {
    let column = create_column("id", Types::INTEGER);
    let editor = accounts(vec![column.clone()]);
    invalid_modification(editor.add_column(column));
}

#[test]
fn testModifyColumn() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .modify_column_by_unquoted_name("id", |editor| editor.set_type_name(Types::BIGINT))
        .create()
        .unwrap();
    assert_eq!(columns(&table), [create_column("id", Types::BIGINT)]);
}

#[test]
fn testModifyNonExistingColumn() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.modify_column_by_unquoted_name("account_id", |editor| editor));
}

#[test]
fn testRenameColumn() {
    let table = accounts(vec![
        create_column("id", Types::INTEGER),
        create_column("username", Types::STRING),
    ])
    .set_indexes(vec![index("idx_username", &["id", "username"])])
    .set_unique_constraints(vec![
        UniqueConstraint::editor()
            .set_unquoted_column_names(&["id", "username"])
            .create()
            .unwrap(),
    ])
    .set_foreign_key_constraints(vec![
        ForeignKeyConstraint::editor()
            .set_unquoted_referencing_column_names(&["id", "username"])
            .set_unquoted_referenced_table_name("users", None)
            .set_unquoted_referenced_column_names(&["id", "username"])
            .create()
            .unwrap(),
    ])
    .set_primary_key_constraint(Some(
        PrimaryKeyConstraint::editor()
            .set_unquoted_column_names(&["id", "username"])
            .create()
            .unwrap(),
    ))
    .rename_column_by_unquoted_name("username", "user_name")
    .create()
    .unwrap();

    assert_eq!(
        columns(&table),
        [
            create_column("id", Types::INTEGER),
            create_column("user_name", Types::STRING),
        ]
    );

    assert_eq!(
        table.get_index("idx_username").unwrap(),
        &index("idx_username", &["id", "user_name"])
    );

    assert_eq!(
        table
            .get_unique_constraints()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [UniqueConstraint::editor()
            .set_unquoted_column_names(&["id", "user_name"])
            .create()
            .unwrap()]
    );

    assert_eq!(
        table
            .get_foreign_keys()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [ForeignKeyConstraint::editor()
            .set_unquoted_referencing_column_names(&["id", "user_name"])
            .set_unquoted_referenced_table_name("users", None)
            .set_unquoted_referenced_column_names(&["id", "username"])
            .create()
            .unwrap()]
    );

    assert_eq!(
        table.get_primary_key_constraint().unwrap(),
        Some(
            &PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&["id", "user_name"])
                .create()
                .unwrap()
        )
    );

    assert_eq!(
        table.get_renamed_columns().iter().collect::<Vec<_>>(),
        [("user_name", &"username".to_owned())]
    );
}

#[test]
fn testRenameColumnToExistingName() {
    let editor = accounts(vec![
        create_column("id", Types::INTEGER),
        create_column("value", Types::STRING),
    ]);
    invalid_modification(editor.rename_column_by_unquoted_name("id", "value"));
}

fn rename_column_unreferenced_by(add_object: impl FnOnce(TableEditor) -> TableEditor) -> Table {
    let editor = accounts(vec![
        create_column("id", Types::INTEGER),
        create_column("username", Types::STRING),
        create_column("city", Types::STRING),
    ]);
    add_object(editor)
        .rename_column_by_unquoted_name("city", "town")
        .create()
        .unwrap()
}

#[test]
fn testRenameColumnDoesNotRebuildUnaffectedIndex() {
    let index = index("idx_username", &["username"]);
    let table = rename_column_unreferenced_by(|editor| editor.set_indexes(vec![index.clone()]));
    assert_eq!(table.get_index("idx_username").unwrap(), &index);
}

#[test]
fn testRenameColumnDoesNotRebuildUnaffectedUniqueConstraint() {
    let unique_constraint = UniqueConstraint::editor()
        .set_unquoted_column_names(&["username"])
        .create()
        .unwrap();
    let table = rename_column_unreferenced_by(|editor| {
        editor.set_unique_constraints(vec![unique_constraint.clone()])
    });
    assert_eq!(
        table
            .get_unique_constraints()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [unique_constraint]
    );
}

#[test]
fn testRenameColumnDoesNotRebuildUnaffectedForeignKeyConstraint() {
    let foreign_key_constraint = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["username"])
        .set_unquoted_referenced_table_name("users", None)
        .set_unquoted_referenced_column_names(&["id"])
        .create()
        .unwrap();
    let table = rename_column_unreferenced_by(|editor| {
        editor.set_foreign_key_constraints(vec![foreign_key_constraint.clone()])
    });
    assert_eq!(
        table
            .get_foreign_keys()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [foreign_key_constraint]
    );
}

#[test]
fn testRenameColumnDoesNotRebuildUnaffectedPrimaryKeyConstraint() {
    let primary_key_constraint = PrimaryKeyConstraint::editor()
        .set_unquoted_column_names(&["id"])
        .create()
        .unwrap();
    let table = rename_column_unreferenced_by(|editor| {
        editor.set_primary_key_constraint(Some(primary_key_constraint.clone()))
    });
    assert_eq!(
        table.get_primary_key_constraint().unwrap(),
        Some(&primary_key_constraint)
    );
}

#[test]
fn testDropColumn() {
    let column1 = create_column("id", Types::INTEGER);
    let column2 = create_column("value", Types::STRING);
    let table = accounts(vec![column1, column2.clone()])
        .drop_column_by_unquoted_name("id")
        .create()
        .unwrap();
    assert_eq!(columns(&table), [column2]);
}

#[test]
fn testDropNonExistingColumn() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.drop_column_by_unquoted_name("account_id"));
}

#[test]
fn testSetIndexes() {
    let index = index("idx_id", &["id"]);
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .set_indexes(vec![index.clone()])
        .create()
        .unwrap();
    assert_eq!(
        table.get_indexes().values().cloned().collect::<Vec<_>>(),
        [index]
    );
}

#[test]
fn testAddExistingIndex() {
    let index = index("idx_id", &["id"]);
    let editor =
        accounts(vec![create_column("id", Types::INTEGER)]).set_indexes(vec![index.clone()]);
    invalid_modification(editor.add_index(index));
}

#[test]
fn testRenameIndex() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .add_index(index("idx_id", &["id"]))
        .rename_index_by_unquoted_name("idx_id", "idx_account_id")
        .create()
        .unwrap();
    assert_eq!(
        table.get_indexes().values().cloned().collect::<Vec<_>>(),
        [index("idx_account_id", &["id"])]
    );
}

#[test]
fn testRenameNonExistingIndex() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.rename_index_by_unquoted_name("idx_id", "idx_account_id"));
}

#[test]
fn testRenameIndexToExistingName() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]).set_indexes(vec![
        index("idx_id", &["id"]),
        index("idx_account_id", &["id"]),
    ]);
    invalid_modification(editor.rename_index_by_unquoted_name("idx_id", "idx_account_id"));
}

#[test]
fn testDropIndex() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .set_indexes(vec![index("idx_id", &["id"])])
        .drop_index_by_unquoted_name("idx_id")
        .create()
        .unwrap();
    assert!(table.get_indexes().is_empty());
}

#[test]
fn testDropNonExistingIndex() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.drop_index_by_unquoted_name("idx_id"));
}

fn pk_id() -> PrimaryKeyConstraint {
    PrimaryKeyConstraint::editor()
        .set_unquoted_column_names(&["id"])
        .create()
        .unwrap()
}

#[test]
fn testAddPrimaryKeyConstraint() {
    let primary_key_constraint = pk_id();
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .add_primary_key_constraint(primary_key_constraint.clone())
        .create()
        .unwrap();
    assert_eq!(
        table.get_primary_key_constraint().unwrap(),
        Some(&primary_key_constraint)
    );
}

#[test]
fn testSetNullPrimaryKeyConstraint() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .set_primary_key_constraint(None)
        .create()
        .unwrap();
    assert!(table.get_primary_key_constraint().unwrap().is_none());
}

#[test]
fn testAddPrimaryKeyConstraintWhenOneAlreadyExists() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)])
        .set_primary_key_constraint(Some(pk_id()));
    invalid_modification(editor.add_primary_key_constraint(pk_id()));
}

#[test]
fn testDropPrimaryKeyConstraint() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .set_primary_key_constraint(Some(pk_id()))
        .drop_primary_key_constraint()
        .create()
        .unwrap();
    assert!(table.get_primary_key_constraint().unwrap().is_none());
}

#[test]
fn testDropNonExistingPrimaryKeyConstraint() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.drop_primary_key_constraint());
}

#[test]
fn testPrimaryKeyConstraintReplacesPrimaryIndex() {
    let mut table = accounts(vec![create_column("id", Types::INTEGER)])
        .create()
        .unwrap();
    table.set_primary_key(&["id"], Some("pk_id")).unwrap();

    assert!(table.get_primary_key_constraint().unwrap().is_some());
    assert!(table.has_index("pk_id"));

    let table = table
        .edit()
        .unwrap()
        .drop_primary_key_constraint()
        .add_primary_key_constraint(pk_id())
        .create()
        .unwrap();
    assert!(!table.has_index("pk_id"));
}

fn uq_accounts_id() -> UniqueConstraint {
    UniqueConstraint::editor()
        .set_unquoted_name("uq_accounts_id")
        .set_unquoted_column_names(&["id"])
        .create()
        .unwrap()
}

#[test]
fn testSetUniqueConstraints() {
    let unique_constraint = UniqueConstraint::editor()
        .set_unquoted_column_names(&["id"])
        .create()
        .unwrap();
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .set_unique_constraints(vec![unique_constraint.clone()])
        .create()
        .unwrap();
    assert_eq!(
        table
            .get_unique_constraints()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [unique_constraint]
    );
}

#[test]
fn testAddExistingUniqueConstraint() {
    let editor =
        accounts(vec![create_column("id", Types::INTEGER)]).add_unique_constraint(uq_accounts_id());
    invalid_modification(editor.add_unique_constraint(uq_accounts_id()));
}

#[test]
fn testDropUniqueConstraint() {
    let table = accounts(vec![create_column("id", Types::INTEGER)])
        .add_unique_constraint(uq_accounts_id())
        .drop_unique_constraint_by_unquoted_name("uq_accounts_id")
        .create()
        .unwrap();
    assert!(table.get_unique_constraints().is_empty());
}

#[test]
fn testDropNonExistingUniqueConstraint() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.drop_unique_constraint_by_unquoted_name("uq_accounts_id"));
}

fn fk_accounts_users() -> ForeignKeyConstraint {
    ForeignKeyConstraint::editor()
        .set_unquoted_name("fk_accounts_users")
        .set_unquoted_referencing_column_names(&["user_id"])
        .set_unquoted_referenced_table_name("users", None)
        .set_unquoted_referenced_column_names(&["id"])
        .create()
        .unwrap()
}

#[test]
fn testSetForeignKeyConstraints() {
    let foreign_key_constraint = fk_accounts_users();
    let table = accounts(vec![
        create_column("id", Types::INTEGER),
        create_column("user_id", Types::INTEGER),
    ])
    .set_foreign_key_constraints(vec![foreign_key_constraint.clone()])
    .create()
    .unwrap();
    assert_eq!(
        table
            .get_foreign_keys()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        [foreign_key_constraint]
    );
}

#[test]
fn testAddExistingForeignKeyConstraint() {
    let editor = accounts(vec![create_column("user_id", Types::INTEGER)])
        .add_foreign_key_constraint(fk_accounts_users());
    invalid_modification(editor.add_foreign_key_constraint(fk_accounts_users()));
}

#[test]
fn testDropForeignKeyConstraint() {
    let table = accounts(vec![create_column("user_id", Types::INTEGER)])
        .add_foreign_key_constraint(fk_accounts_users())
        .drop_foreign_key_constraint_by_unquoted_name("fk_accounts_users")
        .create()
        .unwrap();
    assert!(table.get_foreign_keys().is_empty());
}

#[test]
fn testDropNonExistingForeignKeyConstraint() {
    let editor = accounts(vec![create_column("id", Types::INTEGER)]);
    invalid_modification(editor.drop_foreign_key_constraint_by_unquoted_name("fk_accounts_users"));
}

#[test]
fn testSetComment() {
    let table = Table::editor()
        .set_unquoted_name("accounts", Some("public"))
        .set_columns(vec![create_column("id", Types::INTEGER)])
        .set_comment("This is the \"accounts\" table")
        .create()
        .unwrap();
    assert_eq!(
        table.get_comment().as_deref(),
        Some("This is the \"accounts\" table")
    );
}

//! Port of tests/Schema/IndexEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the `#[TestWith]` rows are written in place.
//!
//! Upstream: doctrine/dbal tests/Schema/IndexEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::name::UnqualifiedName;
use doctrine_dbal::schema::{Index, IndexedColumn};
use php_value::{PhpArray, Value, php_array};
use schema_support::expect_exception;

fn create_name() -> UnqualifiedName {
    UnqualifiedName::unquoted("idx").unwrap()
}

fn create_column_name() -> UnqualifiedName {
    UnqualifiedName::unquoted("id").unwrap()
}

fn column(name: UnqualifiedName, length: Option<i64>) -> IndexedColumn {
    IndexedColumn::new(name, length).unwrap()
}

fn u(name: &str) -> UnqualifiedName {
    UnqualifiedName::unquoted(name).unwrap()
}

fn q(name: &str) -> UnqualifiedName {
    UnqualifiedName::quoted(name).unwrap()
}

#[test]
fn testNameNotSet() {
    let editor = Index::editor().set_unquoted_column_names(&["id"]);
    expect_exception("InvalidIndexDefinition", editor.create());
}

#[test]
fn testColumnsNotSet() {
    let editor = Index::editor().set_unquoted_name("idx_user_id");
    expect_exception("InvalidIndexDefinition", editor.create());
}

#[test]
fn testSetUnquotedName() {
    let index = Index::editor()
        .set_unquoted_name("idx_id")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(index.get_object_name().unwrap(), u("idx_id"));
}

#[test]
fn testSetQuotedName() {
    let index = Index::editor()
        .set_quoted_name("idx_id")
        .set_column_names(vec![create_column_name()])
        .create()
        .unwrap();
    assert_eq!(index.get_object_name().unwrap(), q("idx_id"));
}

#[test]
fn testSetUnquotedColumnNames() {
    let index = Index::editor()
        .set_name(Some(create_name()))
        .set_unquoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        index.get_indexed_columns().unwrap(),
        [column(u("account_id"), None), column(u("user_id"), None)]
    );
}

#[test]
fn testSetQuotedColumnNames() {
    let index = Index::editor()
        .set_name(Some(create_name()))
        .set_quoted_column_names(&["account_id", "user_id"])
        .create()
        .unwrap();
    assert_eq!(
        index.get_indexed_columns().unwrap(),
        [column(q("account_id"), None), column(q("user_id"), None)]
    );
}

#[test]
fn testCreateAppliesColumnLengthToTheCorrectColumn() {
    let user_id = column(u("user_id"), None);
    let last_name = column(u("last_name"), Some(16));
    let index = Index::editor()
        .set_name(Some(create_name()))
        .set_columns(vec![user_id.clone(), last_name.clone()])
        .create()
        .unwrap();
    assert_eq!(index.get_indexed_columns().unwrap(), [user_id, last_name]);
}

#[test]
fn testAddColumn() {
    let index = Index::editor()
        .set_name(Some(create_name()))
        .add_unquoted_column_name("last_name", Some(16))
        .add_quoted_column_name("user_id", None)
        .create()
        .unwrap();
    assert_eq!(
        index.get_indexed_columns().unwrap(),
        [column(u("last_name"), Some(16)), column(q("user_id"), None)]
    );
}

#[test]
fn testPreservesRegularIndexProperties() {
    let options = php_array![
        "lengths" => php_array![0_i64 => 32],
        "where" => "is_active = 1",
    ];
    let index1 = Index::new(
        Some("idx_user_name"),
        ["user_name"],
        false,
        false,
        Vec::<String>::new(),
        options.clone(),
    )
    .unwrap();

    let index2 = index1.edit().unwrap().create().unwrap();

    assert_eq!(index2.get_object_name().unwrap(), u("idx_user_name"));
    assert_eq!(index2.get_columns(), ["user_name".to_owned()]);
    assert!(!index2.is_unique());
    assert!(!index2.is_primary());
    assert_eq!(index2.get_flags(), [] as [std::string::String; 0]);
    assert_eq!(index2.get_options(), &options);
    // assertSame() also compares the types of the values.
    assert_eq!(
        index2.get_options().get(&"where".into()),
        Some(&Value::string("is_active = 1"))
    );
}

#[test]
fn testPreservesIndexFlags() {
    for flags in [["fulltext"], ["spatial"], ["clustered"]] {
        let index1 = Index::new(
            Some("idx_test"),
            ["test"],
            false,
            false,
            flags,
            PhpArray::new(),
        )
        .unwrap();
        let index2 = index1.edit().unwrap().create().unwrap();
        assert_eq!(index2.get_flags(), flags.map(str::to_owned), "{flags:?}");
    }
}

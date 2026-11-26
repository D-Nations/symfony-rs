//! Port of tests/Schema/TableTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the data provider's rows are written in place.
//! `assertSame()` on objects compares them by value (and a registered
//! type by its instance). Tests that need the `MySQL` platform are pending.
//! `TableEditor` returns the first error of a modification from
//! `create()` (PHP throws from the modifying call).
//!
//! Upstream: doctrine/dbal tests/Schema/TableTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use std::sync::Arc;

use doctrine_dbal::schema::name::{Identifier, OptionallyQualifiedName, UnqualifiedName};
use doctrine_dbal::schema::{
    Column, ForeignKeyConstraint, Index, IndexType, PrimaryKeyConstraint, Table, TableEditor,
    UniqueConstraint,
};
use doctrine_dbal::types::{self, Type, Types};
use php_value::{PhpArray, Value, php_array};
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

const SCHEMA_EXCEPTION: &str = "Doctrine\\DBAL\\Schema\\SchemaException";

fn col(name: &str, type_name: &str) -> Column {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
        .create()
        .unwrap()
}

fn editor(name: &str, columns: Vec<Column>) -> TableEditor {
    Table::editor()
        .set_unquoted_name(name, None)
        .set_columns(columns)
}

fn table(name: &str, columns: Vec<Column>) -> Table {
    editor(name, columns).create().unwrap()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn renamed(table: &Table) -> Vec<(String, String)> {
    table
        .get_renamed_columns()
        .iter()
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect()
}

fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn same_type(a: &Arc<dyn Type>, b: &Arc<dyn Type>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

fn no_flags() -> Vec<String> {
    Vec::new()
}

#[test]
fn testCreateWithInvalidTableName() {
    expect_exception("Doctrine\\DBAL\\Exception", Table::new(""));
}

#[test]
fn testGetName() {
    let table = table("foo", vec![col("id", Types::INTEGER)]);
    assert_eq!(
        table.get_object_name().unwrap(),
        OptionallyQualifiedName::unquoted("foo", None).unwrap()
    );
}

#[test]
fn testColumns() {
    let table = table(
        "foo",
        vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)],
    );
    assert!(table.has_column("foo"));
    assert!(table.has_column("bar"));
    assert!(!table.has_column("baz"));
    assert_eq!(
        table.get_column("foo").unwrap().get_object_name().unwrap(),
        UnqualifiedName::unquoted("foo").unwrap()
    );
    assert_eq!(
        table.get_column("foo").unwrap().get_object_name().unwrap(),
        UnqualifiedName::unquoted("foo").unwrap()
    );
    assert_eq!(table.get_columns().len(), 2);
}

#[test]
fn testRenameColumn() {
    let type_txt = types::get_type(Types::TEXT).unwrap();
    let mut table = table("foo", vec![col("foo", Types::STRING)]);

    assert!(!table.has_column("bar"));
    assert!(table.has_column("foo"));

    let column = table.rename_column("foo", "bar").unwrap();
    column.set_type(&type_txt).unwrap();
    assert!(table.has_column("bar"), "Should now have bar column");
    assert!(
        !table.has_column("foo"),
        "Should not have foo column anymore"
    );
    assert_eq!(table.get_columns().len(), 1);

    assert_eq!(renamed(&table), pairs(&[("bar", "foo")]));
    table.rename_column("bar", "baz").unwrap();

    assert!(table.has_column("baz"), "Should now have baz column");
    assert!(
        !table.has_column("bar"),
        "Should not have bar column anymore"
    );
    assert_eq!(renamed(&table), pairs(&[("baz", "foo")]));
    assert_eq!(table.get_columns().len(), 1);
}

#[test]
fn testRenameColumnThroughEditor() {
    let table = table("foo", vec![col("foo", Types::STRING)]);

    let table = table
        .edit()
        .unwrap()
        .rename_column_by_unquoted_name("foo", "bar")
        .create()
        .unwrap();

    assert!(table.has_column("bar"), "Should now have bar column");
    assert!(
        !table.has_column("foo"),
        "Should not have foo column anymore"
    );
    assert_eq!(table.get_columns().len(), 1);
    assert_eq!(renamed(&table), pairs(&[("bar", "foo")]));

    let table = table
        .edit()
        .unwrap()
        .rename_column_by_unquoted_name("bar", "baz")
        .create()
        .unwrap();

    assert!(table.has_column("baz"), "Should now have baz column");
    assert!(
        !table.has_column("bar"),
        "Should not have bar column anymore"
    );
    assert_eq!(renamed(&table), pairs(&[("baz", "bar")]));
    assert_eq!(table.get_columns().len(), 1);
}

#[test]
fn testRenameColumnException() {
    let mut table = table("foo", vec![col("baz", Types::STRING)]);
    let e = expect_exception(
        "LogicException",
        table.rename_column("baz", "`BaZ`").map(|_| ()),
    );
    assert_eq!(
        e.message().to_string_lossy(),
        "Attempt to rename column \"foo.baz\" to the same name."
    );
}

#[test]
fn testRenameColumnLoop() {
    let mut table = table("foo", vec![col("baz", Types::INTEGER)]);
    table.rename_column("baz", "`foo`").unwrap();
    assert_eq!(table.get_renamed_columns().len(), 1);
    table.rename_column("foo", "Baz").unwrap();
    assert_eq!(table.get_columns().len(), 1);
    assert_eq!(table.get_renamed_columns().len(), 0);
}

#[test]
fn testRenameColumnLoopThroughEditor() {
    let table = table("foo", vec![col("baz", Types::INTEGER)]);
    let table = table
        .edit()
        .unwrap()
        .rename_column_by_unquoted_name("baz", "foo")
        .rename_column_by_unquoted_name("foo", "Baz")
        .create()
        .unwrap();
    assert_eq!(table.get_columns().len(), 1);
    assert_eq!(table.get_renamed_columns().len(), 0);
}

#[test]
fn testRenameColumnInIndex() {
    let mut table = editor(
        "t",
        vec![col("c1", Types::INTEGER), col("c2", Types::INTEGER)],
    )
    .set_indexes(vec![
        Index::editor()
            .set_unquoted_name("idx_c1_c2")
            .set_unquoted_column_names(&["c1", "c2"])
            .create()
            .unwrap(),
    ])
    .create()
    .unwrap();

    table.rename_column("c1", "c1a").unwrap();
    assert_eq!(
        table.get_index("idx_c1_c2").unwrap().get_columns(),
        strings(&["c1a", "c2"])
    );
}

#[test]
fn testRenameColumnInForeignKeyConstraint() {
    let mut table = editor(
        "t1",
        vec![col("c1", Types::INTEGER), col("c2", Types::INTEGER)],
    )
    .set_foreign_key_constraints(vec![
        ForeignKeyConstraint::editor()
            .set_unquoted_name("fk_c1_c2")
            .set_unquoted_referencing_column_names(&["c1", "c2"])
            .set_unquoted_referenced_table_name("t2", None)
            .set_unquoted_referenced_column_names(&["c1", "c2"])
            .create()
            .unwrap(),
    ])
    .create()
    .unwrap();

    table.rename_column("c2", "c2a").unwrap();
    assert_eq!(
        table
            .get_foreign_key("fk_c1_c2")
            .unwrap()
            .get_local_columns(),
        strings(&["c1", "c2a"])
    );
}

#[test]
fn testRenameColumnInUniqueConstraint() {
    let mut table = editor(
        "t",
        vec![col("c1", Types::INTEGER), col("c2", Types::INTEGER)],
    )
    .set_unique_constraints(vec![
        UniqueConstraint::editor()
            .set_unquoted_name("uq_c1_c2")
            .set_unquoted_column_names(&["c1", "c2"])
            .create()
            .unwrap(),
    ])
    .create()
    .unwrap();

    table.rename_column("c1", "c1a").unwrap();
    assert_eq!(
        table
            .get_unique_constraint("uq_c1_c2")
            .unwrap()
            .get_columns(),
        strings(&["c1a", "c2"])
    );
}

#[test]
fn testColumnsCaseInsensitive() {
    let table = table("foo", vec![col("Foo", Types::INTEGER)]);
    assert!(table.has_column("Foo"));
    assert!(table.has_column("foo"));
    assert!(table.has_column("FOO"));

    let column = table.get_column("Foo").unwrap();
    assert!(std::ptr::eq(column, table.get_column("foo").unwrap()));
    assert!(std::ptr::eq(column, table.get_column("FOO").unwrap()));
}

#[test]
fn testCreateColumn() {
    let type_ = types::get_type(Types::INTEGER).unwrap();
    let table = table("foo", vec![col("bar", Types::INTEGER)]);
    assert!(table.has_column("bar"));
    assert!(same_type(
        &type_,
        &table.get_column("bar").unwrap().get_type().unwrap()
    ));
}

#[test]
fn testDropColumn() {
    let mut table = table(
        "foo",
        vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)],
    );
    assert!(table.has_column("foo"));
    assert!(table.has_column("bar"));

    table.drop_column("foo").drop_column("bar");

    assert!(!table.has_column("foo"));
    assert!(!table.has_column("bar"));
}

#[test]
fn testDropColumnThroughEditor() {
    let table = table(
        "foo",
        vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)],
    );
    let table = table
        .edit()
        .unwrap()
        .drop_column_by_unquoted_name("foo")
        .create()
        .unwrap();
    assert!(!table.has_column("foo"));
    assert!(table.has_column("bar"));
}

#[test]
fn testGetUnknownColumnThrowsException() {
    let table = table("foo", vec![col("id", Types::INTEGER)]);
    expect_exception(SCHEMA_EXCEPTION, table.get_column("unknown"));
}

#[test]
fn testAddColumnTwiceThrowsException() {
    expect_exception(
        SCHEMA_EXCEPTION,
        editor(
            "foo",
            vec![col("foo", Types::INTEGER), col("foo", Types::INTEGER)],
        )
        .create(),
    );
}

fn three_columns() -> Vec<Column> {
    vec![
        col("foo", Types::INTEGER),
        col("bar", Types::INTEGER),
        col("baz", Types::INTEGER),
    ]
}

fn idx(name: &str, columns: &[&str]) -> Index {
    Index::editor()
        .set_unquoted_name(name)
        .set_unquoted_column_names(columns)
        .create()
        .unwrap()
}

fn unique_idx(name: &str, columns: &[&str]) -> Index {
    Index::editor()
        .set_unquoted_name(name)
        .set_type(IndexType::Unique)
        .set_unquoted_column_names(columns)
        .create()
        .unwrap()
}

/// `new Index($name, $columns, $isUnique, $isPrimary)`.
fn legacy_index(name: &str, columns: &[&str], is_unique: bool, is_primary: bool) -> Index {
    Index::new(
        Some(name),
        columns.iter().copied(),
        is_unique,
        is_primary,
        no_flags(),
        PhpArray::new(),
    )
    .unwrap()
}

#[test]
fn testCreateIndex() {
    let table = editor("foo", three_columns())
        .set_indexes(vec![
            idx("foo_foo_bar_idx", &["foo", "bar"]),
            unique_idx("foo_bar_baz_uniq", &["bar", "baz"]),
        ])
        .create()
        .unwrap();
    assert!(table.has_index("foo_foo_bar_idx"));
    assert!(table.has_index("foo_bar_baz_uniq"));
}

#[test]
fn testIndexCaseInsensitive() {
    let table = editor("foo", three_columns())
        .set_indexes(vec![idx("Foo_Idx", &["foo", "bar", "baz"])])
        .create()
        .unwrap();
    assert!(table.has_index("foo_idx"));
    assert!(table.has_index("Foo_Idx"));
    assert!(table.has_index("FOO_IDX"));
}

#[test]
fn testAddIndexes() {
    let table = editor(
        "foo",
        vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)],
    )
    .set_indexes(vec![
        legacy_index("the_primary", &["foo"], true, true),
        unique_idx("bar_idx", &["bar"]),
    ])
    .create()
    .unwrap();

    assert!(table.has_index("the_primary"));
    assert!(table.has_index("bar_idx"));
    assert!(!table.has_index("some_idx"));

    assert!(table.get_primary_key().is_some());

    assert_eq!(
        table
            .get_index("the_primary")
            .unwrap()
            .get_object_name()
            .unwrap(),
        UnqualifiedName::unquoted("the_primary").unwrap()
    );
    assert_eq!(
        table
            .get_index("bar_idx")
            .unwrap()
            .get_object_name()
            .unwrap(),
        UnqualifiedName::unquoted("bar_idx").unwrap()
    );
}

fn create_table_with_single_column() -> Table {
    table("users", vec![col("id", Types::INTEGER)])
}

#[test]
fn testGetUnknownIndexThrowsException() {
    let table = create_table_with_single_column();
    expect_exception("IndexDoesNotExist", table.get_index("unknown"));
}

#[test]
fn testGetUnknownForeignKeyThrowsException() {
    let table = create_table_with_single_column();
    expect_exception("ForeignKeyDoesNotExist", table.get_foreign_key("unknown"));
}

#[test]
fn testGetUnknownUniqueConstraintThrowsException() {
    let table = create_table_with_single_column();
    expect_exception(
        "UniqueConstraintDoesNotExist",
        table.get_unique_constraint("unknown"),
    );
}

#[test]
fn testAddTwoPrimaryThrowsException() {
    let columns = vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)];
    let indexes = vec![
        legacy_index("the_primary", &["foo"], true, true),
        legacy_index("other_primary", &["bar"], true, true),
    ];
    expect_exception(
        SCHEMA_EXCEPTION,
        editor("foo", columns).set_indexes(indexes).create(),
    );
}

#[test]
fn testAddTwoIndexesWithSameNameThrowsException() {
    let columns = vec![col("foo", Types::INTEGER), col("bar", Types::INTEGER)];
    let indexes = vec![
        legacy_index("an_idx", &["foo"], false, false),
        legacy_index("an_idx", &["bar"], false, false),
    ];
    expect_exception(
        SCHEMA_EXCEPTION,
        editor("foo", columns).set_indexes(indexes).create(),
    );
}

#[test]
fn testOptions() {
    let table = editor("foo", vec![col("id", Types::INTEGER)])
        .set_options(php_array!["foo" => "bar"])
        .create()
        .unwrap();
    assert!(table.has_option("foo"));
    assert_eq!(table.get_option("foo"), Value::string("bar"));
}

#[test]
fn testBuilderSetPrimaryKey() {
    expect_no_deprecation("https://github.com/doctrine/dbal/pull/6787", || {
        let mut table = table("foo", vec![col("bar", Types::INTEGER)]);
        table.set_primary_key(&["bar"], None).unwrap();

        assert!(table.has_index("primary"));
        assert!(table.get_primary_key().is_some());
        assert!(table.get_index("primary").unwrap().is_unique());
        assert!(table.get_index("primary").unwrap().is_primary());
    });
}

#[test]
fn testSetPrimaryKeyOnANullableColumn() {
    let mut table = table(
        "users",
        vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .set_not_null(false)
                .create()
                .unwrap(),
        ],
    );
    expect_deprecation("https://github.com/doctrine/dbal/pull/6787", || {
        table.set_primary_key(&["id"], None).unwrap();
    });
    assert!(table.get_column("id").unwrap().get_notnull());
}

#[test]
fn testBuilderAddUniqueIndex() {
    let table = editor("foo", vec![col("bar", Types::INTEGER)])
        .set_indexes(vec![unique_idx("my_idx", &["bar"])])
        .create()
        .unwrap();
    assert!(table.has_index("my_idx"));
    assert!(table.get_index("my_idx").unwrap().is_unique());
    assert!(!table.get_index("my_idx").unwrap().is_primary());
}

#[test]
fn testBuilderAddIndex() {
    let table = editor("foo", vec![col("bar", Types::INTEGER)])
        .set_indexes(vec![idx("my_idx", &["bar"])])
        .create()
        .unwrap();
    assert!(table.has_index("my_idx"));
    assert!(!table.get_index("my_idx").unwrap().is_unique());
    assert!(!table.get_index("my_idx").unwrap().is_primary());
}

#[test]
fn testBuilderAddIndexWithInvalidNameThrowsException() {
    let mut table = table("foo", vec![col("bar", Types::INTEGER)]);
    expect_exception(
        SCHEMA_EXCEPTION,
        table
            .add_index(
                ["bar"],
                Some("invalid name %&/"),
                no_flags(),
                PhpArray::new(),
            )
            .map(|_| ()),
    );
}

#[test]
fn testBuilderAddIndexWithUnknownColumnThrowsException() {
    let mut table = table("foo", vec![col("id", Types::INTEGER)]);
    expect_exception(
        SCHEMA_EXCEPTION,
        table
            .add_index(["bar"], Some("invalidName"), no_flags(), PhpArray::new())
            .map(|_| ()),
    );
}

#[test]
fn testBuilderOptions() {
    let table = editor("foo", vec![col("id", Types::INTEGER)])
        .set_options(php_array!["foo" => "bar"])
        .create()
        .unwrap();
    assert!(table.has_option("foo"));
    assert_eq!(table.get_option("foo"), Value::string("bar"));
}

#[test]
fn testAddForeignKeyConstraintUnknownLocalColumnThrowsException() {
    let editor = editor("foo", vec![col("id", Types::INTEGER)]).set_foreign_key_constraints(vec![
        ForeignKeyConstraint::editor()
            .set_unquoted_referencing_column_names(&["foo"])
            .set_unquoted_referenced_table_name("bar", None)
            .set_unquoted_referenced_column_names(&["id"])
            .create()
            .unwrap(),
    ]);
    expect_exception(SCHEMA_EXCEPTION, editor.create());
}

#[test]
fn testAddForeignKeyConstraint() {
    let table = editor("foo", vec![col("id", Types::INTEGER)])
        .add_foreign_key_constraint(
            ForeignKeyConstraint::new(["id"], "bar", ["id"], "", php_array!["foo" => "bar"])
                .unwrap(),
        )
        .create()
        .unwrap();

    let constraints = table.get_foreign_keys();
    assert_eq!(constraints.len(), 1);
    let constraint = constraints.values().next().unwrap();

    assert!(constraint.has_option("foo"));
    assert_eq!(constraint.get_option("foo"), Value::string("bar"));
}

#[test]
fn testAddIndexWithCaseSensitiveColumnProblem() {
    let table = editor("foo", vec![col("id", Types::INTEGER)])
        .set_indexes(vec![idx("my_idx", &["ID"])])
        .create()
        .unwrap();
    assert!(table.has_index("my_idx"));
    assert_eq!(
        table.get_index("my_idx").unwrap().get_columns(),
        strings(&["ID"])
    );
    assert!(
        table
            .get_index("my_idx")
            .unwrap()
            .spans_columns(&strings(&["id"]))
    );
}

#[test]
fn testAddPrimaryKeyColumnsAreExplicitlySetToNotNull() {
    let mut table = table(
        "foo",
        vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .set_not_null(false)
                .create()
                .unwrap(),
        ],
    );
    assert!(!table.get_column("id").unwrap().get_notnull());
    table.set_primary_key(&["id"], None).unwrap();
    assert!(table.get_column("id").unwrap().get_notnull());
}

#[test]
fn testAllowImplicitSchemaTableInAutogeneratedIndexNames() {
    let table = Table::editor()
        .set_unquoted_name("bar", Some("foo"))
        .set_columns(vec![col("baz", Types::INTEGER)])
        .add_index(Index::editor().set_unquoted_column_names(&["baz"]))
        .create()
        .unwrap();
    assert_eq!(table.get_indexes().len(), 1);
}

fn indexes(table: &Table) -> Vec<(String, Index)> {
    table
        .get_indexes()
        .iter()
        .map(|(k, v)| (k.to_owned(), v.clone()))
        .collect()
}

fn fk_editor_constraint(local: &[&str], table: &str, foreign: &[&str]) -> ForeignKeyConstraint {
    ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(local)
        .set_unquoted_referenced_table_name(table, None)
        .set_unquoted_referenced_column_names(foreign)
        .create()
        .unwrap()
}

#[test]
fn testAddIndexViaEditorGeneratesSameNamesAsMutators() {
    let columns = vec![col("a", Types::INTEGER), col("b", Types::INTEGER)];

    let via_editor = editor("foo", columns.clone())
        .add_index(Index::editor().set_unquoted_column_names(&["a", "b"]))
        .add_index(
            Index::editor()
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["a", "b"]),
        )
        .create()
        .unwrap();

    let mut via_mutators = table("foo", columns);
    via_mutators
        .add_index(["a", "b"], None, no_flags(), PhpArray::new())
        .unwrap();
    via_mutators
        .add_unique_index(["a", "b"], None, PhpArray::new())
        .unwrap();

    assert_eq!(indexes(&via_mutators), indexes(&via_editor));
}

#[test]
fn testAddIndexEditorWithExplicitNameKeepsIt() {
    let table = editor("foo", vec![col("a", Types::INTEGER)])
        .add_index(
            Index::editor()
                .set_unquoted_name("my_idx")
                .set_unquoted_column_names(&["a"]),
        )
        .create()
        .unwrap();
    assert!(table.has_index("my_idx"));
}

#[test]
fn testAddForeignKeyIndexImplicitly() {
    let table = editor("foo", vec![col("id", Types::INTEGER)])
        .set_foreign_key_constraints(vec![fk_editor_constraint(&["id"], "bar", &["id"])])
        .create()
        .unwrap();

    let indexes = table.get_indexes();
    assert_eq!(indexes.len(), 1);
    let index = indexes.values().next().expect("an index");

    assert!(table.has_index(&index.get_name()));
    assert_eq!(index.get_columns(), strings(&["id"]));
}

#[test]
fn testAddForeignKeyDoesNotCreateDuplicateIndex() {
    let table = editor("foo", vec![col("bar", Types::INTEGER)])
        .set_indexes(vec![idx("bar_idx", &["bar"])])
        .set_foreign_key_constraints(vec![fk_editor_constraint(&["bar"], "foo", &["foo"])])
        .create()
        .unwrap();

    assert_eq!(table.get_indexes().len(), 1);
    assert!(table.has_index("bar_idx"));
    assert_eq!(
        table.get_index("bar_idx").unwrap().get_columns(),
        strings(&["bar"])
    );
}

#[test]
fn testAddForeignKeyAddsImplicitIndexIfIndexColumnsDoNotSpan() {
    let table = editor(
        "foo",
        vec![
            col("bar", Types::INTEGER),
            col("baz", Types::STRING),
            col("bloo", Types::STRING),
        ],
    )
    .set_indexes(vec![
        idx("composite_idx", &["baz", "bar"]),
        idx("full_idx", &["bar", "baz", "bloo"]),
    ])
    .set_foreign_key_constraints(vec![fk_editor_constraint(
        &["bar", "baz"],
        "bar",
        &["foo", "baz"],
    )])
    .create()
    .unwrap();

    assert_eq!(table.get_indexes().len(), 3);
    assert!(table.has_index("composite_idx"));
    assert!(table.has_index("full_idx"));
    assert!(table.has_index("idx_8c73652176ff8caa78240498"));
    assert_eq!(
        table.get_index("composite_idx").unwrap().get_columns(),
        strings(&["baz", "bar"])
    );
    assert_eq!(
        table.get_index("full_idx").unwrap().get_columns(),
        strings(&["bar", "baz", "bloo"])
    );
    assert_eq!(
        table
            .get_index("idx_8c73652176ff8caa78240498")
            .unwrap()
            .get_columns(),
        strings(&["bar", "baz"])
    );
}

#[test]
fn testOverrulingIndexDoesNotDropOverruledIndex() {
    let table = editor("bar", vec![col("baz", Types::INTEGER)])
        .add_index(Index::editor().set_unquoted_column_names(&["baz"]))
        .create()
        .unwrap();

    let indexes = table.get_indexes();
    assert_eq!(indexes.len(), 1);
    let index = indexes.values().next().expect("an index").clone();

    let table = table
        .edit()
        .unwrap()
        .add_index(
            Index::editor()
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["baz"]),
        )
        .create()
        .unwrap();

    assert_eq!(table.get_indexes().len(), 2);
    assert!(table.has_index(&index.get_name()));
}

#[test]
fn testAllowsAddingDuplicateIndexesBasedOnColumns() {
    let table = editor("foo", vec![col("bar", Types::INTEGER)])
        .set_indexes(vec![
            idx("bar_idx", &["bar"]),
            idx("duplicate_idx", &["bar"]),
        ])
        .create()
        .unwrap();

    assert_eq!(table.get_indexes().len(), 2);
    assert!(table.has_index("bar_idx"));
    assert!(table.has_index("duplicate_idx"));
    assert_eq!(
        table.get_index("bar_idx").unwrap().get_columns(),
        strings(&["bar"])
    );
    assert_eq!(
        table.get_index("duplicate_idx").unwrap().get_columns(),
        strings(&["bar"])
    );
}

#[test]
fn testAllowsAddingFulfillingIndexesBasedOnColumns() {
    let table = editor(
        "foo",
        vec![col("bar", Types::INTEGER), col("baz", Types::STRING)],
    )
    .set_indexes(vec![
        idx("bar_idx", &["bar"]),
        idx("fulfilling_idx", &["bar", "baz"]),
    ])
    .create()
    .unwrap();

    assert_eq!(table.get_indexes().len(), 2);
    assert!(table.has_index("bar_idx"));
    assert!(table.has_index("fulfilling_idx"));
    assert_eq!(
        table.get_index("bar_idx").unwrap().get_columns(),
        strings(&["bar"])
    );
    assert_eq!(
        table.get_index("fulfilling_idx").unwrap().get_columns(),
        strings(&["bar", "baz"])
    );
}

#[test]
fn testPrimaryKeyOverrulingUniqueIndexDoesNotDropUniqueIndex() {
    let mut table = editor("bar", vec![col("baz", Types::INTEGER)])
        .set_indexes(vec![unique_idx("idx_unique", &["baz"])])
        .create()
        .unwrap();

    table.set_primary_key(&["baz"], None).unwrap();

    // Table should only contain both the primary key table index and the
    // unique one, even though it was overruled
    assert_eq!(table.get_indexes().len(), 2);
    assert!(table.get_primary_key().is_some());
    assert!(table.has_index("idx_unique"));
}

fn local_table_with_foreign_key() -> Table {
    editor("local", vec![col("id", Types::INTEGER)])
        .set_foreign_key_constraints(vec![fk_editor_constraint(&["id"], "foreign", &["id"])])
        .create()
        .unwrap()
}

#[test]
fn testAddingFulfillingRegularIndexOverridesImplicitForeignKeyConstraintIndex() {
    let local_table = local_table_with_foreign_key();
    assert_eq!(local_table.get_indexes().len(), 1);

    let local_table = local_table
        .edit()
        .unwrap()
        .add_index(idx("explicit_idx", &["id"]))
        .create()
        .unwrap();

    assert_eq!(local_table.get_indexes().len(), 1);
    assert!(local_table.has_index("explicit_idx"));
}

#[test]
fn testAddingFulfillingUniqueIndexOverridesImplicitForeignKeyConstraintIndex() {
    let local_table = local_table_with_foreign_key();
    assert_eq!(local_table.get_indexes().len(), 1);

    let local_table = local_table
        .edit()
        .unwrap()
        .add_index(unique_idx("explicit_idx", &["id"]))
        .create()
        .unwrap();

    assert_eq!(local_table.get_indexes().len(), 1);
    assert!(local_table.has_index("explicit_idx"));
}

#[test]
fn testAddingFulfillingPrimaryKeyOverridesImplicitForeignKeyConstraintIndex() {
    let mut local_table = table("local", vec![col("id", Types::INTEGER)]);

    local_table
        .add_foreign_key_constraint("foreign", &["id"], &["id"], PhpArray::new(), None)
        .unwrap();

    assert_eq!(local_table.get_indexes().len(), 1);

    local_table
        .set_primary_key(&["id"], Some("explicit_idx"))
        .unwrap();

    assert_eq!(local_table.get_indexes().len(), 1);
    assert!(local_table.has_index("explicit_idx"));
}

// testAddingFulfillingExplicitIndexOverridingImplicitForeignKeyConstraintIndexWithSameName:
// pending. Its last assertion, `assertNotSame($implicitIndex, ...)`, tells
// the explicit index from the implicit one by object identity; here the
// two are equal values.

// testQuotedTableName: pending (needs the `MySQL` platform).

#[test]
fn testTableHasPrimaryKey() {
    let mut table = table("test", vec![col("foo", Types::INTEGER)]);
    assert!(table.get_primary_key().is_none());
    table.set_primary_key(&["foo"], None).unwrap();
    assert!(table.get_primary_key().is_some());
}

#[test]
fn testAddIndexWithQuotedColumns() {
    let table = editor(
        "test",
        vec![
            Column::editor()
                .set_quoted_name("foo")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
            col("bar", Types::INTEGER),
        ],
    )
    .add_index(Index::editor().set_quoted_column_names(&["foo", "bar"]))
    .create()
    .unwrap();
    assert!(table.columns_are_indexed(&strings(&["\"foo\"", "\"bar\""])));
}

#[test]
fn testAddForeignKeyWithQuotedColumnsAndTable() {
    let table = editor(
        "foo",
        vec![
            Column::editor()
                .set_quoted_name("foo")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
            col("bar", Types::INTEGER),
        ],
    )
    .add_foreign_key_constraint(
        ForeignKeyConstraint::editor()
            .set_quoted_referencing_column_names(&["foo", "bar"])
            .set_quoted_referenced_table_name("boing", None)
            .set_unquoted_referenced_column_names(&["id"])
            .create()
            .unwrap(),
    )
    .create()
    .unwrap();
    assert_eq!(table.get_foreign_keys().len(), 1);
}

// testQuoteSchemaPrefixed: pending (needs the `MySQL` platform).

#[test]
fn testDropIndex() {
    let mut table = table("test", vec![col("id", Types::INTEGER)]);
    table
        .add_index(["id"], Some("idx"), no_flags(), PhpArray::new())
        .unwrap();
    assert!(table.has_index("idx"));
    table.drop_index("idx").unwrap();
    assert!(!table.has_index("idx"));
}

#[test]
fn testDropIndexThroughEditor() {
    let table = editor("test", vec![col("id", Types::INTEGER)])
        .set_indexes(vec![idx("idx", &["id"])])
        .create()
        .unwrap();
    assert!(table.has_index("idx"));
    let table = table
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("idx")
        .create()
        .unwrap();
    assert!(!table.has_index("idx"));
}

fn pk(columns: &[&str]) -> PrimaryKeyConstraint {
    PrimaryKeyConstraint::editor()
        .set_unquoted_column_names(columns)
        .create()
        .unwrap()
}

#[test]
fn testDropPrimaryKey() {
    let mut table = editor("test", vec![col("id", Types::INTEGER)])
        .set_primary_key_constraint(Some(pk(&["id"])))
        .create()
        .unwrap();
    assert!(table.get_primary_key().is_some());
    table.drop_primary_key().unwrap();
    assert!(table.get_primary_key().is_none());
}

#[test]
fn testDropPrimaryKeyConstraintThroughEditor() {
    let table = editor("test", vec![col("id", Types::INTEGER)])
        .set_primary_key_constraint(Some(pk(&["id"])))
        .create()
        .unwrap();
    assert!(table.get_primary_key().is_some());
    let table = table
        .edit()
        .unwrap()
        .drop_primary_key_constraint()
        .create()
        .unwrap();
    assert!(table.get_primary_key().is_none());
}

/// `self::assertSame($table, $table->renameIndex($old, $new))`.
fn rename_index_returns_table(table: &mut Table, old_name: &str, new_name: Option<&str>) {
    let expected: *const Table = &raw const *table;
    let returned = table.rename_index(old_name, new_name).unwrap();
    assert!(std::ptr::eq(returned, expected));
}

#[test]
fn testRenameIndex() {
    let mut table = editor(
        "test",
        vec![
            col("id", Types::INTEGER),
            col("foo", Types::INTEGER),
            col("bar", Types::INTEGER),
            col("baz", Types::INTEGER),
        ],
    )
    .set_primary_key_constraint(Some(
        PrimaryKeyConstraint::editor()
            .set_unquoted_name("pk")
            .set_unquoted_column_names(&["id"])
            .create()
            .unwrap(),
    ))
    .create()
    .unwrap();

    table
        .add_index(["foo"], Some("idx"), strings(&["flag"]), PhpArray::new())
        .unwrap();
    table
        .add_unique_index(["bar", "baz"], Some("uniq"), PhpArray::new())
        .unwrap();

    // Rename to custom name.
    rename_index_returns_table(&mut table, "pk", Some("pk_new"));
    rename_index_returns_table(&mut table, "idx", Some("idx_new"));
    rename_index_returns_table(&mut table, "uniq", Some("uniq_new"));

    assert!(table.get_primary_key().is_some());
    assert!(table.has_index("pk_new"));
    assert!(table.has_index("idx_new"));
    assert!(table.has_index("uniq_new"));

    assert!(!table.has_index("pk"));
    assert!(!table.has_index("idx"));
    assert!(!table.has_index("uniq"));

    let flagged = |name: &str| {
        Index::new(Some(name), ["foo"], false, false, ["flag"], PhpArray::new()).unwrap()
    };

    assert_eq!(
        table.get_primary_key(),
        Some(&legacy_index("pk_new", &["id"], true, true))
    );
    assert_eq!(
        table.get_index("pk_new").unwrap(),
        &legacy_index("pk_new", &["id"], true, true)
    );
    assert_eq!(table.get_index("idx_new").unwrap(), &flagged("idx_new"));
    assert_eq!(
        table.get_index("uniq_new").unwrap(),
        &legacy_index("uniq_new", &["bar", "baz"], true, false)
    );

    // Rename to auto-generated name.
    rename_index_returns_table(&mut table, "pk_new", None);
    rename_index_returns_table(&mut table, "idx_new", None);
    rename_index_returns_table(&mut table, "uniq_new", None);

    assert!(table.get_primary_key().is_some());
    assert!(table.has_index("primary"));
    assert!(table.has_index("IDX_D87F7E0C8C736521"));
    assert!(table.has_index("UNIQ_D87F7E0C76FF8CAA78240498"));

    assert!(!table.has_index("pk_new"));
    assert!(!table.has_index("idx_new"));
    assert!(!table.has_index("uniq_new"));

    assert_eq!(
        table.get_primary_key(),
        Some(&legacy_index("primary", &["id"], true, true))
    );
    assert_eq!(
        table.get_index("primary").unwrap(),
        &legacy_index("primary", &["id"], true, true)
    );
    assert_eq!(
        table.get_index("IDX_D87F7E0C8C736521").unwrap(),
        &flagged("IDX_D87F7E0C8C736521")
    );
    assert_eq!(
        table.get_index("UNIQ_D87F7E0C76FF8CAA78240498").unwrap(),
        &legacy_index(
            "UNIQ_D87F7E0C76FF8CAA78240498",
            &["bar", "baz"],
            true,
            false
        )
    );

    // Rename to same name (changed case).
    rename_index_returns_table(&mut table, "primary", Some("PRIMARY"));
    rename_index_returns_table(
        &mut table,
        "IDX_D87F7E0C8C736521",
        Some("idx_D87F7E0C8C736521"),
    );
    rename_index_returns_table(
        &mut table,
        "UNIQ_D87F7E0C76FF8CAA78240498",
        Some("uniq_D87F7E0C76FF8CAA78240498"),
    );

    assert!(table.get_primary_key().is_some());
    assert!(table.has_index("primary"));
    assert!(table.has_index("IDX_D87F7E0C8C736521"));
    assert!(table.has_index("UNIQ_D87F7E0C76FF8CAA78240498"));
}

#[test]
fn testRenameIndexThroughEditor() {
    let table = editor("test", vec![col("id", Types::INTEGER)])
        .set_indexes(vec![idx("idx", &["id"])])
        .create()
        .unwrap();
    let table = table
        .edit()
        .unwrap()
        .rename_index_by_unquoted_name("idx", "idx_new")
        .create()
        .unwrap();
    assert!(!table.has_index("idx"));
    assert!(table.has_index("idx_new"));
}

#[test]
fn testRenameNonExistingIndexToTheSameName() {
    let mut table = table("test", vec![col("id", Types::INTEGER)]);
    expect_exception(
        "IndexDoesNotExist",
        table.rename_index("test", Some("test")).map(|_| ()),
    );
}

#[test]
fn testKeepsIndexOptionsOnRenamingRegularIndex() {
    let mut table = table("foo", vec![col("id", Types::INTEGER)]);
    table
        .add_index(
            ["id"],
            Some("idx_bar"),
            no_flags(),
            php_array!["where" => "1 = 1"],
        )
        .unwrap();
    table.rename_index("idx_bar", Some("idx_baz")).unwrap();
    assert_eq!(
        table.get_index("idx_baz").unwrap().get_options(),
        &php_array!["where" => "1 = 1"]
    );
}

#[test]
fn testKeepsIndexOptionsOnRenamingUniqueIndex() {
    let mut table = table("foo", vec![col("id", Types::INTEGER)]);
    table
        .add_unique_index(["id"], Some("idx_bar"), php_array!["where" => "1 = 1"])
        .unwrap();
    table.rename_index("idx_bar", Some("idx_baz")).unwrap();
    assert_eq!(
        table.get_index("idx_baz").unwrap().get_options(),
        &php_array!["where" => "1 = 1"]
    );
}

#[test]
fn testThrowsExceptionOnRenamingNonExistingIndex() {
    let mut table = table("test", vec![col("id", Types::INTEGER)]);
    table
        .add_index(["id"], Some("idx"), no_flags(), PhpArray::new())
        .unwrap();
    expect_exception(
        SCHEMA_EXCEPTION,
        table.rename_index("foo", Some("bar")).map(|_| ()),
    );
}

#[test]
fn testThrowsExceptionOnRenamingToAlreadyExistingIndex() {
    let mut table = table(
        "test",
        vec![col("id", Types::INTEGER), col("foo", Types::INTEGER)],
    );
    table
        .add_index(["id"], Some("idx_id"), no_flags(), PhpArray::new())
        .unwrap();
    table
        .add_index(["foo"], Some("idx_foo"), no_flags(), PhpArray::new())
        .unwrap();
    expect_exception(
        SCHEMA_EXCEPTION,
        table.rename_index("idx_id", Some("idx_foo")).map(|_| ()),
    );
}

#[test]
fn testNormalizesColumnNames() {
    for asset_name in [
        "foo", "FOO", "`foo`", "`FOO`", "\"foo\"", "\"FOO\"", "\"foo\"", "\"FOO\"",
    ] {
        let mut table = Table::new("test").unwrap();

        table
            .add_column(asset_name, Types::INTEGER, &PhpArray::new())
            .unwrap();
        table
            .add_index([asset_name], Some(asset_name), no_flags(), PhpArray::new())
            .unwrap();
        table
            .add_foreign_key_constraint(
                "test",
                &[asset_name],
                &[asset_name],
                PhpArray::new(),
                Some(asset_name),
            )
            .unwrap();

        assert!(table.has_column(asset_name), "{asset_name}");
        assert!(table.has_column("foo"), "{asset_name}");

        assert!(table.has_index(asset_name), "{asset_name}");
        assert!(table.has_index("foo"), "{asset_name}");

        assert!(table.has_foreign_key(asset_name), "{asset_name}");
        assert!(table.has_foreign_key("foo"), "{asset_name}");

        table.rename_index(asset_name, Some(asset_name)).unwrap();
        assert!(table.has_index(asset_name), "{asset_name}");
        assert!(table.has_index("foo"), "{asset_name}");

        table.rename_index(asset_name, Some("foo")).unwrap();
        assert!(table.has_index(asset_name), "{asset_name}");
        assert!(table.has_index("foo"), "{asset_name}");

        table.rename_index("foo", Some(asset_name)).unwrap();
        assert!(table.has_index(asset_name), "{asset_name}");
        assert!(table.has_index("foo"), "{asset_name}");

        table.rename_index(asset_name, Some("bar")).unwrap();
        assert!(!table.has_index(asset_name), "{asset_name}");
        assert!(!table.has_index("foo"), "{asset_name}");
        assert!(table.has_index("bar"), "{asset_name}");

        table.rename_index("bar", Some(asset_name)).unwrap();

        table.drop_column(asset_name);
        table.drop_index(asset_name).unwrap();
        table.remove_foreign_key(asset_name).unwrap();

        assert!(!table.has_column(asset_name), "{asset_name}");
        assert!(!table.has_column("foo"), "{asset_name}");
        assert!(!table.has_index(asset_name), "{asset_name}");
        assert!(!table.has_index("foo"), "{asset_name}");
        assert!(!table.has_foreign_key(asset_name), "{asset_name}");
        assert!(!table.has_foreign_key("foo"), "{asset_name}");
    }
}

#[test]
fn testTableComment() {
    let mut table = table("bar", vec![col("id", Types::INTEGER)]);
    assert!(table.get_comment().is_none());
    table.set_comment("foo");
    assert_eq!(table.get_comment().as_deref(), Some("foo"));
}

#[test]
fn testUniqueConstraintWithEmptyName() {
    let columns = vec![
        col("column1", Types::STRING),
        col("column2", Types::STRING),
        col("column3", Types::STRING),
        col("column4", Types::STRING),
    ];
    let unique_constraints = vec![
        UniqueConstraint::new("", ["column1", "column2"], no_flags(), PhpArray::new()),
        UniqueConstraint::new("", ["column3", "column4"], no_flags(), PhpArray::new()),
    ];

    let table = editor("test", columns)
        .set_unique_constraints(unique_constraints.clone())
        .create()
        .unwrap();

    let constraints = table.get_unique_constraints();
    assert_eq!(constraints.len(), 2);

    let constraint_names: Vec<&str> = constraints.keys().collect();
    assert_eq!(constraint_names[0], "fk_d87f7e0c341ce00bad15b1b1");
    assert_eq!(constraint_names[1], "fk_d87f7e0cda12812744761484");

    assert_eq!(
        constraints.get("fk_d87f7e0c341ce00bad15b1b1"),
        Some(&unique_constraints[0])
    );
    assert_eq!(
        constraints.get("fk_d87f7e0cda12812744761484"),
        Some(&unique_constraints[1])
    );
}

#[test]
fn testRemoveUniqueConstraint() {
    let mut table = editor("foo", vec![col("bar", Types::INTEGER)])
        .set_unique_constraints(vec![
            UniqueConstraint::editor()
                .set_unquoted_name("unique_constraint")
                .set_unquoted_column_names(&["bar"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();
    table.remove_unique_constraint("unique_constraint").unwrap();
    assert!(!table.has_unique_constraint("unique_constraint"));
}

#[test]
fn testRemoveUniqueConstraintUnknownNameThrowsException() {
    let mut table = table("foo", vec![col("bar", Types::INTEGER)]);
    expect_exception(
        SCHEMA_EXCEPTION,
        table.remove_unique_constraint("unique_constraint"),
    );
}

#[test]
fn testDropColumnWithForeignKeyConstraint() {
    let mut table = table("t1", vec![col("id", Types::INTEGER)]);
    table
        .add_foreign_key_constraint("t2", &["id"], &["id"], PhpArray::new(), None)
        .unwrap();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6559", || {
        table.drop_column("id");
    });
}

#[test]
fn testDropColumnWithUniqueConstraint() {
    let mut table = editor("t", vec![col("id", Types::INTEGER)])
        .set_unique_constraints(vec![
            UniqueConstraint::editor()
                .set_unquoted_column_names(&["id"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6559", || {
        table.drop_column("id");
    });
}

#[test]
fn testDropColumnWithoutConstraints() {
    let mut table = table("t", vec![col("id", Types::INTEGER)]);
    expect_no_deprecation("https://github.com/doctrine/dbal/pull/6559", || {
        table.drop_column("id");
    });
}

#[test]
fn testGetUnqualifiedObjectName() {
    let table = Table::new("products").unwrap();
    let name = table.get_object_name().unwrap();
    assert_eq!(
        name.get_unqualified_name(),
        &Identifier::unquoted("products").unwrap()
    );
    assert!(name.get_qualifier().is_none());
}

#[test]
fn testGetQualifiedObjectName() {
    let table = Table::new("inventory.products").unwrap();
    let name = table.get_object_name().unwrap();
    assert_eq!(
        name.get_unqualified_name(),
        &Identifier::unquoted("products").unwrap()
    );
    assert_eq!(
        name.get_qualifier(),
        Some(&Identifier::unquoted("inventory").unwrap())
    );
}

#[test]
fn testPrimaryKeyConstraintIsDerivedFromIndex() {
    let mut table = table("t", vec![col("id", Types::INTEGER)]);
    table.set_primary_key(&["id"], None).unwrap();
    assert_eq!(
        table.get_primary_key_constraint().unwrap(),
        Some(&pk(&["id"]))
    );
}

#[test]
fn testIndexIsDerivedFromPrimaryKeyConstraint() {
    let table = editor("t", vec![col("id", Types::INTEGER)])
        .set_primary_key_constraint(Some(pk(&["id"])))
        .create()
        .unwrap();
    assert_eq!(
        table.get_primary_key(),
        Some(&legacy_index("primary", &["id"], false, true))
    );
}

#[test]
fn testDroppingIndexDropsPrimaryKeyConstraint() {
    let mut table = table("t", vec![col("id", Types::INTEGER)]);
    table.set_primary_key(&["id"], None).unwrap();
    assert!(table.get_primary_key_constraint().unwrap().is_some());
    table.drop_primary_key().unwrap();
    assert!(table.get_primary_key_constraint().unwrap().is_none());
}

#[test]
fn testCannotAddIndexToExistingPrimaryKeyConstraint() {
    let mut table = table("t", vec![col("id", Types::INTEGER)]);
    table.add_primary_key_constraint(pk(&["id"])).unwrap();
    expect_exception(
        "PrimaryKeyAlreadyExists",
        table.set_primary_key(&["id"], None).map(|_| ()),
    );
}

#[test]
fn testCannotAddPrimaryKeyConstrainToExistingIndex() {
    let mut table = table("t", vec![col("id", Types::INTEGER)]);
    table.set_primary_key(&["id"], None).unwrap();
    expect_exception(
        "PrimaryKeyAlreadyExists",
        table.add_primary_key_constraint(pk(&["id"])).map(|_| ()),
    );
}

#[test]
fn testInvalidPrimaryKeyIndex() {
    let mut table = table(
        "t",
        vec![
            Column::editor()
                .set_quoted_name("id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ],
    );
    expect_deprecation("https://github.com/doctrine/dbal/pull/6787", || {
        table.set_primary_key(&["\"id"], None).unwrap();
    });
    assert!(table.get_primary_key().is_some());
    expect_exception("InvalidState", table.get_primary_key_constraint());
}

#[test]
fn testOverwritingForeignKeyConstraint() {
    let mut table = table("foo", vec![col("id", Types::INTEGER)]);
    table
        .add_foreign_key_constraint("bar", &["id"], &["id"], PhpArray::new(), None)
        .unwrap();
    expect_deprecation("https://github.com/doctrine/dbal/pull/7125", || {
        table
            .add_foreign_key_constraint("baz", &["id"], &["id"], PhpArray::new(), None)
            .unwrap();
    });
}

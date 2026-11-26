//! Port of tests/Platforms/SQLitePlatformTest.php with the tests it
//! inherits from tests/Platforms/AbstractPlatformTestCase.php, at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. The test names
//! are PHPUnit's; `SQLitePlatformTest`'s overridden expectations
//! (`getGenerateTableSql()`, ...) are written in place. The data
//! providers' rows are the recorded ones (tests/data/php.json).
//!
//! Upstream: doctrine/dbal tests/Platforms/SQLitePlatformTest.php
//! Upstream: doctrine/dbal tests/Platforms/AbstractPlatformTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{array, data, provider, text};
use doctrine_dbal::deprecations;
use doctrine_dbal::platforms::{AbstractPlatform, SQLitePlatform, TrimMode};
use doctrine_dbal::schema::name::{Identifier, OptionallyQualifiedName, UnqualifiedName};
use doctrine_dbal::schema::{
    Column, ColumnDiff, ColumnEditor, CurrentDate, CurrentTimestamp, Deferrability,
    ForeignKeyConstraint, Index, IndexType, PrimaryKeyConstraint, Table, TableDiff, TableDiffParts,
    TypeObject, UniqueConstraint,
};
use doctrine_dbal::types::{self, Type, TypeArgument, Types};
use doctrine_dbal::{Exception, TransactionIsolationLevel};
use php_value::{PhpArray, PhpString, Value, php_array};

fn platform() -> SQLitePlatform {
    SQLitePlatform::new()
}

/// `Column::editor()->setUnquotedName($name)->setTypeName($type)`.
fn column(name: &str, type_name: &str) -> ColumnEditor {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the argument of set_primary_key_constraint()"
)]
fn pk(columns: &[&str]) -> Option<PrimaryKeyConstraint> {
    Some(
        PrimaryKeyConstraint::editor()
            .set_unquoted_column_names(columns)
            .create()
            .unwrap(),
    )
}

/// `$this->expectDeprecationWithIdentifier($link)` around `f`.
fn expect_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert!(
        deprecations::count(link) > before,
        "Expected deprecation with identifier \"{link}\" was not triggered"
    );
    deprecations::disable();
    result
}

/// `$this->expectNoDeprecationWithIdentifier($link)` around `f`.
fn expect_no_deprecation<T>(link: &str, f: impl FnOnce() -> T) -> T {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(link);
    let result = f();
    assert_eq!(
        deprecations::count(link),
        before,
        "Unexpected deprecation with identifier \"{link}\" was triggered"
    );
    deprecations::disable();
    result
}

fn sql(text: &str) -> PhpString {
    PhpString::from(text)
}

fn strings(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|s| (*s).to_owned()).collect()
}

// AbstractPlatformTestCase

#[test]
fn testQuoteIdentifier() {
    assert_eq!(
        platform().quote_identifier("test.test"),
        "\"test\".\"test\""
    );
}

#[test]
fn testReturnsForeignKeyReferentialActionSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testReturnsForeignKeyReferentialActionSQL",
    ) {
        assert_eq!(
            platform()
                .get_foreign_key_referential_action_sql(&text(&args[0]))
                .unwrap(),
            text(&args[1]),
            "{name}"
        );
    }
}

#[test]
fn testGetInvalidForeignKeyReferentialActionSQL() {
    let e = platform()
        .get_foreign_key_referential_action_sql("unknown")
        .unwrap_err();
    assert!(e.is_instance_of("InvalidArgumentException"), "{e}");
}

#[test]
fn testGetUnknownDoctrineMappingType() {
    let e = platform().get_doctrine_type_mapping("foobar").unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testRegisterDoctrineMappingType() {
    let platform = platform();
    platform
        .register_doctrine_type_mapping("foo", Types::INTEGER)
        .unwrap();
    assert_eq!(
        platform.get_doctrine_type_mapping("foo").unwrap(),
        Types::INTEGER
    );
}

#[derive(Debug)]
struct TestType;

impl Type for TestType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\Type@anonymous"
    }

    fn get_mapped_database_types(&self, _platform: &dyn AbstractPlatform) -> Vec<PhpString> {
        vec![sql("TESTTYPE")]
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_decimal_type_declaration_sql(column)
    }
}

#[test]
fn testCaseInsensitiveDoctrineTypeMappingFromType() {
    let type_: Arc<dyn Type> = Arc::new(TestType);
    if types::has_type("testtype") {
        types::override_type("testtype", TypeArgument::Instance(type_)).unwrap();
    } else {
        types::add_type("testtype", TypeArgument::Instance(type_)).unwrap();
    }
    assert_eq!(
        platform().get_doctrine_type_mapping("TeStTyPe").unwrap(),
        "testtype"
    );
}

#[test]
fn testRegisterUnknownDoctrineMappingType() {
    let e = platform()
        .register_doctrine_type_mapping("foo", "bar")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testCreateWithNoColumns() {
    let table = Table::new("test").unwrap();
    let e = platform().get_create_table_sql(&table).unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testGeneratesTableCreationSql() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("test", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap();

    let sql = platform().get_create_table_sql(&table).unwrap();
    assert_eq!(
        sql[0],
        "CREATE TABLE test (id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL, test VARCHAR(255) DEFAULT NULL)"
    );
}

#[test]
fn testGenerateTableWithMultiColumnUniqueIndex() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("foo", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
            column("bar", Types::STRING)
                .set_length(Some(255))
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .add_index(
            Index::editor()
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["foo", "bar"]),
        )
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE test (foo VARCHAR(255) DEFAULT NULL, bar VARCHAR(255) DEFAULT NULL)",
            "CREATE UNIQUE INDEX UNIQ_D87F7E0C8C73652176FF8CAA ON test (foo, bar)",
        ])
    );
}

fn index(name: &str, columns: &[&str], is_unique: bool, is_primary: bool) -> Index {
    Index::new(
        Some(name),
        columns.iter().copied(),
        is_unique,
        is_primary,
        Vec::<String>::new(),
        PhpArray::new(),
    )
    .unwrap()
}

#[test]
fn testGeneratesIndexCreationSql() {
    let index_def = index("my_idx", &["user_name", "last_login"], false, false);
    assert_eq!(
        platform()
            .get_create_index_sql(&index_def, "mytable")
            .unwrap(),
        "CREATE INDEX my_idx ON mytable (user_name, last_login)"
    );
}

#[test]
fn testGeneratesUniqueIndexCreationSql() {
    let index_def = index("index_name", &["test", "test2"], true, false);
    assert_eq!(
        platform().get_create_index_sql(&index_def, "test").unwrap(),
        "CREATE UNIQUE INDEX index_name ON test (test, test2)"
    );
}

#[test]
fn testGeneratesPartialIndexesSqlOnlyWhenSupportingPartialIndexes() {
    let platform = platform();
    let where_ = "test IS NULL AND test2 IS NOT NULL";
    let index_def = Index::new(
        Some("name"),
        ["test", "test2"],
        false,
        false,
        Vec::<String>::new(),
        php_array!["where" => where_],
    )
    .unwrap();
    let unique_constraint = UniqueConstraint::editor()
        .set_unquoted_name("name")
        .set_unquoted_column_names(&["test", "test2"])
        .create()
        .unwrap();

    let expected = format!(" WHERE {where_}");

    // supportsInlineIndexDeclaration() is true
    let mut indexes = vec![platform.get_index_declaration_sql(&index_def).unwrap()];

    let unique_constraint_sql = platform
        .get_unique_constraint_declaration_sql(&unique_constraint)
        .unwrap();
    assert!(
        !unique_constraint_sql.ends_with(&expected),
        "WHERE clause should NOT be present"
    );

    indexes.push(platform.get_create_index_sql(&index_def, "table").unwrap());

    for index in indexes {
        if platform.supports_partial_indexes() {
            assert!(index.ends_with(&expected), "WHERE clause should be present");
        } else {
            assert!(
                !index.ends_with(&expected),
                "WHERE clause should NOT be present"
            );
        }
    }
}

#[test]
fn testGeneratesForeignKeyCreationSql() {
    // SQLitePlatformTest: $this->expectException(Exception::class)
    let fk = ForeignKeyConstraint::new(["fk_name_id"], "other_table", ["id"], "", PhpArray::new())
        .unwrap();
    let e = platform()
        .get_create_foreign_key_sql(&fk, "test")
        .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testGeneratesBitAndComparisonExpressionSql() {
    assert_eq!(
        platform().get_bit_and_comparison_expression("2", "4"),
        "(2 & 4)"
    );
}

#[test]
fn testGeneratesBitOrComparisonExpressionSql() {
    assert_eq!(
        platform().get_bit_or_comparison_expression("2", "4"),
        "(2 | 4)"
    );
}

#[test]
fn testGetCustomColumnDeclarationSql() {
    assert_eq!(
        platform()
            .get_column_declaration_sql(
                "foo",
                &php_array!["columnDefinition" => "MEDIUMINT(6) UNSIGNED"]
            )
            .unwrap(),
        "foo MEDIUMINT(6) UNSIGNED"
    );
}

#[test]
fn testGetDefaultValueDeclarationSQL() {
    // non-timestamp value will get single quotes
    assert_eq!(
        platform()
            .get_default_value_declaration_sql(&php_array![
                "typeName" => Types::STRING,
                "default" => "non_timestamp",
            ])
            .unwrap(),
        " DEFAULT 'non_timestamp'"
    );
}

#[test]
fn testGetDefaultValueDeclarationSQLDateTime() {
    let platform = platform();
    // timestamps on datetime types should not be quoted
    for type_ in [
        Types::DATETIME_MUTABLE,
        Types::DATETIMETZ_MUTABLE,
        Types::DATETIME_IMMUTABLE,
        Types::DATETIMETZ_IMMUTABLE,
    ] {
        let mut column = php_array!["typeName" => type_];
        column.insert("default", CurrentTimestamp::value());
        assert_eq!(
            platform.get_default_value_declaration_sql(&column).unwrap(),
            format!(" DEFAULT {}", platform.get_current_timestamp_sql())
        );
    }
}

#[test]
fn testGetDefaultValueDeclarationSQLForIntegerTypes() {
    for type_ in [Types::BIGINT, Types::INTEGER, Types::SMALLINT] {
        assert_eq!(
            platform()
                .get_default_value_declaration_sql(&php_array!["typeName" => type_, "default" => 1])
                .unwrap(),
            " DEFAULT 1"
        );
    }
}

#[test]
fn testGetDefaultValueDeclarationSQLForDateType() {
    let platform = platform();
    let current_date_sql = platform.get_current_date_sql();
    for type_ in [Types::DATE_MUTABLE, Types::DATE_IMMUTABLE] {
        let mut column = php_array!["typeName" => type_];
        column.insert("default", CurrentDate::value());
        assert_eq!(
            platform.get_default_value_declaration_sql(&column).unwrap(),
            format!(" DEFAULT {current_date_sql}")
        );
    }
}

#[test]
fn testGetDefaultValueDeclarationSQLWithLegacyTypeKey() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        let mut column = PhpArray::new();
        column.insert(
            "type",
            TypeObject::value(types::get_type(Types::INTEGER).unwrap()),
        );
        column.insert("default", Value::Int(1));
        assert_eq!(
            platform()
                .get_default_value_declaration_sql(&column)
                .unwrap(),
            " DEFAULT 1"
        );
    });
}

#[test]
fn testGetColumnDeclarationListSQLWithLegacyTypeKey() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        let platform = platform();
        let expected = platform
            .get_column_declaration_list_sql(&[php_array![
                "name" => "id",
                "notnull" => true,
                "typeName" => Types::INTEGER,
            ]])
            .unwrap();
        let mut column = php_array!["name" => "id", "notnull" => true];
        column.insert(
            "type",
            TypeObject::value(types::get_type(Types::INTEGER).unwrap()),
        );
        assert_eq!(
            platform.get_column_declaration_list_sql(&[column]).unwrap(),
            expected
        );
    });
}

#[test]
fn testGetColumnDeclarationSQLWithTypeNameDoesNotTriggerDeprecation() {
    expect_no_deprecation("https://github.com/doctrine/dbal/pull/7490", || {
        platform()
            .get_column_declaration_sql(
                "id",
                &php_array!["name" => "id", "typeName" => Types::INTEGER],
            )
            .unwrap();
    });
}

#[test]
fn testGetColumnDeclarationSQLWithoutType() {
    let e = platform()
        .get_column_declaration_sql("id", &php_array!["name" => "id"])
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::INVALID_COLUMN_DECLARATION),
        "{e}"
    );
    assert!(
        e.message()
            .to_string_lossy()
            .contains("Column \"id\" has no type."),
        "{e}"
    );
}

#[test]
fn testKeywordList() {
    let keyword_list = platform().get_reserved_keywords_list();
    assert!(keyword_list.is_keyword("table"));
}

#[test]
fn testQuotedColumnInPrimaryKeyPropagation() {
    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            column("create", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(pk(&["create"]))
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE \"quoted\" (\"create\" VARCHAR(255) NOT NULL, PRIMARY KEY (\"create\"))"
        ])
    );
}

#[test]
fn testQuotedColumnInIndexPropagation() {
    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            column("create", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .add_index(Index::editor().set_unquoted_column_names(&["create"]))
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE \"quoted\" (\"create\" VARCHAR(255) NOT NULL)",
            "CREATE INDEX IDX_22660D028FD6E0FB ON \"quoted\" (\"create\")",
        ])
    );
}

#[test]
fn testQuotedNameInIndexSQL() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            column("column1", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
        ])
        .set_indexes(vec![
            Index::editor()
                .set_quoted_name("key")
                .set_unquoted_column_names(&["column1"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE test (column1 VARCHAR(255) NOT NULL)",
            "CREATE INDEX \"key\" ON test (column1)",
        ])
    );
}

#[test]
fn testQuotedColumnInForeignKeyPropagation() {
    let referencing_column_names = vec![
        UnqualifiedName::unquoted("create").unwrap(),
        UnqualifiedName::unquoted("foo").unwrap(),
        UnqualifiedName::quoted("bar").unwrap(),
    ];
    let referenced_column_names = vec![
        UnqualifiedName::unquoted("create").unwrap(),
        UnqualifiedName::unquoted("bar").unwrap(),
        UnqualifiedName::quoted("foo-bar").unwrap(),
    ];

    let string_column = |editor: ColumnEditor| {
        editor
            .set_type_name(Types::STRING)
            .set_length(Some(255))
            .create()
            .unwrap()
    };

    let table = Table::editor()
        .set_quoted_name("quoted", None)
        .set_columns(vec![
            string_column(Column::editor().set_unquoted_name("create")),
            string_column(Column::editor().set_unquoted_name("foo")),
            string_column(Column::editor().set_quoted_name("bar")),
        ])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_RESERVED_KEYWORD")
                .set_referencing_column_names(referencing_column_names.clone())
                .set_unquoted_referenced_table_name("foreign", None)
                .set_referenced_column_names(referenced_column_names.clone())
                .create()
                .unwrap(),
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_NON_RESERVED_KEYWORD")
                .set_referencing_column_names(referencing_column_names.clone())
                .set_unquoted_referenced_table_name("foo", None)
                .set_referenced_column_names(referenced_column_names.clone())
                .create()
                .unwrap(),
            ForeignKeyConstraint::editor()
                .set_unquoted_name("FK_WITH_INTENDED_QUOTATION")
                .set_referencing_column_names(referencing_column_names)
                .set_quoted_referenced_table_name("foo-bar", None)
                .set_referenced_column_names(referenced_column_names)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE \"quoted\" (\
             \"create\" VARCHAR(255) NOT NULL, foo VARCHAR(255) NOT NULL, \"bar\" VARCHAR(255) NOT NULL, \
             CONSTRAINT FK_WITH_RESERVED_KEYWORD FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES \"foreign\" (\"create\", bar, \"foo-bar\") NOT DEFERRABLE INITIALLY IMMEDIATE, \
             CONSTRAINT FK_WITH_NON_RESERVED_KEYWORD FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES foo (\"create\", bar, \"foo-bar\") NOT DEFERRABLE INITIALLY IMMEDIATE, \
             CONSTRAINT FK_WITH_INTENDED_QUOTATION FOREIGN KEY (\"create\", foo, \"bar\") \
             REFERENCES \"foo-bar\" (\"create\", bar, \"foo-bar\") NOT DEFERRABLE INITIALLY IMMEDIATE)",
            "CREATE INDEX IDX_22660D028FD6E0FB8C7365216D704F76 ON \"quoted\" (\"create\", foo, \"bar\")",
        ])
    );
}

#[test]
fn testQuotesReservedKeywordInUniqueConstraintDeclarationSQL() {
    let constraint =
        UniqueConstraint::new("select", ["foo"], Vec::<String>::new(), PhpArray::new());
    assert_eq!(
        platform()
            .get_unique_constraint_declaration_sql(&constraint)
            .unwrap(),
        "CONSTRAINT \"select\" UNIQUE (foo)"
    );
}

#[test]
fn testQuotesReservedKeywordInTruncateTableSQL() {
    assert_eq!(
        platform().get_truncate_table_sql("select", false),
        "DELETE FROM \"select\""
    );
}

#[test]
fn testQuotesReservedKeywordInIndexDeclarationSQL() {
    let index = index("select", &["foo"], false, false);
    assert_eq!(
        platform().get_index_declaration_sql(&index).unwrap(),
        "INDEX \"select\" (foo)"
    );
}

#[test]
fn testSupportsCommentOnStatement() {
    assert!(!platform().supports_comment_on_statement());
}

#[test]
fn testGetCreateSchemaSQL() {
    let e = platform().get_create_schema_sql("schema").unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

#[test]
fn testAlterTableChangeQuotedColumn() {
    let platform = platform();
    let table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![column("select", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        table.clone(),
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                table.get_column("select").unwrap().clone(),
                column("select", Types::STRING)
                    .set_length(Some(255))
                    .create()
                    .unwrap(),
            )],
            ..TableDiffParts::default()
        },
    );

    assert!(
        platform
            .get_alter_table_sql(&table_diff)
            .unwrap()
            .join(";")
            .contains(&platform.quote_single_identifier("select"))
    );
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLNoLength() {
    assert_eq!(
        platform()
            .get_string_type_declaration_sql(&php_array!["fixed" => true])
            .unwrap(),
        sql("CHAR")
    );
}

#[test]
fn testGetFixedLengthStringTypeDeclarationSQLWithLength() {
    assert_eq!(
        platform()
            .get_string_type_declaration_sql(&php_array!["fixed" => true, "length" => 16])
            .unwrap(),
        sql("CHAR(16)")
    );
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLNoLength() {
    // SQLitePlatform's VARCHAR snippet takes no length.
    assert_eq!(
        platform()
            .get_string_type_declaration_sql(&php_array!["name" => "email"])
            .unwrap(),
        sql("VARCHAR")
    );
}

#[test]
fn testGetVariableLengthStringTypeDeclarationSQLWithLength() {
    assert_eq!(
        platform()
            .get_string_type_declaration_sql(&php_array!["length" => 16])
            .unwrap(),
        sql("VARCHAR(16)")
    );
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLNoLength() {
    assert_eq!(
        platform()
            .get_binary_type_declaration_sql(&php_array!["name" => "checksum", "fixed" => true])
            .unwrap(),
        sql("BLOB")
    );
}

#[test]
fn testGetFixedLengthBinaryTypeDeclarationSQLWithLength() {
    assert_eq!(
        platform()
            .get_binary_type_declaration_sql(&php_array!["fixed" => true, "length" => 16])
            .unwrap(),
        sql("BLOB")
    );
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLNoLength() {
    assert_eq!(
        platform()
            .get_binary_type_declaration_sql(&php_array!["name" => "attachment"])
            .unwrap(),
        sql("BLOB")
    );
}

#[test]
fn testGetVariableLengthBinaryTypeDeclarationSQLWithLength() {
    assert_eq!(
        platform()
            .get_binary_type_declaration_sql(&php_array!["length" => 16])
            .unwrap(),
        sql("BLOB")
    );
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoPrecision() {
    let e = platform()
        .get_decimal_type_declaration_sql(&php_array!["name" => "price", "scale" => 2])
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::INVALID_COLUMN_DECLARATION),
        "{e}"
    );
}

#[test]
fn testGetDecimalTypeDeclarationSQLNoScale() {
    let e = platform()
        .get_decimal_type_declaration_sql(&php_array!["name" => "price", "precision" => 10])
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::INVALID_COLUMN_DECLARATION),
        "{e}"
    );
}

#[test]
fn testReturnsJsonTypeDeclarationSQL() {
    let platform = platform();
    let mut column = php_array!["length" => 666, "notnull" => true];
    column.insert(
        "type",
        TypeObject::value(types::get_type(Types::JSON).unwrap()),
    );
    assert_eq!(
        platform.get_json_type_declaration_sql(&column).unwrap(),
        platform.get_clob_type_declaration_sql(&column).unwrap()
    );
}

#[test]
fn testReturnsJsonbTypeDeclarationSQL() {
    let platform = platform();
    expect_deprecation("https://github.com/doctrine/dbal/pull/6939", || {
        assert_eq!(
            platform
                .get_json_type_declaration_sql(&php_array!["jsonb" => true])
                .unwrap(),
            platform
                .get_jsonb_type_declaration_sql(&PhpArray::new())
                .unwrap()
        );
    });
}

fn mytable_with_id(name: &str, qualifier: Option<&str>) -> Table {
    Table::editor()
        .set_unquoted_name(name, qualifier)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap()
}

fn renamed(pairs: &[(&str, &str)]) -> TableDiffParts {
    TableDiffParts {
        renamed_indexes: pairs
            .iter()
            .map(|(old, new)| ((*old).to_owned(), index(new, &["id"], false, false)))
            .collect(),
        ..TableDiffParts::default()
    }
}

#[test]
fn testAlterTableRenameIndex() {
    let table = mytable_with_id("mytable", None);
    let table_diff = TableDiff::new(table, renamed(&[("idx_foo", "idx_bar")]));

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__mytable AS SELECT id FROM mytable",
            "DROP TABLE mytable",
            "CREATE TABLE mytable (id INTEGER NOT NULL, PRIMARY KEY (id))",
            "INSERT INTO mytable (id) SELECT id FROM __temp__mytable",
            "DROP TABLE __temp__mytable",
            "CREATE INDEX idx_bar ON mytable (id)",
        ])
    );
}

#[test]
fn testQuotesAlterTableRenameIndex() {
    let table = mytable_with_id("table", None);
    let table_diff = TableDiff::new(table, renamed(&[("create", "select"), ("`foo`", "`bar`")]));

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__table AS SELECT id FROM \"table\"",
            "DROP TABLE \"table\"",
            "CREATE TABLE \"table\" (id INTEGER NOT NULL, PRIMARY KEY (id))",
            "INSERT INTO \"table\" (id) SELECT id FROM __temp__table",
            "DROP TABLE __temp__table",
            "CREATE INDEX \"select\" ON \"table\" (id)",
            "CREATE INDEX \"bar\" ON \"table\" (id)",
        ])
    );
}

#[test]
#[ignore = "markTestIncomplete upstream: SQLitePlatform::getAlterTable() is broken when used with schemas"]
fn testAlterTableRenameIndexInSchema() {
    let table = mytable_with_id("mytable", Some("myschema"));
    let table_diff = TableDiff::new(table, renamed(&[("idx_foo", "idx_bar")]));
    let _ = platform().get_alter_table_sql(&table_diff);
}

#[test]
#[ignore = "markTestIncomplete upstream: SQLitePlatform::getAlterTable() is broken when used with schemas"]
fn testQuotesAlterTableRenameIndexInSchema() {
    let table = Table::editor()
        .set_name(OptionallyQualifiedName::new(
            Identifier::unquoted("table").unwrap(),
            Some(Identifier::quoted("schema").unwrap()),
        ))
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(pk(&["id"]))
        .create()
        .unwrap();
    let table_diff = TableDiff::new(table, renamed(&[("create", "select"), ("`foo`", "`bar`")]));
    let _ = platform().get_alter_table_sql(&table_diff);
}

#[test]
fn testGetCommentOnColumnSQLWithoutQuoteCharacter() {
    assert_eq!(
        platform().get_comment_on_column_sql("mytable", "id", "This is a comment"),
        "COMMENT ON COLUMN mytable.id IS 'This is a comment'"
    );
}

#[test]
fn testGetCommentOnColumnSQLWithQuoteCharacter() {
    assert_eq!(
        platform().get_comment_on_column_sql("mytable", "id", "It's a quote !"),
        "COMMENT ON COLUMN mytable.id IS 'It''s a quote !'"
    );
}

#[test]
fn testGetCommentOnColumnSQL() {
    let platform = platform();
    assert_eq!(
        vec![
            // regular identifiers
            platform.get_comment_on_column_sql("foo", "bar", "comment"),
            // explicitly quoted identifiers
            platform.get_comment_on_column_sql("`Foo`", "`BAR`", "comment"),
            // reserved keyword identifiers
            platform.get_comment_on_column_sql("select", "from", "comment"),
        ],
        strings(&[
            "COMMENT ON COLUMN foo.bar IS 'comment'",
            "COMMENT ON COLUMN \"Foo\".\"BAR\" IS 'comment'",
            "COMMENT ON COLUMN \"select\".\"from\" IS 'comment'",
        ])
    );
}

#[test]
fn testGeneratesInlineColumnCommentSQL() {
    let platform = platform();
    assert!(platform.supports_inline_column_comments());
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGeneratesInlineColumnCommentSQL",
    ) {
        assert_eq!(
            platform
                .get_inline_column_comment_sql(&text(&args[0]))
                .unwrap(),
            text(&args[1]),
            "{name}"
        );
    }
}

#[test]
#[ignore = "markTestSkipped upstream: SQLitePlatform supports inline column comments"]
fn testThrowsExceptionOnGeneratingInlineColumnCommentSQLIfUnsupported() {
    assert!(!platform().supports_inline_column_comments());
}

#[test]
fn testQuoteStringLiteral() {
    let platform = platform();
    assert_eq!(platform.quote_string_literal("No quote"), "'No quote'");
    assert_eq!(
        platform.quote_string_literal("It's a quote"),
        "'It''s a quote'"
    );
    assert_eq!(platform.quote_string_literal("'"), "''''");
}

#[test]
fn testReturnsGuidTypeDeclarationSQL() {
    // SQLitePlatformTest overrides it.
    assert_eq!(
        platform()
            .get_guid_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("CHAR(36)")
    );
}

#[test]
fn testAlterStringToFixedString() {
    let table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![
            column("name", Types::STRING)
                .set_length(Some(2))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        table.clone(),
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                table.get_column("name").unwrap().clone(),
                column("name", Types::STRING)
                    .set_fixed(true)
                    .set_length(Some(2))
                    .create()
                    .unwrap(),
            )],
            ..TableDiffParts::default()
        },
    );

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__mytable AS SELECT name FROM mytable",
            "DROP TABLE mytable",
            "CREATE TABLE mytable (name CHAR(2) NOT NULL)",
            "INSERT INTO mytable (name) SELECT name FROM __temp__mytable",
            "DROP TABLE __temp__mytable",
        ])
    );
}

#[test]
fn testGeneratesAlterTableRenameIndexUsedByForeignKeySQL() {
    let fk = |name: &str, column: &str| {
        ForeignKeyConstraint::editor()
            .set_unquoted_name(name)
            .set_unquoted_referencing_column_names(&[column])
            .set_unquoted_referenced_table_name("foreign_table", None)
            .set_unquoted_referenced_column_names(&["id"])
            .create()
            .unwrap()
    };
    let idx = |name: &str, column: &str| {
        Index::editor()
            .set_unquoted_name(name)
            .set_unquoted_column_names(&[column])
            .create()
            .unwrap()
    };
    let primary_table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![
            column("foo", Types::INTEGER).create().unwrap(),
            column("bar", Types::INTEGER).create().unwrap(),
            column("baz", Types::INTEGER).create().unwrap(),
        ])
        .set_indexes(vec![idx("idx_foo", "foo"), idx("idx_bar", "bar")])
        .set_foreign_key_constraints(vec![fk("fk_foo", "foo"), fk("fk_bar", "bar")])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        primary_table,
        TableDiffParts {
            renamed_indexes: vec![(
                "idx_foo".to_owned(),
                index("idx_foo_renamed", &["foo"], false, false),
            )],
            ..TableDiffParts::default()
        },
    );

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__mytable AS SELECT foo, bar, baz FROM mytable",
            "DROP TABLE mytable",
            "CREATE TABLE mytable (foo INTEGER NOT NULL, bar INTEGER NOT NULL, baz INTEGER NOT NULL, \
             CONSTRAINT fk_foo FOREIGN KEY (foo) REFERENCES foreign_table (id) \
             NOT DEFERRABLE INITIALLY IMMEDIATE, \
             CONSTRAINT fk_bar FOREIGN KEY (bar) REFERENCES foreign_table (id) \
             NOT DEFERRABLE INITIALLY IMMEDIATE)",
            "INSERT INTO mytable (foo, bar, baz) SELECT foo, bar, baz FROM __temp__mytable",
            "DROP TABLE __temp__mytable",
            "CREATE INDEX idx_bar ON mytable (bar)",
            "CREATE INDEX idx_foo_renamed ON mytable (foo)",
        ])
    );
}

#[test]
fn testGeneratesDecimalTypeDeclarationSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGeneratesDecimalTypeDeclarationSQL",
    ) {
        assert_eq!(
            platform()
                .get_decimal_type_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

#[test]
fn testGeneratesFloatDeclarationSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGeneratesFloatDeclarationSQL",
    ) {
        assert_eq!(
            platform()
                .get_float_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

#[test]
fn testGeneratesSmallFloatDeclarationSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGeneratesSmallFloatDeclarationSQL",
    ) {
        assert_eq!(
            platform()
                .get_small_float_declaration_sql(&array(&args[0]))
                .unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

#[test]
fn testItEscapesStringsForLike() {
    assert_eq!(
        platform().escape_string_for_like("_25% off_ your next purchase \\o/", "\\"),
        "\\_25\\% off\\_ your next purchase \\\\o/"
    );
}

#[test]
fn testZeroOffsetWithoutLimitIsIgnored() {
    let query = "SELECT * FROM user";
    assert_eq!(
        platform().modify_limit_query(query, None, 0).unwrap(),
        query
    );
}

#[test]
fn testAsciiSQLDeclaration() {
    for (name, args) in provider("Platforms/SQLitePlatformTest", "testAsciiSQLDeclaration") {
        assert_eq!(
            platform()
                .get_ascii_string_type_declaration_sql(&array(&args[1]))
                .unwrap(),
            sql(&text(&args[0])),
            "{name}"
        );
    }
}

#[test]
fn testGetEnumDeclarationSQL() {
    for (name, args) in provider("Platforms/SQLitePlatformTest", "testGetEnumDeclarationSQL") {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        assert_eq!(
            platform().get_enum_declaration_sql(&column).unwrap(),
            sql(&text(&args[1])),
            "{name}"
        );
    }
}

#[test]
fn testGetEnumDeclarationWithLengthSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGetEnumDeclarationWithLengthSQL",
    ) {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        column.insert("length", php_value::vectors::decode(&args[1]).unwrap());
        assert_eq!(
            platform().get_enum_declaration_sql(&column).unwrap(),
            sql(&text(&args[2])),
            "{name}"
        );
    }
}

#[test]
fn testGetEnumDeclarationExceptionWithLengthSQL() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGetEnumDeclarationExceptionWithLengthSQL",
    ) {
        let mut column = PhpArray::new();
        column.insert("values", Value::Array(array(&args[0])));
        column.insert("length", php_value::vectors::decode(&args[1]).unwrap());
        let e = platform().get_enum_declaration_sql(&column).unwrap_err();
        assert!(e.is_instance_of("InvalidArgumentException"), "{name}: {e}");
    }
}

#[test]
fn testGetEnumDeclarationSQLWithInvalidValues() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testGetEnumDeclarationSQLWithInvalidValues",
    ) {
        let e = platform()
            .get_enum_declaration_sql(&array(&args[0]))
            .unwrap_err();
        assert!(
            e.is_instance_of(Exception::COLUMN_VALUES_REQUIRED),
            "{name}: {e}"
        );
    }
}

// SQLitePlatformTest

#[test]
fn testGeneratesSqlSnippets() {
    let platform = platform();
    assert_eq!(platform.get_regexp_expression().unwrap(), "REGEXP");
    assert_eq!(
        platform.get_substring_expression("column", "5", None),
        "SUBSTR(column, 5)"
    );
    assert_eq!(
        platform.get_substring_expression("column", "0", Some("5")),
        "SUBSTR(column, 0, 5)"
    );
}

#[test]
fn testGeneratesTransactionCommands() {
    let platform = platform();
    for (level, expected) in [
        (
            TransactionIsolationLevel::ReadUncommitted,
            "PRAGMA read_uncommitted = 0",
        ),
        (
            TransactionIsolationLevel::ReadCommitted,
            "PRAGMA read_uncommitted = 1",
        ),
        (
            TransactionIsolationLevel::RepeatableRead,
            "PRAGMA read_uncommitted = 1",
        ),
        (
            TransactionIsolationLevel::Serializable,
            "PRAGMA read_uncommitted = 1",
        ),
    ] {
        assert_eq!(
            platform.get_set_transaction_isolation_sql(level).unwrap(),
            expected
        );
    }
}

#[test]
fn testIgnoresUnsignedIntegerDeclarationForAutoIncrementalIntegers() {
    assert_eq!(
        platform()
            .get_integer_type_declaration_sql(
                &php_array!["autoincrement" => true, "unsigned" => true]
            )
            .unwrap(),
        sql("INTEGER PRIMARY KEY AUTOINCREMENT")
    );
}

#[test]
fn testGeneratesTypeDeclarationForSmallIntegers() {
    let platform = platform();
    let cases = [
        (PhpArray::new(), "SMALLINT"),
        (
            php_array!["autoincrement" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (
            php_array!["autoincrement" => true, "primary" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (php_array!["unsigned" => false], "SMALLINT"),
        (php_array!["unsigned" => true], "SMALLINT UNSIGNED"),
    ];
    for (column, expected) in cases {
        assert_eq!(
            platform
                .get_small_int_type_declaration_sql(&column)
                .unwrap(),
            sql(expected)
        );
    }
}

#[test]
fn testGeneratesTypeDeclarationForIntegers() {
    let platform = platform();
    let cases = [
        (PhpArray::new(), "INTEGER"),
        (
            php_array!["autoincrement" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (
            php_array!["autoincrement" => true, "unsigned" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (
            php_array!["autoincrement" => true, "primary" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (php_array!["unsigned" => false], "INTEGER"),
        (php_array!["unsigned" => true], "INTEGER UNSIGNED"),
    ];
    for (column, expected) in cases {
        assert_eq!(
            platform.get_integer_type_declaration_sql(&column).unwrap(),
            sql(expected)
        );
    }
}

#[test]
fn testGeneratesTypeDeclarationForBigIntegers() {
    let platform = platform();
    let cases = [
        (PhpArray::new(), "BIGINT"),
        (
            php_array!["autoincrement" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (
            php_array!["autoincrement" => true, "unsigned" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (
            php_array!["autoincrement" => true, "primary" => true],
            "INTEGER PRIMARY KEY AUTOINCREMENT",
        ),
        (php_array!["unsigned" => false], "BIGINT"),
        (php_array!["unsigned" => true], "BIGINT UNSIGNED"),
    ];
    for (column, expected) in cases {
        assert_eq!(
            platform.get_big_int_type_declaration_sql(&column).unwrap(),
            sql(expected)
        );
    }
}

#[test]
fn testGeneratesIndexCreationSqlWithSchema() {
    let index_def = index("i", &["a", "b"], false, false);
    assert_eq!(
        platform()
            .get_create_index_sql(&index_def, "main.mytable")
            .unwrap(),
        "CREATE INDEX main.i ON mytable (a, b)"
    );
}

/// `new class () extends SqlitePlatform { getCreatePrimaryKeySQL() }`.
struct PrimaryKeyTestPlatform(SQLitePlatform);

impl AbstractPlatform for PrimaryKeyTestPlatform {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Platforms\\SQLitePlatform@anonymous"
    }
    fn get_boolean_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_boolean_type_declaration_sql(c)
    }
    fn get_integer_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_integer_type_declaration_sql(c)
    }
    fn get_big_int_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_big_int_type_declaration_sql(c)
    }
    fn get_small_int_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_small_int_type_declaration_sql(c)
    }
    fn get_clob_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_clob_type_declaration_sql(c)
    }
    fn get_blob_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_blob_type_declaration_sql(c)
    }
    fn get_date_time_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_date_time_type_declaration_sql(c)
    }
    fn get_date_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_date_type_declaration_sql(c)
    }
    fn get_time_type_declaration_sql(&self, c: &PhpArray) -> Result<PhpString, Exception> {
        self.0.get_time_type_declaration_sql(c)
    }
    fn get_create_index_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        // SQLitePlatform::getCreateIndexSQL() up to the primary key branch.
        if index.is_primary() {
            return self.get_create_primary_key_sql(index, table);
        }
        self.0.get_create_index_sql(index, table)
    }
    fn get_create_primary_key_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        Ok(format!(
            "TEST: {table}, {} - {}",
            index.get_object_name()?,
            index.get_columns().join(", ")
        ))
    }
}

#[test]
fn testGeneratesPrimaryIndexCreationSqlWithSchema() {
    let primary_index_def = index("i2", &["a", "b"], false, true);
    assert_eq!(
        PrimaryKeyTestPlatform(platform())
            .get_create_index_sql(&primary_index_def, "main.mytable")
            .unwrap(),
        "TEST: main.mytable, i2 - a, b"
    );
}

#[test]
fn testModifyLimitQuery() {
    assert_eq!(
        platform()
            .modify_limit_query("SELECT * FROM user", Some(10), 0)
            .unwrap(),
        "SELECT * FROM user LIMIT 10"
    );
}

#[test]
fn testModifyLimitQueryWithEmptyOffset() {
    assert_eq!(
        platform()
            .modify_limit_query("SELECT * FROM user", Some(10), 0)
            .unwrap(),
        "SELECT * FROM user LIMIT 10"
    );
}

#[test]
fn testModifyLimitQueryWithOffsetAndEmptyLimit() {
    assert_eq!(
        platform()
            .modify_limit_query("SELECT * FROM user", None, 10)
            .unwrap(),
        "SELECT * FROM user LIMIT -1 OFFSET 10"
    );
}

#[test]
fn testGenerateTableSqlShouldNotAutoQuotePrimaryKey() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            Column::editor()
                .set_quoted_name("like")
                .set_type_name(Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(Some(
            PrimaryKeyConstraint::editor()
                .set_quoted_column_names(&["like"])
                .create()
                .unwrap(),
        ))
        .create()
        .unwrap();

    let create_table_sql = platform().get_create_table_sql(&table).unwrap();
    assert_eq!(
        create_table_sql[0],
        "CREATE TABLE test (\"like\" INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL)"
    );
}

#[test]
fn testRenameNonExistingColumn() {
    let table = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        table,
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                column("data", Types::STRING).create().unwrap(),
                column("value", Types::STRING).create().unwrap(),
            )],
            ..TableDiffParts::default()
        },
    );

    let e = platform().get_alter_table_sql(&table_diff).unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{e}");
}

fn deferred_fk(column: &str, table: &str, deferrability: Deferrability) -> ForeignKeyConstraint {
    ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&[column])
        .set_unquoted_referenced_table_name(table, None)
        .set_unquoted_referenced_column_names(&["id"])
        .set_deferrability(deferrability)
        .create()
        .unwrap()
}

fn user_columns() -> Vec<Column> {
    ["id", "article", "post", "parent"]
        .iter()
        .map(|name| column(name, Types::INTEGER).create().unwrap())
        .collect()
}

fn user_foreign_keys() -> Vec<ForeignKeyConstraint> {
    vec![
        deferred_fk("article", "article", Deferrability::Deferrable),
        deferred_fk("post", "post", Deferrability::Deferred),
        deferred_fk("parent", "user", Deferrability::Deferred),
    ]
}

#[test]
fn testCreateTableWithDeferredForeignKeys() {
    let table = Table::editor()
        .set_unquoted_name("user", None)
        .set_columns(user_columns())
        .set_primary_key_constraint(pk(&["id"]))
        .set_foreign_key_constraints(user_foreign_keys())
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&[
            "CREATE TABLE user (\
             id INTEGER NOT NULL, article INTEGER NOT NULL, post INTEGER NOT NULL, parent INTEGER NOT NULL\
             , PRIMARY KEY (id)\
             , FOREIGN KEY (article) REFERENCES article (id) DEFERRABLE INITIALLY IMMEDIATE\
             , FOREIGN KEY (post) REFERENCES post (id) DEFERRABLE INITIALLY DEFERRED\
             , FOREIGN KEY (parent) REFERENCES user (id) DEFERRABLE INITIALLY DEFERRED\
             )",
            "CREATE INDEX IDX_8D93D64923A0E66 ON user (article)",
            "CREATE INDEX IDX_8D93D6495A8A6C8D ON user (post)",
            "CREATE INDEX IDX_8D93D6493D8E604F ON user (parent)",
        ])
    );
}

#[test]
fn testAlterTable() {
    let table = Table::editor()
        .set_unquoted_name("user", None)
        .set_columns(user_columns())
        .set_primary_key_constraint(pk(&["id"]))
        .set_indexes(vec![
            Index::editor()
                .set_unquoted_name("index1")
                .set_unquoted_column_names(&["article", "post"])
                .create()
                .unwrap(),
        ])
        .set_foreign_key_constraints(user_foreign_keys())
        .create()
        .unwrap();

    let diff = TableDiff::new(
        table.clone(),
        TableDiffParts {
            changed_columns: vec![
                ColumnDiff::new(
                    table.get_column("id").unwrap().clone(),
                    column("key", Types::INTEGER).create().unwrap(),
                ),
                ColumnDiff::new(
                    table.get_column("post").unwrap().clone(),
                    column("comment", Types::INTEGER).create().unwrap(),
                ),
            ],
            dropped_columns: vec![column("parent", Types::INTEGER).create().unwrap()],
            dropped_indexes: vec![table.get_index("index1").unwrap().clone()],
            ..TableDiffParts::default()
        },
    );

    assert_eq!(
        platform().get_alter_table_sql(&diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__user AS SELECT id, article, post FROM user",
            "DROP TABLE user",
            "CREATE TABLE user (\
             \"key\" INTEGER NOT NULL, article INTEGER NOT NULL, comment INTEGER NOT NULL\
             , PRIMARY KEY (\"key\")\
             , FOREIGN KEY (article) REFERENCES article (id) DEFERRABLE INITIALLY IMMEDIATE\
             , FOREIGN KEY (comment) REFERENCES post (id) DEFERRABLE INITIALLY DEFERRED\
             )",
            "INSERT INTO user (\"key\", article, comment) SELECT id, article, post FROM __temp__user",
            "DROP TABLE __temp__user",
            "CREATE INDEX IDX_8D93D64923A0E66 ON user (article)",
            "CREATE INDEX IDX_8D93D6495A8A6C8D ON user (comment)",
        ])
    );
}

#[test]
fn testGeneratesAlterTableRenameColumnSQLWithSchema() {
    let table = Table::editor()
        .set_unquoted_name("t", Some("main"))
        .set_columns(vec![column("a", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    let table_diff = TableDiff::new(
        table,
        TableDiffParts {
            changed_columns: vec![ColumnDiff::new(
                column("a", Types::INTEGER).create().unwrap(),
                column("b", Types::INTEGER).create().unwrap(),
            )],
            ..TableDiffParts::default()
        },
    );

    assert_eq!(
        platform().get_alter_table_sql(&table_diff).unwrap(),
        strings(&[
            "CREATE TEMPORARY TABLE __temp__t AS SELECT a FROM main.t",
            "DROP TABLE main.t",
            "CREATE TABLE main.t (b INTEGER NOT NULL)",
            "INSERT INTO main.t (b) SELECT a FROM __temp__t",
            "DROP TABLE __temp__t",
        ])
    );
}

#[test]
fn testAddedDateTimeColumnWithCurrentDefaultTriggersTableRebuild() {
    for (name, args) in provider(
        "Platforms/SQLitePlatformTest",
        "testAddedDateTimeColumnWithCurrentDefaultTriggersTableRebuild",
    ) {
        let type_name = text(&args[0]);
        let default = text(&args[1]);
        // Rendering the legacy string CURRENT_* default is deprecated.
        expect_deprecation("https://github.com/doctrine/dbal/pull/7195", || {
            let table = Table::editor()
                .set_unquoted_name("t", None)
                .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
                .create()
                .unwrap();

            let table_diff = TableDiff::new(
                table,
                TableDiffParts {
                    added_columns: vec![
                        column("c", &type_name)
                            .set_default_value(Value::string(default.clone()))
                            .create()
                            .unwrap(),
                    ],
                    ..TableDiffParts::default()
                },
            );

            // A CURRENT_* default forces the table-rebuild strategy on
            // SQLite, because ALTER TABLE ... ADD COLUMN cannot use a
            // non-constant default.
            assert!(
                platform().get_alter_table_sql(&table_diff).unwrap().len() > 1,
                "{name}"
            );
        });
    }
}

#[test]
#[ignore = "markTestSkipped upstream: SQLite does not support altering foreign key constraints"]
fn testQuotesDropForeignKeySQL() {}

#[test]
fn testDateAddStaticNumberOfDays() {
    assert_eq!(
        platform()
            .get_date_add_days_expression("rentalBeginsOn", "12")
            .unwrap(),
        "DATETIME(rentalBeginsOn,'+' || 12 || ' DAY')"
    );
}

#[test]
fn testDateAddNumberOfDaysFromColumn() {
    assert_eq!(
        platform()
            .get_date_add_days_expression("rentalBeginsOn", "duration")
            .unwrap(),
        "DATETIME(rentalBeginsOn,'+' || duration || ' DAY')"
    );
}

#[test]
fn testSupportsColumnCollation() {
    assert!(platform().supports_column_collation());
}

#[test]
fn testGetCreateTableSQLWithColumnCollation() {
    let table = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![
            column("no_collation", Types::STRING)
                .set_length(Some(255))
                .create()
                .unwrap(),
            column("column_collation", Types::STRING)
                .set_length(Some(255))
                .set_collation(Some("NOCASE"))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&["CREATE TABLE foo (no_collation VARCHAR(255) NOT NULL, \
             column_collation VARCHAR(255) NOT NULL COLLATE \"NOCASE\")"])
    );
}

#[test]
fn testCreateTableWithoutRowid() {
    let table = Table::editor()
        .set_unquoted_name("mytable", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(pk(&["id"]))
        .set_options(php_array!["without_rowid" => true])
        .create()
        .unwrap();

    assert_eq!(
        platform().get_create_table_sql(&table).unwrap(),
        strings(&["CREATE TABLE mytable (id INTEGER NOT NULL, PRIMARY KEY (id)) WITHOUT ROWID"])
    );
}

#[test]
fn testCreateTableWithNonPrimaryKeyAutoIncrementColumn() {
    let table = Table::editor()
        .set_unquoted_name("test_autoincrement", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    expect_deprecation("https://github.com/doctrine/dbal/pull/6849", || {
        platform().get_create_table_sql(&table).unwrap();
    });
}

#[test]
fn testCreateTableWithCompositePrimaryKeyAutoIncrementColumn() {
    let table = Table::editor()
        .set_unquoted_name("test_autoincrement", None)
        .set_columns(vec![
            column("id1", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("id2", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(pk(&["id1", "id2"]))
        .create()
        .unwrap();

    expect_deprecation("https://github.com/doctrine/dbal/pull/6849", || {
        platform().get_create_table_sql(&table).unwrap();
    });
}

// Vectors: SQLitePlatform's outputs recorded on generated inputs
// (tests/data/php.json "sqlite_platform"; oracle/recorders/doctrine_dbal.php).

type Json = serde_json::Value;

fn json_str(json: &Json) -> &str {
    json.as_str().expect("string")
}

fn json_strs(json: &Json) -> Vec<String> {
    json.as_array()
        .expect("list")
        .iter()
        .map(|item| json_str(item).to_owned())
        .collect()
}

fn opt_str(json: &Json) -> Option<&str> {
    json.as_str()
}

/// `[[option, typed value], ...]` as an options array.
fn options(json: &Json) -> PhpArray {
    let mut out = PhpArray::new();
    for pair in json.as_array().expect("options") {
        let value = php_value::vectors::decode(&pair[1]).expect("typed value");
        out.insert(json_str(&pair[0]), value);
    }
    out
}

fn throws(e: &Exception) -> Json {
    serde_json::json!([
        "throws",
        e.class(),
        String::from_utf8_lossy(e.message().as_bytes())
    ])
}

fn sql_list(result: Result<Vec<String>, Exception>) -> Json {
    match result {
        Ok(sql) => serde_json::json!(["ok", sql]),
        Err(e) => throws(&e),
    }
}

fn vector_column(json: &Json) -> Result<Column, Exception> {
    Column::new(json_str(&json[0]), json_str(&json[1]), &options(&json[2]))
}

/// The recorder's `sqlite_table()`: the table built with the legacy
/// `Table` methods.
fn vector_table(spec: &Json) -> Result<Table, Exception> {
    let mut table = Table::new(json_str(&spec["name"]))?;
    for column in spec["columns"].as_array().expect("columns") {
        table.add_column(
            json_str(&column[0]),
            json_str(&column[1]),
            &options(&column[2]),
        )?;
    }
    if !spec["pk"].is_null() {
        let columns = json_strs(&spec["pk"]);
        let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
        table.set_primary_key(&columns, None)?;
    }
    for index in spec["indexes"].as_array().expect("indexes") {
        let columns = json_strs(&index[1]);
        if index[2].as_bool().expect("unique") {
            table.add_unique_index(columns, opt_str(&index[0]), PhpArray::new())?;
        } else {
            table.add_index(columns, opt_str(&index[0]), Vec::new(), PhpArray::new())?;
        }
    }
    for fk in spec["fks"].as_array().expect("fks") {
        let local = json_strs(&fk[1]);
        let local: Vec<&str> = local.iter().map(String::as_str).collect();
        let foreign = json_strs(&fk[3]);
        let foreign: Vec<&str> = foreign.iter().map(String::as_str).collect();
        table.add_foreign_key_constraint(
            json_str(&fk[2]),
            &local,
            &foreign,
            options(&fk[4]),
            opt_str(&fk[0]),
        )?;
    }
    if let Some(comment) = opt_str(&spec["comment"]) {
        table.set_comment(comment);
    }
    Ok(table)
}

fn vector_index(json: &Json) -> Result<Index, Exception> {
    Index::new(
        opt_str(&json[0]),
        json_strs(&json[1]),
        json[2].as_bool().expect("unique"),
        false,
        Vec::<String>::new(),
        PhpArray::new(),
    )
}

fn vector_alter(old: &Json, diff: &Json) -> Result<Vec<String>, Exception> {
    let table = vector_table(old)?;
    let list = |key: &str| diff[key].as_array().expect("list").clone();
    let mut parts = TableDiffParts::default();
    for column in list("added_columns") {
        parts.added_columns.push(vector_column(&column)?);
    }
    for change in list("changed_columns") {
        let old_column = table.get_column(json_str(&change[0]))?.clone();
        parts
            .changed_columns
            .push(ColumnDiff::new(old_column, vector_column(&change[1])?));
    }
    for name in list("dropped_columns") {
        parts
            .dropped_columns
            .push(table.get_column(json_str(&name))?.clone());
    }
    for index in list("added_indexes") {
        parts.added_indexes.push(vector_index(&index)?);
    }
    for name in list("dropped_indexes") {
        parts
            .dropped_indexes
            .push(table.get_index(json_str(&name))?.clone());
    }
    for rename in list("renamed_indexes") {
        parts
            .renamed_indexes
            .push((json_str(&rename[0]).to_owned(), vector_index(&rename[1])?));
    }
    for fk in list("added_fks") {
        parts.added_foreign_keys.push(ForeignKeyConstraint::new(
            json_strs(&fk[1]),
            json_str(&fk[2]),
            json_strs(&fk[3]),
            opt_str(&fk[0]).unwrap_or(""),
            options(&fk[4]),
        )?);
    }
    for name in list("dropped_fks") {
        parts
            .dropped_foreign_keys
            .push(table.get_foreign_key(json_str(&name))?.clone());
    }
    platform().get_alter_table_sql(&TableDiff::new(table.clone(), parts))
}

fn vector_expression(call: &str, args: &[Json]) -> Json {
    let p = platform();
    let arg = |i: usize| json_str(&args[i]);
    let ok = |sql: String| serde_json::json!(["ok", sql]);
    let result = |r: Result<String, Exception>| match r {
        Ok(sql) => ok(sql),
        Err(e) => throws(&e),
    };
    match call {
        "trim" => {
            let mode = match arg(0) {
                "UNSPECIFIED" => TrimMode::Unspecified,
                "LEADING" => TrimMode::Leading,
                "TRAILING" => TrimMode::Trailing,
                "BOTH" => TrimMode::Both,
                other => panic!("unknown trim mode {other}"),
            };
            ok(p.get_trim_expression(arg(1), mode, opt_str(&args[2])))
        }
        "locate" => result(p.get_locate_expression(arg(0), arg(1), opt_str(&args[2]))),
        "substring" => ok(p.get_substring_expression(arg(0), arg(1), opt_str(&args[2]))),
        "date_add" | "date_sub" => {
            let (date, n) = (arg(1), arg(2));
            let add = call == "date_add";
            result(match (arg(0), add) {
                ("seconds", true) => p.get_date_add_seconds_expression(date, n),
                ("seconds", false) => p.get_date_sub_seconds_expression(date, n),
                ("minutes", true) => p.get_date_add_minutes_expression(date, n),
                ("minutes", false) => p.get_date_sub_minutes_expression(date, n),
                ("hour", true) => p.get_date_add_hour_expression(date, n),
                ("hour", false) => p.get_date_sub_hour_expression(date, n),
                ("days", true) => p.get_date_add_days_expression(date, n),
                ("days", false) => p.get_date_sub_days_expression(date, n),
                ("weeks", true) => p.get_date_add_weeks_expression(date, n),
                ("weeks", false) => p.get_date_sub_weeks_expression(date, n),
                ("month", true) => p.get_date_add_month_expression(date, n),
                ("month", false) => p.get_date_sub_month_expression(date, n),
                ("quarters", true) => p.get_date_add_quarters_expression(date, n),
                ("quarters", false) => p.get_date_sub_quarters_expression(date, n),
                ("years", true) => p.get_date_add_years_expression(date, n),
                ("years", false) => p.get_date_sub_years_expression(date, n),
                (other, _) => panic!("unknown unit {other}"),
            })
        }
        "date_diff" => result(p.get_date_diff_expression(arg(0), arg(1))),
        "mod" => ok(p.get_mod_expression(arg(0), arg(1))),
        "bit_and" => ok(p.get_bit_and_comparison_expression(arg(0), arg(1))),
        "bit_or" => ok(p.get_bit_or_comparison_expression(arg(0), arg(1))),
        "concat" => {
            let strings: Vec<&str> = args.iter().map(json_str).collect();
            ok(p.get_concat_expression(&strings))
        }
        "length" => ok(p.get_length_expression(arg(0))),
        "escape_like" => ok(p.escape_string_for_like(arg(0), arg(1))),
        "quote_literal" => ok(p.quote_string_literal(arg(0))),
        "quote_single_identifier" => ok(p.quote_single_identifier(arg(0))),
        "truncate" => ok(p.get_truncate_table_sql(arg(0), args[1].as_bool().expect("cascade"))),
        "modify_limit" => {
            let limit = args[1].as_i64();
            let offset = args[2].as_i64().expect("offset");
            match p.modify_limit_query(arg(0), limit, offset) {
                Ok(sql) => ok(sql),
                Err(e) => serde_json::json!([
                    "throws",
                    "Doctrine\\DBAL\\Exception\\InvalidArgumentException",
                    e.message()
                ]),
            }
        }
        other => panic!("unknown call {other}"),
    }
}

fn vectors(key: &str) -> &'static [Json] {
    data()["sqlite_platform"][key]
        .as_array()
        .unwrap_or_else(|| panic!("tests/data/php.json has no sqlite_platform.{key} vectors"))
}

fn check_all(key: &str, rows: &[Json], actual: impl Fn(&Json) -> Json) {
    let expected_of = |row: &Json| row.as_array().and_then(|a| a.last()).cloned();
    let failures: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            let expected = expected_of(row)?;
            let got = actual(row);
            (got != expected).then(|| format!("{row}\n  got: {got}"))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} {key} vectors differ:\n{}",
        failures.len(),
        rows.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn sqlite_platform_expression_vectors() {
    let rows = vectors("expressions");
    check_all("expressions", rows, |row| {
        vector_expression(json_str(&row[0]), row[1].as_array().expect("args"))
    });
}

#[test]
fn sqlite_platform_create_table_vectors() {
    let rows = vectors("create_table");
    check_all("create_table", rows, |row| {
        sql_list(vector_table(&row[0]).and_then(|table| platform().get_create_table_sql(&table)))
    });
}

#[test]
fn sqlite_platform_alter_table_vectors() {
    let rows = vectors("alter_table");
    check_all("alter_table", rows, |row| {
        sql_list(vector_alter(&row[0], &row[1]))
    });
}

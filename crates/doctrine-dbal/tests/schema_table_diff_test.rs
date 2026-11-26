//! Port of tests/Schema/TableDiffTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/TableDiffTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::{Column, ForeignKeyConstraint, Table, TableDiff, TableDiffParts};
use doctrine_dbal::types::Types;
use schema_support::{expect_deprecation, expect_exception};

#[test]
fn testCreateWithInvalidDroppedForeignKeyName() {
    let table = Table::editor()
        .set_unquoted_name("t1", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("c1")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let dropped_foreign_keys = ForeignKeyConstraint::editor()
        .set_unquoted_referencing_column_names(&["c1"])
        .set_unquoted_referenced_table_name("t2", None)
        .set_unquoted_referenced_column_names(&["c1"])
        .create()
        .unwrap();

    let diff = expect_deprecation("https://github.com/doctrine/dbal/pull/7143", || {
        TableDiff::new(
            table,
            TableDiffParts {
                dropped_foreign_keys: vec![dropped_foreign_keys],
                ..TableDiffParts::default()
            },
        )
    });

    expect_exception(
        "InvalidState",
        diff.get_dropped_foreign_key_constraint_names(),
    );
}

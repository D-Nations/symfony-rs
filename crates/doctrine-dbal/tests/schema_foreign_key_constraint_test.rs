//! Port of tests/Schema/ForeignKeyConstraintTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the data providers' rows are written in place.
//!
//! Upstream: doctrine/dbal tests/Schema/ForeignKeyConstraintTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::name::{Identifier, OptionallyQualifiedName, UnqualifiedName};
use doctrine_dbal::schema::{
    Deferrability, ForeignKeyConstraint, Index, MatchType, ReferentialAction,
};
use php_value::{PhpArray, php_array};
use schema_support::{expect_deprecation, expect_exception, expect_no_deprecation};

const LINK_6728: &str = "https://github.com/doctrine/dbal/pull/6728";

fn fk(
    local: &[&str],
    foreign_table: &str,
    foreign: &[&str],
    name: &str,
    options: PhpArray,
) -> ForeignKeyConstraint {
    ForeignKeyConstraint::new(
        local.iter().copied(),
        foreign_table,
        foreign.iter().copied(),
        name,
        options,
    )
    .unwrap()
}

#[test]
fn testIntersectsIndexColumns() {
    let rows: [(&[&str], bool); 14] = [
        (&["baz"], false),
        (&["baz", "bloo"], false),
        (&["foo"], true),
        (&["bar"], true),
        (&["foo", "bar"], true),
        (&["bar", "foo"], true),
        (&["foo", "baz"], true),
        (&["baz", "foo"], true),
        (&["bar", "baz"], true),
        (&["baz", "bar"], true),
        (&["foo", "bloo", "baz"], true),
        (&["bloo", "foo", "baz"], true),
        (&["bloo", "baz", "foo"], true),
        (&["FOO"], true),
    ];
    for (index_columns, expected_result) in rows {
        let foreign_key = fk(
            &["foo", "bar"],
            "foreign_table",
            &["fk_foo", "fk_bar"],
            "",
            PhpArray::new(),
        );
        let index = Index::new(
            Some("foo"),
            index_columns.iter().copied(),
            false,
            false,
            Vec::<String>::new(),
            PhpArray::new(),
        )
        .unwrap();
        assert_eq!(
            foreign_key.intersects_index_columns(&index),
            expected_result,
            "{index_columns:?}"
        );
    }
}

#[test]
fn testGetUnqualifiedForeignTableName() {
    for (foreign_table_name, expected) in [
        ("schema.foreign_table", "foreign_table"),
        ("schema.\"foreign_table\"", "foreign_table"),
        ("\"schema\".\"foreign_table\"", "foreign_table"),
        ("foreign_table", "foreign_table"),
    ] {
        let foreign_key = fk(
            &["foo", "bar"],
            foreign_table_name,
            &["fk_foo", "fk_bar"],
            "",
            PhpArray::new(),
        );
        assert_eq!(foreign_key.get_unqualified_foreign_table_name(), expected);
    }
}

#[test]
fn testCompareRestrictAndNoActionAreTheSame() {
    let fk1 = fk(
        &["foo"],
        "bar",
        &["baz"],
        "fk1",
        php_array!["onDelete" => "NO ACTION"],
    );
    let fk2 = fk(
        &["foo"],
        "bar",
        &["baz"],
        "fk1",
        php_array!["onDelete" => "RESTRICT"],
    );
    assert_eq!(fk1.on_delete(), fk2.on_delete());
}

#[test]
fn testGetNonNullObjectName() {
    let foreign_key = fk(
        &["user_id"],
        "users",
        &["id"],
        "fk_user_id",
        PhpArray::new(),
    );
    let name = foreign_key.get_object_name().unwrap().expect("a name");
    assert_eq!(
        name.get_identifier(),
        &Identifier::unquoted("fk_user_id").unwrap()
    );
}

#[test]
fn testGetNullObjectName() {
    let foreign_key = fk(&["user_id"], "users", &["id"], "", PhpArray::new());
    assert!(foreign_key.get_object_name().unwrap().is_none());
}

#[test]
fn testEmptyReferencingColumnNames() {
    let foreign_key = fk(&[], "users", &["id"], "", PhpArray::new());
    expect_exception("InvalidState", foreign_key.get_referencing_column_names());
}

#[test]
fn testInvalidReferencingColumnNames() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(&[""], "users", &["id"], "", PhpArray::new())
    });
    expect_exception("InvalidState", foreign_key.get_referencing_column_names());
}

#[test]
fn testInvalidReferencedTableName() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(&["user_id"], "", &["id"], "", PhpArray::new())
    });
    expect_exception("InvalidState", foreign_key.get_referenced_table_name());
}

#[test]
fn testEmptyReferencedColumnNames() {
    let foreign_key = fk(&["user_id"], "users", &[], "", PhpArray::new());
    expect_exception("InvalidState", foreign_key.get_referenced_column_names());
}

#[test]
fn testInvalidReferencedColumnNames() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(&["user_id"], "users", &[""], "", PhpArray::new())
    });
    expect_exception("InvalidState", foreign_key.get_referenced_column_names());
}

#[test]
fn testInvalidMatchType() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(
            &["user_id"],
            "users",
            &["id"],
            "",
            php_array!["match" => "MAYBE"],
        )
    });
    expect_exception("InvalidState", foreign_key.get_match_type());
}

#[test]
fn testInvalidOnUpdateAction() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(
            &["user_id"],
            "users",
            &["id"],
            "",
            php_array!["onUpdate" => "DROP"],
        )
    });
    expect_exception("InvalidState", foreign_key.get_on_update_action());
}

#[test]
fn testInvalidOnDeleteAction() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(
            &["user_id"],
            "users",
            &["id"],
            "",
            php_array!["onDelete" => "DROP"],
        )
    });
    expect_exception("InvalidState", foreign_key.get_on_delete_action());
}

#[test]
fn testInvalidDeferrability() {
    let foreign_key = expect_deprecation(LINK_6728, || {
        fk(
            &["user_id"],
            "users",
            &["id"],
            "",
            php_array!["deferrable" => false, "deferred" => true],
        )
    });
    expect_exception("InvalidState", foreign_key.get_deferrability());
}

#[test]
fn testParseDeferrability() {
    let rows = [
        ("unspecified", php_array![], Deferrability::NotDeferrable),
        (
            "INITIALLY IMMEDIATE",
            php_array!["deferred" => false],
            Deferrability::NotDeferrable,
        ),
        (
            "INITIALLY DEFERRED",
            php_array!["deferred" => true],
            Deferrability::Deferred,
        ),
        (
            "NOT DEFERRABLE",
            php_array!["deferrable" => false],
            Deferrability::NotDeferrable,
        ),
        (
            "NOT DEFERRABLE INITIALLY IMMEDIATE",
            php_array!["deferrable" => false, "deferred" => false],
            Deferrability::NotDeferrable,
        ),
        (
            "DEFERRABLE",
            php_array!["deferrable" => true],
            Deferrability::Deferrable,
        ),
        (
            "DEFERRABLE INITIALLY IMMEDIATE",
            php_array!["deferrable" => true, "deferred" => false],
            Deferrability::Deferrable,
        ),
        (
            "DEFERRABLE INITIALLY DEFERRED",
            php_array!["deferrable" => true, "deferred" => true],
            Deferrability::Deferred,
        ),
    ];
    for (row, options, expected) in rows {
        let foreign_key = expect_no_deprecation(LINK_6728, || {
            fk(&["user_id"], "users", &["id"], "", options)
        });
        assert_eq!(foreign_key.get_deferrability().unwrap(), expected, "{row}");
    }
}

#[test]
fn testAllValidProperties() {
    let foreign_key = expect_no_deprecation(LINK_6728, || {
        fk(
            &["user_id"],
            "users",
            &["id"],
            "fk_user_id",
            PhpArray::new(),
        )
    });
    assert_eq!(
        foreign_key.get_referencing_column_names().unwrap(),
        [UnqualifiedName::unquoted("user_id").unwrap()]
    );
    assert_eq!(
        foreign_key.get_referenced_table_name().unwrap(),
        &OptionallyQualifiedName::unquoted("users", None).unwrap()
    );
    assert_eq!(
        foreign_key.get_referenced_column_names().unwrap(),
        [UnqualifiedName::unquoted("id").unwrap()]
    );
    assert_eq!(foreign_key.get_match_type().unwrap(), MatchType::Simple);
    assert_eq!(
        foreign_key.get_on_update_action().unwrap(),
        ReferentialAction::NoAction
    );
    assert_eq!(
        foreign_key.get_on_delete_action().unwrap(),
        ReferentialAction::NoAction
    );
    assert_eq!(
        foreign_key.get_deferrability().unwrap(),
        Deferrability::NotDeferrable
    );
}

//! Port of tests/Schema/UniqueConstraintTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the data provider's rows are written in place.
//!
//! Upstream: doctrine/dbal tests/Schema/UniqueConstraintTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::UniqueConstraint;
use doctrine_dbal::schema::name::UnqualifiedName;
use php_value::{PhpArray, php_array};
use schema_support::{expect_deprecation, expect_exception};

const LINK_6685: &str = "https://github.com/doctrine/dbal/pull/6685";

fn constraint(columns: &[&str], flags: &[&str], options: PhpArray) -> UniqueConstraint {
    UniqueConstraint::new("", columns.iter().copied(), flags.iter().copied(), options)
}

#[test]
fn testGetNonNullObjectName() {
    let name = UnqualifiedName::unquoted("uq_user_id").unwrap();
    let unique_constraint = UniqueConstraint::editor()
        .set_name(Some(name.clone()))
        .set_column_names(vec![UnqualifiedName::unquoted("user_id").unwrap()])
        .create()
        .unwrap();
    assert_eq!(unique_constraint.get_object_name().unwrap(), Some(name));
}

#[test]
fn testGetNullObjectName() {
    let unique_constraint = UniqueConstraint::editor()
        .set_unquoted_column_names(&["user_id"])
        .create()
        .unwrap();
    assert!(unique_constraint.get_object_name().unwrap().is_none());
}

#[test]
fn testInstantiateWithOptions() {
    expect_deprecation(LINK_6685, || {
        constraint(&["user_id"], &[], php_array!["option" => "value"]);
    });
}

#[test]
fn testGetColumnNames() {
    let unique_constraint = constraint(&["user_id"], &[], PhpArray::new());
    assert_eq!(
        unique_constraint.get_column_names().unwrap(),
        [UnqualifiedName::unquoted("user_id").unwrap()]
    );
}

#[test]
fn testInvalidColumnNames() {
    let unique_constraint =
        expect_deprecation(LINK_6685, || constraint(&[""], &[], PhpArray::new()));
    expect_exception("InvalidState", unique_constraint.get_column_names());
}

#[test]
fn testEmptyColumnNames() {
    let unique_constraint = expect_deprecation(LINK_6685, || constraint(&[], &[], PhpArray::new()));
    expect_exception("InvalidState", unique_constraint.get_column_names());
}

#[test]
fn testIsClustered() {
    for (row, flags, expected) in [
        ("clustered", &["clustered"][..], true),
        ("not clustered", &[][..], false),
    ] {
        let unique_constraint = constraint(&["user_id"], flags, PhpArray::new());
        assert_eq!(unique_constraint.is_clustered(), expected, "{row}");
    }
}

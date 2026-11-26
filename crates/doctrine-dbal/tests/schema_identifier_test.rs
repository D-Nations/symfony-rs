//! Port of tests/Schema/IdentifierTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/IdentifierTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::Identifier;
use doctrine_dbal::schema::name::Identifier as NameIdentifier;
use schema_support::expect_deprecation;

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        Identifier::new("", false);
    });
}

#[test]
fn testGetObjectName() {
    let identifier = Identifier::new("warehouse.inventory.products.id", false);
    let name = identifier.get_object_name().unwrap();
    let expected: Vec<NameIdentifier> = ["warehouse", "inventory", "products", "id"]
        .into_iter()
        .map(|value| NameIdentifier::unquoted(value).unwrap())
        .collect();
    assert_eq!(name.get_identifiers(), expected.as_slice());
}

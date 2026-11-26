//! Port of tests/Schema/Index/IndexedColumnTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/Index/IndexedColumnTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::IndexedColumn;
use doctrine_dbal::schema::name::UnqualifiedName;
use schema_support::expect_exception;

#[test]
fn testNonPositiveColumnLength() {
    expect_exception(
        "InvalidIndexDefinition",
        IndexedColumn::new(UnqualifiedName::unquoted("id").unwrap(), Some(-1)),
    );
}

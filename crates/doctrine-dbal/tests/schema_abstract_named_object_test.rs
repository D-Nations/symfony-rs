//! Port of tests/Schema/AbstractNamedObjectTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the anonymous subclasses of `AbstractNamedObject` are
//! `AbstractAsset::new()` with `AssetKind::Named` (and the parser they do
//! not override).
//!
//! Upstream: doctrine/dbal tests/Schema/AbstractNamedObjectTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::{AbstractAsset, AssetKind, NameParser};
use schema_support::{expect_deprecation, expect_exception};

const CLASS: &str = "Doctrine\\DBAL\\Schema\\AbstractNamedObject@anonymous";

fn object(name: &str) -> AbstractAsset {
    AbstractAsset::new(
        CLASS,
        AssetKind::Named,
        NameParser::NotImplemented(CLASS),
        Some(name),
    )
}

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        object("");
    });
}

#[test]
fn testSetAccessToUninitializedName() {
    let object = object("");
    expect_exception("InvalidState", object.get_object_name());
}

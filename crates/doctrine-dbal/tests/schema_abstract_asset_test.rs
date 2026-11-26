//! Port of tests/Schema/AbstractAssetTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the anonymous subclasses of `AbstractAsset` are
//! `AbstractAsset::new()` with `AssetKind::Bare`. The name parsing tests
//! need the Oracle, PostgreSQL and MySQL platforms, which are not ported
//! (pending).
//!
//! Upstream: doctrine/dbal tests/Schema/AbstractAssetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use doctrine_dbal::schema::{AbstractAsset, AssetKind, NameParser};
use schema_support::expect_deprecation;

const CLASS: &str = "Doctrine\\DBAL\\Schema\\AbstractAsset@anonymous";

#[test]
fn testConstructWithoutArguments() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6610", || {
        AbstractAsset::new(
            CLASS,
            AssetKind::Bare,
            NameParser::NotImplemented(CLASS),
            None,
        );
    });
}

#[test]
fn testCreateParserNotImplemented() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6592", || {
        AbstractAsset::new(
            CLASS,
            AssetKind::Bare,
            NameParser::NotImplemented(CLASS),
            Some("foo"),
        );
    });
}

//! Port of tests/Schema/AbstractOptionallyNamedObjectTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; the anonymous subclasses of
//! `AbstractOptionallyNamedObject` are `AbstractAsset::new()` with
//! `AssetKind::OptionallyNamed`.
//!
//! Upstream: doctrine/dbal tests/Schema/AbstractOptionallyNamedObjectTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::schema::{AbstractAsset, AssetKind, NameParser};

const CLASS: &str = "Doctrine\\DBAL\\Schema\\AbstractOptionallyNamedObject@anonymous";

#[test]
fn testEmptyName() {
    for (row, name) in [("empty-string", Some("")), ("empty-null", None)] {
        let object = AbstractAsset::new(
            CLASS,
            AssetKind::OptionallyNamed,
            NameParser::NotImplemented(CLASS),
            name,
        );
        assert!(object.get_object_name().unwrap().is_none(), "{row}");
    }
}

// testSetAccessToUninitializedName: pending. The PHP test builds a
// subclass whose constructor skips `parent::__construct()`; every
// `AbstractAsset` here is built by `AbstractAsset::new()`.

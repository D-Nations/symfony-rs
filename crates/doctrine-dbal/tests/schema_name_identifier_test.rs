//! Port of tests/Schema/Name/IdentifierTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the data providers build objects, so their rows are written
//! in place.
//!
//! Upstream: doctrine/dbal tests/Schema/Name/IdentifierTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::schema::name::{Identifier, UnquotedIdentifierFolding};

fn q(value: &str) -> Identifier {
    Identifier::quoted(value).unwrap()
}

fn u(value: &str) -> Identifier {
    Identifier::unquoted(value).unwrap()
}

#[test]
fn testIdentifierCannotBeEmpty() {
    let e = Identifier::unquoted("").unwrap_err();
    assert!(
        e.is_instance_of("Doctrine\\DBAL\\Schema\\Exception\\InvalidIdentifier"),
        "{e}"
    );
}

#[test]
fn testToString() {
    for (identifier, expected) in [
        (u("id"), "id"),
        (q("name"), "\"name\""),
        (q("\"value\""), "\"\"\"value\"\"\""),
    ] {
        assert_eq!(identifier.to_string(), expected);
    }
}

#[test]
fn testEqualsToSelf() {
    let identifier = u("id");
    assert!(identifier.equals(&identifier, UnquotedIdentifierFolding::None));
}

#[test]
fn testEqualsToOther() {
    for (identifier1, identifier2, folding) in [
        (u("id"), u("id"), UnquotedIdentifierFolding::None),
        (q("id"), q("id"), UnquotedIdentifierFolding::None),
        (q("id"), u("ID"), UnquotedIdentifierFolding::Lower),
        (q("ID"), u("id"), UnquotedIdentifierFolding::Upper),
    ] {
        assert!(identifier1.equals(&identifier2, folding));
        assert!(identifier2.equals(&identifier1, folding));
    }
}

#[test]
fn testNonEqualsToOther() {
    for (identifier1, identifier2, folding) in [
        (u("foo"), u("bar"), UnquotedIdentifierFolding::None),
        (u("id"), u("ID"), UnquotedIdentifierFolding::None),
        (q("id"), q("ID"), UnquotedIdentifierFolding::Lower),
        (q("ID"), q("id"), UnquotedIdentifierFolding::Upper),
    ] {
        assert!(!identifier1.equals(&identifier2, folding));
        assert!(!identifier2.equals(&identifier1, folding));
    }
}

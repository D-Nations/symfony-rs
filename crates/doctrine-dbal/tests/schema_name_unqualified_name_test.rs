//! Port of tests/Schema/Name/UnqualifiedNameTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/Name/UnqualifiedNameTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::schema::name::{UnqualifiedName, UnquotedIdentifierFolding};

#[test]
fn testQuoted() {
    let name = UnqualifiedName::quoted("id").unwrap();
    let identifier = name.get_identifier();
    assert!(identifier.is_quoted());
    assert_eq!(identifier.get_value(), "id");
    assert_eq!(name.to_string(), "\"id\"");
}

#[test]
fn testUnquoted() {
    let name = UnqualifiedName::unquoted("id").unwrap();
    let identifier = name.get_identifier();
    assert!(!identifier.is_quoted());
    assert_eq!(identifier.get_value(), "id");
    assert_eq!(name.to_string(), "id");
}

#[test]
fn testEqualsToSelf() {
    let name = UnqualifiedName::unquoted("id").unwrap();
    assert!(name.equals(&name, UnquotedIdentifierFolding::None));
}

#[test]
fn testEqualsToOther() {
    let name1 = UnqualifiedName::unquoted("id").unwrap();
    let name2 = UnqualifiedName::unquoted("id").unwrap();
    assert!(name1.equals(&name2, UnquotedIdentifierFolding::None));
    assert!(name2.equals(&name1, UnquotedIdentifierFolding::None));
}

#[test]
fn testNotEqualsToOther() {
    let name1 = UnqualifiedName::unquoted("id").unwrap();
    let name2 = UnqualifiedName::unquoted("name").unwrap();
    assert!(!name1.equals(&name2, UnquotedIdentifierFolding::None));
    assert!(!name2.equals(&name1, UnquotedIdentifierFolding::None));
}

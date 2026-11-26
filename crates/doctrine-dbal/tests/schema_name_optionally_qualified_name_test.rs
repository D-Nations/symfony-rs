//! Port of tests/Schema/Name/OptionallyQualifiedNameTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; the data providers build objects, so their
//! rows are written in place.
//!
//! Upstream: doctrine/dbal tests/Schema/Name/OptionallyQualifiedNameTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use doctrine_dbal::schema::name::{OptionallyQualifiedName, UnquotedIdentifierFolding};

const NONE: UnquotedIdentifierFolding = UnquotedIdentifierFolding::None;

fn u(name: &str, qualifier: Option<&str>) -> OptionallyQualifiedName {
    OptionallyQualifiedName::unquoted(name, qualifier).unwrap()
}

#[test]
fn testQualifiedQuoted() {
    let name = OptionallyQualifiedName::quoted("customers", Some("inventory")).unwrap();
    let unqualified_name = name.get_unqualified_name();
    assert!(unqualified_name.is_quoted());
    assert_eq!(unqualified_name.get_value(), "customers");
    let qualifier = name.get_qualifier().expect("a qualifier");
    assert!(qualifier.is_quoted());
    assert_eq!(qualifier.get_value(), "inventory");
    assert_eq!(name.to_string(), "\"inventory\".\"customers\"");
}

#[test]
fn testUnqualifiedQuoted() {
    let name = OptionallyQualifiedName::quoted("customers", None).unwrap();
    let unqualified_name = name.get_unqualified_name();
    assert!(unqualified_name.is_quoted());
    assert_eq!(unqualified_name.get_value(), "customers");
    assert!(name.get_qualifier().is_none());
    assert_eq!(name.to_string(), "\"customers\"");
}

#[test]
fn testQualifiedUnquoted() {
    let name = u("customers", Some("inventory"));
    let unqualified_name = name.get_unqualified_name();
    assert!(!unqualified_name.is_quoted());
    assert_eq!(unqualified_name.get_value(), "customers");
    let qualifier = name.get_qualifier().expect("a qualifier");
    assert!(!qualifier.is_quoted());
    assert_eq!(qualifier.get_value(), "inventory");
    assert_eq!(name.to_string(), "inventory.customers");
}

#[test]
fn testUnqualifiedUnquoted() {
    let name = u("customers", None);
    let unqualified_name = name.get_unqualified_name();
    assert!(!unqualified_name.is_quoted());
    assert_eq!(unqualified_name.get_value(), "customers");
    assert!(name.get_qualifier().is_none());
    assert_eq!(name.to_string(), "customers");
}

#[test]
fn testEqualsToSelf() {
    let name = u("user.id", None);
    assert!(name.equals(&name, NONE).unwrap());
}

#[test]
fn testEqualsNames() {
    for (name1, name2) in [
        (u("id", None), u("id", None)),
        (u("id", Some("user")), u("id", Some("user"))),
    ] {
        assert!(name2.equals(&name1, NONE).unwrap());
        assert!(name1.equals(&name2, NONE).unwrap());
    }
}

#[test]
fn testUnequalsNames() {
    for (name1, name2) in [
        (u("id", None), u("name", None)),
        (u("id", Some("user")), u("name", Some("user"))),
        (u("id", Some("user")), u("id", Some("order"))),
    ] {
        assert!(!name1.equals(&name2, NONE).unwrap());
        assert!(!name2.equals(&name1, NONE).unwrap());
    }
}

#[test]
fn testIncomparableNames() {
    for (name1, name2) in [
        (u("id", None), u("id", Some("user"))),
        (u("id", Some("user")), u("id", None)),
    ] {
        let e = name1.equals(&name2, NONE).unwrap_err();
        assert!(
            e.is_instance_of("Doctrine\\DBAL\\Schema\\Exception\\IncomparableNames"),
            "{e}"
        );
    }
}

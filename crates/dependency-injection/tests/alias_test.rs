//! Tests/AliasTest.php at symfony/dependency-injection 9b34cad8.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/AliasTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_dependency_injection::{Alias, ErrorKind};

#[test]
fn testConstructor() {
    let alias = Alias::new("foo", false);
    assert_eq!("foo", alias.to_string());
    assert!(!alias.is_public());
}

#[test]
fn testCanConstructANonPublicAlias() {
    let alias = Alias::new("foo", false);
    assert_eq!("foo", alias.to_string());
    assert!(!alias.is_public());
}

#[test]
fn testCanConstructAPrivateAlias() {
    // Upstream passes a third argument, which PHP ignores.
    let alias = Alias::new("foo", false);
    assert_eq!("foo", alias.to_string());
    assert!(!alias.is_public());
    assert!(alias.is_private());
}

#[test]
fn testCanSetPublic() {
    let alias = Alias::new("foo", false);
    alias.set_public(true);
    assert!(alias.is_public());
}

#[test]
fn testCanDeprecateAnAlias() {
    let alias = Alias::new("foo", false);
    alias
        .set_deprecated(
            "vendor/package",
            "1.1",
            "The %alias_id% service is deprecated.",
        )
        .unwrap();
    assert!(alias.is_deprecated());
}

#[test]
fn testReturnsCorrectDeprecation() {
    let alias = Alias::new("foo", false);
    alias
        .set_deprecated("vendor/package", "1.1", "The \"%alias_id%\" is deprecated.")
        .unwrap();
    let deprecation = alias.deprecation("foo").unwrap();
    assert_eq!("The \"foo\" is deprecated.", deprecation.message);
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
}

#[test]
fn testCannotDeprecateWithAnInvalidTemplate() {
    for message in [
        "invalid \r message %alias_id%",
        "invalid \n message %alias_id%",
        "invalid */ message %alias_id%",
        "this is deprecated",
    ] {
        let def = Alias::new("foo", false);
        let e = def.set_deprecated("package", "1.1", message).unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument), "{message:?}");
    }
}

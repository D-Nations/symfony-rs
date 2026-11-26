//! Port of tests/ConfigurationTest.php.
//!
//! Upstream: doctrine/dbal tests/ConfigurationTest.php
#![allow(non_snake_case)]

use doctrine_dbal::Configuration;

fn set_up() -> Configuration {
    Configuration::new()
}

/// Tests that the default auto-commit mode for connections can be
/// retrieved from the configuration container.
#[test]
fn testReturnsDefaultConnectionAutoCommitMode() {
    assert!(set_up().get_auto_commit());
}

/// Tests that the default auto-commit mode for connections can be set in
/// the configuration container.
#[test]
fn testSetsDefaultConnectionAutoCommitMode() {
    let mut config = set_up();
    config.set_auto_commit(false);
    assert!(!config.get_auto_commit());
}

//! Tests/Compiler/AutoAliasServicePassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; the fixture classes are only names here.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AutoAliasServicePassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, php_array};
use symfony_dependency_injection::compiler::{AutoAliasServicePass, CompilerPassInterface};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";

#[test]
fn testProcessWithMissingParameter() {
    let mut container = ContainerBuilder::new();

    container.register("example", None).unwrap().add_tag(
        "auto_alias",
        php_array!["format" => "%non_existing%.example"],
    );

    let mut pass = AutoAliasServicePass;

    let e = pass.process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
}

#[test]
fn testProcessWithMissingFormat() {
    let mut container = ContainerBuilder::new();

    container
        .register("example", None)
        .unwrap()
        .add_tag("auto_alias", PhpArray::new());
    container.set_parameter("existing", "mysql").unwrap();

    let mut pass = AutoAliasServicePass;

    let e = pass.process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
}

#[test]
fn testProcessWithNonExistingAlias() {
    let mut container = ContainerBuilder::new();

    container
        .register("example", Some(&format!("{NS}ServiceClassDefault")))
        .unwrap()
        .add_tag("auto_alias", php_array!["format" => "%existing%.example"]);
    container.set_parameter("existing", "mysql").unwrap();

    let mut pass = AutoAliasServicePass;
    pass.process(&mut container).unwrap();

    assert_eq!(
        Some(format!("{NS}ServiceClassDefault")),
        container.get_definition("example").unwrap().class()
    );
}

#[test]
fn testProcessWithExistingAlias() {
    let mut container = ContainerBuilder::new();

    container
        .register("example", Some(&format!("{NS}ServiceClassDefault")))
        .unwrap()
        .add_tag("auto_alias", php_array!["format" => "%existing%.example"]);

    container
        .register("mysql.example", Some(&format!("{NS}ServiceClassMysql")))
        .unwrap();
    container.set_parameter("existing", "mysql").unwrap();

    let mut pass = AutoAliasServicePass;
    pass.process(&mut container).unwrap();

    assert!(container.has_alias("example"));
    assert_eq!(
        "mysql.example",
        container.get_alias("example").unwrap().id()
    );
    assert_eq!(
        Some(format!("{NS}ServiceClassMysql")),
        container.get_definition("mysql.example").unwrap().class()
    );
}

#[test]
fn testProcessWithManualAlias() {
    let mut container = ContainerBuilder::new();

    container
        .register("example", Some(&format!("{NS}ServiceClassDefault")))
        .unwrap()
        .add_tag("auto_alias", php_array!["format" => "%existing%.example"]);

    container
        .register("mysql.example", Some(&format!("{NS}ServiceClassMysql")))
        .unwrap();
    container
        .register("mariadb.example", Some(&format!("{NS}ServiceClassMariaDb")))
        .unwrap();
    container.set_alias("example", "mariadb.example").unwrap();
    container.set_parameter("existing", "mysql").unwrap();

    let mut pass = AutoAliasServicePass;
    pass.process(&mut container).unwrap();

    assert!(container.has_alias("example"));
    assert_eq!(
        "mariadb.example",
        container.get_alias("example").unwrap().id()
    );
    assert_eq!(
        Some(format!("{NS}ServiceClassMariaDb")),
        container.get_definition("mariadb.example").unwrap().class()
    );
}

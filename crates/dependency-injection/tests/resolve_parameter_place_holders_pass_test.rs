//! Tests/Compiler/ResolveParameterPlaceHoldersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `setUp()` is [`set_up`].
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveParameterPlaceHoldersPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::BoundArgument;
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ResolveParameterPlaceHoldersPass,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind};

struct Fixture {
    container: ContainerBuilder,
    foo_definition: Definition,
}

fn set_up() -> Fixture {
    let mut compiler_pass = ResolveParameterPlaceHoldersPass::default();
    let mut container = create_container_builder();
    compiler_pass.process(&mut container).unwrap();
    let foo_definition = container.get_definition("foo").unwrap();
    Fixture {
        container,
        foo_definition,
    }
}

fn create_container_builder() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();

    container.set_parameter("foo.class", "Foo").unwrap();
    container
        .set_parameter("foo.factory.class", "FooFactory")
        .unwrap();
    container.set_parameter("foo.arg1", "bar").unwrap();
    container
        .set_parameter("foo.arg2", php_array!["%foo.arg1%" => "baz"])
        .unwrap();
    container.set_parameter("foo.method", "foobar").unwrap();
    container.set_parameter("foo.property.name", "bar").unwrap();
    container
        .set_parameter("foo.property.value", "baz")
        .unwrap();
    container.set_parameter("foo.file", "foo.php").unwrap();
    container.set_parameter("alias.id", "bar").unwrap();

    let foo_definition = container.register("foo", Some("%foo.class%")).unwrap();
    foo_definition.set_factory(php_array!["%foo.factory.class%", "getFoo"]);
    foo_definition.set_arguments(php_array![
        Value::from("%foo.arg1%"),
        Value::from(php_array!["%foo.arg1%" => "baz"])
    ]);
    foo_definition
        .add_method_call(
            "%foo.method%",
            php_array!["%foo.arg1%", "%foo.arg2%"],
            false,
        )
        .unwrap();
    foo_definition.set_property("%foo.property.name%", "%foo.property.value%");
    foo_definition.set_file(Some("%foo.file%"));
    foo_definition.set_bindings(&php_array!["$baz" => "%env(BAZ)%"]);

    container.set_alias("%alias.id%", "foo").unwrap();

    container
}

#[test]
fn testClassParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(Some("Foo".to_string()), f.foo_definition.class());
}

#[test]
fn testFactoryParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(
        Value::from(php_array!["FooFactory", "getFoo"]),
        f.foo_definition.factory()
    );
}

#[test]
fn testArgumentParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(
        php_array![Value::from("bar"), Value::from(php_array!["bar" => "baz"])],
        f.foo_definition.arguments()
    );
}

#[test]
fn testMethodCallParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(
        php_array![php_array![
            Value::from("foobar"),
            Value::from(php_array![
                Value::from("bar"),
                Value::from(php_array!["bar" => "baz"])
            ])
        ]],
        f.foo_definition.method_calls()
    );
}

#[test]
fn testPropertyParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(php_array!["bar" => "baz"], f.foo_definition.properties());
}

#[test]
fn testFileParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!(Some("foo.php".to_string()), f.foo_definition.file());
}

#[test]
fn testAliasParametersShouldBeResolved() {
    let f = set_up();
    assert_eq!("foo", f.container.get_alias("bar").unwrap().to_string());
}

#[test]
fn testBindingsShouldBeResolved() {
    let f = set_up();
    let bindings = f.container.get_definition("foo").unwrap().bindings();
    let binding = BoundArgument::from_value(bindings.get(&Key::from("$baz")).unwrap()).unwrap();
    let bound_value = binding.values().get(&Key::Int(0)).cloned().unwrap();

    assert_eq!(
        f.container
            .get_parameter_bag()
            .resolve_value(&Value::from("%env(BAZ)%"), &[])
            .unwrap(),
        bound_value
    );
}

#[test]
fn testParameterNotFoundExceptionsIsThrown() {
    let mut container = ContainerBuilder::new();
    let definition = container.register("baz_service_id", None).unwrap();
    definition.set_argument(0, "%non_existent_param%");

    let mut pass = ResolveParameterPlaceHoldersPass::default();
    let e = pass.process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "The service \"baz_service_id\" has a dependency on a non-existent parameter \"non_existent_param\".",
        e.message()
    );
}

#[test]
fn testParameterNotFoundExceptionsIsNotThrown() {
    let mut container = ContainerBuilder::new();
    let definition = container.register("baz_service_id", None).unwrap();
    definition.set_argument(0, "%non_existent_param%");

    let mut pass = ResolveParameterPlaceHoldersPass::new(true, false);
    pass.process(&mut container).unwrap();

    assert_eq!(1, definition.errors().len());
}

#[test]
fn testOnlyProxyTagIsResolved() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("a_param", "here_you_go").unwrap();
    let definition = container.register("def", None).unwrap();
    definition.add_tag("foo", php_array!["bar" => "%a_param%"]);
    definition.add_tag("proxy", php_array!["interface" => "%a_param%"]);

    let mut pass = ResolveParameterPlaceHoldersPass::new(true, false);
    pass.process(&mut container).unwrap();

    let expected: PhpArray = php_array![
        "foo" => php_array![php_array!["bar" => "%a_param%"]],
        "proxy" => php_array![php_array!["interface" => "here_you_go"]]
    ];
    assert_eq!(expected, definition.tags());
}

//! Tests/Compiler/ResolveNamedArgumentsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture classes (`NamedArgumentsDummy`,
//! `SimilarArgumentsDummy`, `NamedArgumentsVariadicsDummy`,
//! `FactoryDummyWithoutReturnTypes`, `TestDefinition1`,
//! `CaseSensitiveClass`, PSR-11's `ContainerInterface` and the test
//! file's `NoConstructor`) are class metadata.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveNamedArgumentsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::object::StdClass;
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveNamedArgumentsPass};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, Reference, Result};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const PSR_CONTAINER: &str = "Psr\\Container\\ContainerInterface";

fn fixture(name: &str) -> String {
    format!("{FIXTURES}{name}")
}

fn no_constructor() -> String {
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NoConstructor".to_string()
}

fn not_exist() -> String {
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\NotExist".to_string()
}

fn param(name: &str, type_: Option<&str>) -> ParameterMetadata {
    ParameterMetadata::new(name, type_)
}

fn registry() -> Arc<ClassRegistry> {
    let case_sensitive = fixture("CaseSensitiveClass");
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class("stdClass"));
    registry.add_class(ClassMetadata::interface(PSR_CONTAINER));
    registry.add_class(
        ClassMetadata::class(&case_sensitive).with_method(
            MethodMetadata::public("__construct")
                .with_parameters(vec![param("identifier", None).with_default(Value::Null)]),
        ),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("NamedArgumentsDummy"))
            .with_method(MethodMetadata::public("__construct").with_parameters(vec![
                param("c", Some(&case_sensitive)),
                param("apiKey", None),
                param("hostName", None),
                param("container", Some(PSR_CONTAINER)),
                param("objects", Some("iterable")),
            ]))
            .with_method(
                MethodMetadata::public("setApiKey").with_parameters(vec![param("apiKey", None)]),
            )
            .with_method(
                MethodMetadata::public("setSensitiveClass")
                    .with_parameters(vec![param("c", Some(&case_sensitive))]),
            )
            .with_method(
                MethodMetadata::public("setAnotherC").with_parameters(vec![param("c", None)]),
            ),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("SimilarArgumentsDummy")).with_method(
            MethodMetadata::public("__construct").with_parameters(vec![
                param("class1", Some(&case_sensitive)),
                param("token", None),
                param("class2", Some(&case_sensitive)),
            ]),
        ),
    );
    let mut variadics = param("variadics", None);
    variadics.is_variadic = true;
    variadics.is_optional = true;
    registry.add_class(
        ClassMetadata::class(&fixture("NamedArgumentsVariadicsDummy")).with_method(
            MethodMetadata::public("__construct")
                .with_parameters(vec![param("class", Some("stdClass")), variadics]),
        ),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("FactoryDummyWithoutReturnTypes"))
            .with_method(MethodMetadata::public("createTestDefinition1")),
    );
    registry.add_class(ClassMetadata::class(
        "Symfony\\Component\\DependencyInjection\\Definition",
    ));
    registry.add_class(
        ClassMetadata::class(&fixture("TestDefinition1"))
            .with_parent("Symfony\\Component\\DependencyInjection\\Definition"),
    );
    registry.add_class(
        ClassMetadata::class(&no_constructor()).with_method(
            MethodMetadata::public("create")
                .with_static(true)
                .with_parameters(vec![param("apiKey", None)]),
        ),
    );
    Arc::new(registry)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(registry());
    container
}

fn process(container: &mut ContainerBuilder) -> Result<()> {
    let mut pass = ResolveNamedArgumentsPass::new();
    pass.process(container)
}

#[test]
fn testProcess() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let foo = Value::from(Reference::new("foo"));
    definition.set_arguments(php_array![
        2 => "http://api.example.com",
        "$apiKey" => "123",
        0 => foo.clone(),
    ]);
    definition
        .add_method_call("setApiKey", php_array!["$apiKey" => "123"], false)
        .unwrap();

    process(&mut container).unwrap();

    assert_eq!(
        php_array![0 => foo, 1 => "123", 2 => "http://api.example.com"],
        definition.arguments()
    );
    assert_eq!(
        php_array![php_array![
            Value::from("setApiKey"),
            Value::from(php_array!["123"])
        ]],
        definition.method_calls()
    );
}

#[test]
fn testWithFactory() {
    let mut container = container();

    container
        .register("factory", Some(&no_constructor()))
        .unwrap();
    let definition = container.register("foo", Some(&no_constructor())).unwrap();
    definition
        .set_factory(php_array![Reference::new("factory"), "create"])
        .set_arguments(php_array!["$apiKey" => "123"]);

    process(&mut container).unwrap();

    assert_eq!(php_array![0 => "123"], definition.arguments());
}

#[test]
fn testClassNull() {
    let mut container = container();

    let definition = container
        .register(&fixture("NamedArgumentsDummy"), None)
        .unwrap();
    definition.set_arguments(php_array!["$apiKey" => "123"]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
}

#[test]
fn testClassNotExist() {
    let mut container = container();

    let definition = container
        .register(&not_exist(), Some(&not_exist()))
        .unwrap();
    definition.set_arguments(php_array!["$apiKey" => "123"]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
}

#[test]
fn testClassNoConstructor() {
    let mut container = container();

    let definition = container
        .register(&no_constructor(), Some(&no_constructor()))
        .unwrap();
    definition.set_arguments(php_array!["$apiKey" => "123"]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
}

#[test]
fn testArgumentNotFound() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_arguments(php_array!["$notFound" => "123"]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Invalid service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NamedArgumentsDummy\": method \"__construct()\" has no argument named \"$notFound\". Check your service definition.",
        e.message()
    );
}

#[test]
fn testCorrectMethodReportedInException() {
    let mut container = container();

    let factory = fixture("FactoryDummyWithoutReturnTypes");
    container.register(&factory, Some(&factory)).unwrap();

    let class = fixture("TestDefinition1");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_factory(php_array![factory.as_str(), "createTestDefinition1"]);
    definition.set_arguments(php_array!["$notFound" => "123"]);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Invalid service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\TestDefinition1\": method \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\FactoryDummyWithoutReturnTypes::createTestDefinition1()\" has no argument named \"$notFound\". Check your service definition.",
        e.message()
    );
}

#[test]
fn testTypedArgument() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let foo = Value::from(Reference::new("foo"));
    let mut arguments = php_array!["$apiKey" => "123"];
    arguments.insert(fixture("CaseSensitiveClass"), foo.clone());
    definition.set_arguments(arguments);

    process(&mut container).unwrap();

    assert_eq!(php_array![foo, Value::from("123")], definition.arguments());
}

#[test]
fn testTypedArgumentWithMissingDollar() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_argument("apiKey", "123");

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Invalid service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NamedArgumentsDummy\": did you forget to add the \"$\" prefix to argument \"apiKey\"?",
        e.message()
    );
}

#[test]
fn testInterfaceTypedArgument() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let expected = Value::from(Reference::new("foo"));
    definition.set_argument(PSR_CONTAINER, expected.clone());

    process(&mut container).unwrap();

    assert_eq!(expected, definition.argument("container").unwrap());
}

#[test]
fn testResolvesMultipleArgumentsOfTheSameType() {
    let mut container = container();

    let class = fixture("SimilarArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let foo = Value::from(Reference::new("foo"));
    let mut arguments = PhpArray::new();
    arguments.insert(fixture("CaseSensitiveClass"), foo.clone());
    arguments.insert("$token", Value::from("qwerty"));
    definition.set_arguments(arguments);

    process(&mut container).unwrap();

    assert_eq!(
        php_array![foo.clone(), Value::from("qwerty"), foo],
        definition.arguments()
    );
}

#[test]
fn testResolvePrioritizeNamedOverType() {
    let mut container = container();

    let class = fixture("SimilarArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let foo = Value::from(Reference::new("foo"));
    let bar = Value::from(Reference::new("bar"));
    let mut arguments = PhpArray::new();
    arguments.insert(fixture("CaseSensitiveClass"), foo.clone());
    arguments.insert("$token", Value::from("qwerty"));
    arguments.insert("$class1", bar.clone());
    definition.set_arguments(arguments);

    process(&mut container).unwrap();

    assert_eq!(
        php_array![bar, Value::from("qwerty"), foo],
        definition.arguments()
    );
}

#[test]
fn testVariadics() {
    let mut container = container();

    let class = fixture("NamedArgumentsVariadicsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    let std_class = StdClass::object(PhpArray::new());
    let foo = Value::from(Reference::new("foo"));
    let bar = Value::from(Reference::new("bar"));
    let baz = Value::from(Reference::new("baz"));
    definition.set_arguments(php_array![
        "$class" => std_class.clone(),
        "$variadics" => php_array![foo.clone(), bar.clone(), baz.clone()],
    ]);

    process(&mut container).unwrap();

    assert_eq!(
        php_array![0 => std_class, 1 => foo, 2 => bar, 3 => baz],
        definition.arguments()
    );
}

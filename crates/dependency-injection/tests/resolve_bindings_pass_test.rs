//! Tests/Compiler/ResolveBindingsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's (`expectExceptionMessage()` is a substring check); the data
//! provider is a loop. Fixture classes, with their `#[Target]`
//! attributes, are class metadata; `FooUnitEnum::BAR` is an enum case
//! object. `assertEquals()` on argument objects compares what they hold.
//!
//! testIterableBindingTypehint's
//! autoloader (that `iterable` is never looked up as a class) has no
//! counterpart: the class registry is never asked for it either.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveBindingsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;

use php_value::object::EnumCase;
use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::{
    AbstractArgument, BoundArgument, INSTANCEOF_BINDING, IteratorArgument, ServiceLocatorArgument,
};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassKind, ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{
    AutowireRequiredMethodsPass, CompilerPassInterface, DefinitionErrorExceptionPass,
    ResolveBindingsPass,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference, Result,
};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const PSR_CONTAINER: &str = "Psr\\Container\\ContainerInterface";
const TARGET: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Target";
const REQUIRED: &str = "Symfony\\Contracts\\Service\\Attribute\\Required";
const TARGETED_BINDINGS_SERVICE: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\TargetedBindingsService";

fn fixture(name: &str) -> String {
    format!("{FIXTURES}{name}")
}

fn param(name: &str, type_: Option<&str>) -> ParameterMetadata {
    ParameterMetadata::new(name, type_)
}

fn target(parameter: ParameterMetadata, name: &str) -> ParameterMetadata {
    let mut parameter = parameter;
    parameter.attributes.push(AttributeMetadata {
        class: TARGET.to_string(),
        arguments: php_array![name],
    });
    parameter
}

fn constructor(parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    MethodMetadata::public("__construct").with_parameters(parameters)
}

fn registry() -> Arc<ClassRegistry> {
    let case_sensitive = fixture("CaseSensitiveClass");
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class("stdClass"));
    registry.add_class(ClassMetadata::interface(PSR_CONTAINER));
    registry.add_class(
        ClassMetadata::class(&case_sensitive).with_method(constructor(vec![
            param("identifier", None).with_default(Value::Null),
        ])),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("NamedArgumentsDummy"))
            .with_method(constructor(vec![
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
    let mut enum_class = ClassMetadata::class(&fixture("FooUnitEnum"));
    enum_class.kind = ClassKind::Enum;
    enum_class.interfaces = vec!["UnitEnum".to_string()];
    registry.add_class(ClassMetadata::interface("UnitEnum"));
    registry.add_class(enum_class);
    registry.add_class(
        ClassMetadata::class(&fixture("NamedEnumArgumentDummy")).with_method(constructor(vec![
            param("bar", Some(&fixture("FooUnitEnum"))),
        ])),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("NamedIterableArgumentDummy"))
            .with_method(constructor(vec![param("items", Some("iterable"))])),
    );
    registry.add_class(ClassMetadata::class(&fixture("ParentNotExists")).with_parent("NotExists"));
    registry.add_class(ClassMetadata::interface(&fixture("BarInterface")));
    registry.add_class(
        ClassMetadata::class(&fixture("WithTarget")).with_method(constructor(vec![target(
            param("bar", Some(&fixture("BarInterface"))),
            "image.storage",
        )])),
    );
    registry.add_class(
        ClassMetadata::class(&fixture("UntypedWithTarget"))
            .with_method(constructor(vec![target(param("key", None), "apiKey")])),
    );
    registry.add_class(
        ClassMetadata::class(TARGETED_BINDINGS_SERVICE).with_method(constructor(vec![
            target(param("arg1", None), "targetName"),
            target(param("variableName", None), "unusedTarget"),
            target(param("commonName", None), "commonName"),
        ])),
    );
    let mut set_default_locale = MethodMetadata::public("setDefaultLocale")
        .with_parameters(vec![param("defaultLocale", None)]);
    set_default_locale.attributes.push(AttributeMetadata {
        class: REQUIRED.to_string(),
        arguments: PhpArray::new(),
    });
    registry
        .add_class(ClassMetadata::class(&fixture("ScalarSetter")).with_method(set_default_locale));
    Arc::new(registry)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(registry());
    container
}

fn process(container: &mut ContainerBuilder) -> Result<()> {
    let mut pass = ResolveBindingsPass::new();
    pass.process(container)
}

fn bound(value: impl Into<Value>) -> Value {
    BoundArgument::new(value.into(), true, 0, None).to_value()
}

fn reference_id(value: &Value) -> String {
    Reference::from_value(value).unwrap().id().to_string()
}

fn assert_error(result: Result<()>, kind: &ErrorKind, message: &str) {
    let e = result.unwrap_err();
    assert!(e.is(kind), "{e:?}");
    assert!(e.message().contains(message), "{}", e.message());
}

#[test]
fn testProcess() {
    let mut container = container();

    let mut bindings = PhpArray::new();
    bindings.insert(fixture("CaseSensitiveClass"), bound(Reference::new("foo")));
    bindings.insert(
        "Psr\\Container\\ContainerInterface $container",
        BoundArgument::new(
            ServiceLocatorArgument::new(PhpArray::new()).to_value(),
            true,
            INSTANCEOF_BINDING,
            None,
        )
        .to_value(),
    );
    bindings.insert(
        "iterable $objects",
        BoundArgument::new(
            IteratorArgument::tagged("tag.name", None, false, Vec::new(), true).to_value(),
            true,
            INSTANCEOF_BINDING,
            None,
        )
        .to_value(),
    );

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_arguments(php_array![1 => "123"]);
    definition
        .add_method_call("setSensitiveClass", PhpArray::new(), false)
        .unwrap();
    definition.set_bindings(&bindings);

    container
        .register("foo", Some(&fixture("CaseSensitiveClass")))
        .unwrap()
        .set_bindings(&bindings);

    process(&mut container).unwrap();

    let arguments = definition.arguments();
    let keys: Vec<Key> = arguments.keys().cloned().collect();
    assert_eq!(
        vec![Key::Int(0), Key::Int(1), Key::Int(3), Key::Int(4)],
        keys
    );
    assert_eq!("foo", reference_id(arguments.get(&Key::Int(0)).unwrap()));
    assert_eq!(Some(&Value::from("123")), arguments.get(&Key::Int(1)));
    let locator = ServiceLocatorArgument::from_value(arguments.get(&Key::Int(3)).unwrap()).unwrap();
    assert_eq!(PhpArray::new(), locator.values());
    let tagged = IteratorArgument::from_value(arguments.get(&Key::Int(4)).unwrap()).unwrap();
    assert_eq!(Some("tag.name".to_string()), tagged.tag());

    let calls = definition.method_calls();
    assert_eq!(1, calls.len());
    let call = calls.get(&Key::Int(0)).unwrap().as_array().unwrap();
    assert_eq!(
        Some(&Value::from("setSensitiveClass")),
        call.get(&Key::Int(0))
    );
    let call_arguments = call.get(&Key::Int(1)).unwrap().as_array().unwrap();
    assert_eq!(1, call_arguments.len());
    assert_eq!(
        "foo",
        reference_id(call_arguments.get(&Key::Int(0)).unwrap())
    );
}

#[test]
fn testProcessEnum() {
    let mut container = container();

    let bar = Value::Object(EnumCase::new(fixture("FooUnitEnum"), "BAR", None).object());
    let mut bindings = PhpArray::new();
    bindings.insert(
        format!("{} $bar", fixture("FooUnitEnum")),
        bound(bar.clone()),
    );

    let class = fixture("NamedEnumArgumentDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_bindings(&bindings);

    process(&mut container).unwrap();

    assert_eq!(php_array![bar], definition.arguments());
}

#[test]
fn testUnusedBinding() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_bindings(&php_array!["$quz" => "123"]);

    assert_error(
        process(&mut container),
        &ErrorKind::InvalidArgument,
        "A binding is configured for an argument named \"$quz\" for service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NamedArgumentsDummy\", but no corresponding argument has been found. It may be unused and should be removed, or it may have a typo.",
    );
}

#[test]
fn testMissingParent() {
    let mut container = container();

    let class = fixture("ParentNotExists");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_bindings(&php_array!["$quz" => "123"]);

    assert_error(
        process(&mut container),
        &ErrorKind::InvalidArgument,
        "A binding is configured for an argument named \"$quz\" for service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\ParentNotExists\", but no corresponding argument has been found. It may be unused and should be removed, or it may have a typo.",
    );
}

#[test]
fn testTypedReferenceSupport() {
    let mut container = container();
    let case_sensitive = fixture("CaseSensitiveClass");

    let mut bindings = PhpArray::new();
    bindings.insert(case_sensitive.as_str(), bound(Reference::new("foo")));
    bindings.insert(format!("{case_sensitive} $c"), bound(Reference::new("bar")));

    // Explicit service id
    let class = fixture("NamedArgumentsDummy");
    let definition1 = container.register("def1", Some(&class)).unwrap();
    let typed_ref = Value::from(Reference::typed(
        "bar",
        case_sensitive.as_str(),
        InvalidBehavior::Exception,
        None,
        PhpArray::new(),
    ));
    definition1.add_argument(typed_ref.clone());
    definition1.set_bindings(&bindings);

    let definition2 = container.register("def2", Some(&class)).unwrap();
    definition2.add_argument(Reference::typed(
        case_sensitive.as_str(),
        case_sensitive.as_str(),
        InvalidBehavior::Exception,
        None,
        PhpArray::new(),
    ));
    definition2.set_bindings(&bindings);

    let definition3 = container.register("def3", Some(&class)).unwrap();
    definition3.add_argument(Reference::typed(
        case_sensitive.as_str(),
        case_sensitive.as_str(),
        InvalidBehavior::Exception,
        Some("c".to_string()),
        PhpArray::new(),
    ));
    definition3.set_bindings(&bindings);

    process(&mut container).unwrap();

    assert_eq!(
        php_array![typed_ref],
        container.get_definition("def1").unwrap().arguments()
    );
    let first = |id: &str| {
        container
            .get_definition(id)
            .unwrap()
            .arguments()
            .get(&Key::Int(0))
            .cloned()
            .unwrap()
    };
    assert_eq!(
        Some(&Reference::new("foo")),
        Reference::from_value(&first("def2"))
    );
    assert_eq!(
        Some(&Reference::new("bar")),
        Reference::from_value(&first("def3"))
    );
    assert_eq!(
        1,
        container.get_definition("def2").unwrap().arguments().len()
    );
    assert_eq!(
        1,
        container.get_definition("def3").unwrap().arguments().len()
    );
}

#[test]
fn testScalarSetterAttribute() {
    let mut container = container();

    let definition = container
        .autowire("foo", Some(&fixture("ScalarSetter")))
        .unwrap();
    definition.set_bindings(&php_array!["$defaultLocale" => "fr"]);

    AutowireRequiredMethodsPass::new()
        .process(&mut container)
        .unwrap();
    ResolveBindingsPass::new().process(&mut container).unwrap();

    let expected = php_array![php_array!["setDefaultLocale", php_array!["fr"]]];
    let actual = definition.method_calls();
    assert!(
        php_value::compare::loose_equals(&Value::Array(expected), &Value::Array(actual.clone())),
        "{actual:?}"
    );
}

#[test]
fn testWithNonExistingSetterAndBinding() {
    let mut container = container();

    let logger = Definition::with_class("logger");
    logger.set_factory("logger");
    let bindings = php_array!["$c" => logger.to_value()];

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition
        .add_method_call("setLogger", PhpArray::new(), false)
        .unwrap();
    definition.set_bindings(&bindings);

    assert_error(
        process(&mut container),
        &ErrorKind::Runtime,
        "Invalid service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NamedArgumentsDummy\": method \"setLogger()\" does not exist.",
    );
}

#[test]
fn testSyntheticServiceWithBind() {
    let mut container = container();
    let argument = BoundArgument::new(Value::from("bar"), true, 0, None);

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("synthetic.service"));

    let mut bindings = PhpArray::new();
    bindings.insert("$apiKey", argument.to_value());
    container
        .register("synthetic.service", None)
        .unwrap()
        .set_synthetic(true)
        .set_bindings(&bindings);

    let class = fixture("NamedArgumentsDummy");
    container
        .register(&class, Some(&class))
        .unwrap()
        .set_bindings(&bindings);

    ResolveBindingsPass::new().process(&mut container).unwrap();
    DefinitionErrorExceptionPass::new()
        .process(&mut container)
        .unwrap();

    let mut expected = PhpArray::new();
    expected.insert(1, Value::from("bar"));
    assert!(Value::Array(expected).identical(&Value::Array(
        container.get_definition(&class).unwrap().arguments()
    )));
}

#[test]
fn testEmptyBindingTypehint() {
    let mut container = container();
    let bindings = php_array!["string $apiKey" => bound("foo")];
    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_bindings(&bindings);

    assert_error(
        process(&mut container),
        &ErrorKind::InvalidArgument,
        "Did you forget to add the type \"string\" to argument \"$apiKey\" of method \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\NamedArgumentsDummy::__construct()\"?",
    );
}

fn provide_empty_binding_typehint_with_multiple_bindings_cases()
-> Vec<(&'static str, &'static str, String, PhpArray)> {
    vec![
        // multiple bindings with matching first
        (
            "string",
            "apiKey",
            fixture("NamedArgumentsDummy"),
            php_array!["string $apiKey" => bound("foo"), "int $hostName" => bound(80)],
        ),
        // multiple bindings with matching last
        (
            "string",
            "apiKey",
            fixture("NamedArgumentsDummy"),
            php_array!["int $hostName" => bound(80), "string $apiKey" => bound("foo")],
        ),
        // three bindings with matching in the middle
        (
            "string",
            "apiKey",
            fixture("NamedArgumentsDummy"),
            php_array![
                "bool $debug" => bound(true),
                "string $apiKey" => bound("foo"),
                "int $hostName" => bound(80)
            ],
        ),
        // class type binding
        (
            "Psr\\Log\\LoggerInterface",
            "apiKey",
            fixture("NamedArgumentsDummy"),
            php_array![
                "int $hostName" => bound(80),
                "Psr\\Log\\LoggerInterface $apiKey" => bound(Reference::new("logger"))
            ],
        ),
        // binding matching parameter name with #[Target] attribute
        (
            "string",
            "key",
            fixture("UntypedWithTarget"),
            php_array!["int $other" => bound(80), "string $key" => bound("secret")],
        ),
    ]
}

#[test]
fn testEmptyBindingTypehintWithMultipleBindings() {
    for (expected_type, expected_argument, class, bindings) in
        provide_empty_binding_typehint_with_multiple_bindings_cases()
    {
        let mut container = container();
        let definition = container.register(&class, Some(&class)).unwrap();
        definition.set_bindings(&bindings);

        assert_error(
            process(&mut container),
            &ErrorKind::InvalidArgument,
            &format!(
                "Did you forget to add the type \"{expected_type}\" to argument \"${expected_argument}\""
            ),
        );
    }
}

#[test]
fn testIterableBindingTypehint() {
    let mut container = container();
    let definition = container
        .register("bar", Some(&fixture("NamedIterableArgumentDummy")))
        .unwrap();
    definition.set_bindings(&php_array![
        "iterable $items" => IteratorArgument::tagged("foo", None, false, Vec::new(), true).to_value()
    ]);
    process(&mut container).unwrap();

    let argument = container
        .get_definition("bar")
        .unwrap()
        .argument(0)
        .unwrap();
    assert!(
        IteratorArgument::from_value(&argument).is_some_and(|a| a.is_tagged()),
        "assertInstanceOf(TaggedIteratorArgument::class)"
    );
}

#[test]
fn testBindWithTarget() {
    let mut container = container();

    let mut bindings = PhpArray::new();
    bindings.insert(
        format!("{} $imageStorage", fixture("BarInterface")),
        Value::from(Reference::new("bar")),
    );
    container
        .register("with_target", Some(&fixture("WithTarget")))
        .unwrap()
        .set_bindings(&bindings);

    process(&mut container).unwrap();

    assert_eq!(
        "bar",
        reference_id(
            &container
                .get_definition("with_target")
                .unwrap()
                .argument(0)
                .unwrap()
        )
    );
}

#[test]
fn testBindWithNamedArgs() {
    let mut container = container();

    let bindings = php_array!["$apiKey" => bound("K")];

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_arguments(php_array!["c" => "C", "hostName" => "H"]);
    definition.set_bindings(&bindings);

    process(&mut container).unwrap();

    assert_eq!(php_array!["C", "K", "H"], definition.arguments());
}

#[test]
fn testAbstractArg() {
    let mut container = container();

    let class = fixture("NamedArgumentsDummy");
    let definition = container.register(&class, Some(&class)).unwrap();
    definition.set_arguments(php_array![
        0 => AbstractArgument::new("").to_value(),
        "apiKey" => AbstractArgument::new("").to_value()
    ]);
    definition.set_bindings(&php_array!["$c" => bound("C"), "$apiKey" => bound("K")]);

    process(&mut container).unwrap();

    assert_eq!(php_array!["C", "K"], definition.arguments());
}

#[test]
fn testBindingsOnTargetedArguments() {
    let mut container = container();
    container
        .register("service", Some(TARGETED_BINDINGS_SERVICE))
        .unwrap()
        .set_autowired(true)
        .set_bindings(&php_array![
            "$targetName" => "bound_via_target",
            "$variableName" => "bound_via_variable",
            "$commonName" => "bound_via_common_name"
        ]);

    process(&mut container).unwrap();

    let definition = container.get_definition("service").unwrap();

    // 1. Priority: Binding matches the #[Target] name
    assert_eq!(
        Value::from("bound_via_target"),
        definition.argument(0).unwrap()
    );

    // 2. Fallback: Binding matches the variable name (Target name 'unusedTarget' has no binding)
    assert_eq!(
        Value::from("bound_via_variable"),
        definition.argument(1).unwrap()
    );

    // 3. Equality: Target name and variable name are identical
    assert_eq!(
        Value::from("bound_via_common_name"),
        definition.argument(2).unwrap()
    );
}

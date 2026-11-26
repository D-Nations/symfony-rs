//! Tests/Compiler/RegisterAutoconfigureAttributesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! The fixture classes of Tests/Fixtures are given as class metadata,
//! their attributes as attribute metadata; `realpath()` of a fixture file
//! is the file the metadata names. `assertEquals()` on definitions is
//! PHPUnit's object comparison (class and properties).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RegisterAutoconfigureAttributesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::too_many_lines)]

use std::sync::Arc;

use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::argument::{BoundArgument, INSTANCEOF_BINDING};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry,
};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, RegisterAutoconfigureAttributesPass,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind, Reference};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\";

fn f(short: &str) -> String {
    format!("{FIXTURES}{short}")
}

fn file(short: &str) -> String {
    format!("/symfony/dependency-injection/Tests/Fixtures/{short}.php")
}

fn attr(short: &str, arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: format!("{ATTRIBUTE}{short}"),
        arguments,
    }
}

fn class(short: &str, attributes: Vec<AttributeMetadata>) -> ClassMetadata {
    let mut class = ClassMetadata::class(&f(short)).with_file(&file(short));
    class.attributes = attributes;
    class
}

fn list(values: Vec<Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    for short in ["Autoconfigure", "Lazy"] {
        r.add_class(ClassMetadata::class(&format!("{ATTRIBUTE}{short}")));
    }
    r.add_class(
        ClassMetadata::class(&format!("{ATTRIBUTE}AutoconfigureTag"))
            .with_parent(&format!("{ATTRIBUTE}Autoconfigure")),
    );
    r.add_class(
        ClassMetadata::class(&format!("{ATTRIBUTE}AutoconfigureResourceTag"))
            .with_parent(&format!("{ATTRIBUTE}Autoconfigure")),
    );

    r.add_class(class(
        "AutoconfigureAttributed",
        vec![attr(
            "Autoconfigure",
            php_array![
                "lazy" => true,
                "public" => true,
                "autowire" => true,
                "shared" => true,
                "properties" => php_array!["bar" => "baz"],
                "configurator" => "@bla",
                "tags" => list(vec![
                    Value::from("a_tag"),
                    Value::from(php_array!["another_tag" => php_array!["attr" => 234_i64]]),
                ]),
                "calls" => list(vec![Value::from(
                    php_array!["setBar" => php_array![2_i64, 3_i64]]
                )]),
                "bind" => php_array!["$bar" => 1_i64],
                "constructor" => "create",
            ],
        )],
    ));
    let mut interface = ClassMetadata::interface(&f("AutoconfiguredInterface"))
        .with_file(&file("AutoconfiguredInterface"));
    interface.attributes = vec![attr(
        "AutoconfigureTag",
        php_array!["attributes" => php_array!["foo" => 123_i64]],
    )];
    r.add_class(interface);
    r.add_class(class(
        "AutoconfigureRepeated",
        vec![
            attr(
                "Autoconfigure",
                php_array!["public" => true, "shared" => false],
            ),
            attr("Autoconfigure", php_array!["lazy" => true]),
        ],
    ));
    r.add_class(class(
        "AutoconfigureRepeatedOverwrite",
        vec![
            attr(
                "Autoconfigure",
                php_array!["public" => true, "shared" => false],
            ),
            attr(
                "Autoconfigure",
                php_array!["lazy" => true, "shared" => true, "public" => false],
            ),
        ],
    ));
    r.add_class(class(
        "AutoconfigureRepeatedTag",
        vec![
            attr(
                "AutoconfigureTag",
                php_array!["foo", php_array!["priority" => 2_i64]],
            ),
            attr("AutoconfigureTag", php_array!["bar"]),
        ],
    ));
    r.add_class(class(
        "AutoconfigureRepeatedCalls",
        vec![
            attr(
                "Autoconfigure",
                php_array!["calls" => list(vec![Value::from(php_array!["setBar", php_array!["arg2"]])])],
            ),
            attr(
                "Autoconfigure",
                php_array!["calls" => list(vec![Value::from(php_array!["setFoo", php_array!["arg1"]])])],
            ),
        ],
    ));
    r.add_class(class(
        "AutoconfigureRepeatedBindings",
        vec![
            attr(
                "Autoconfigure",
                php_array!["bind" => php_array!["$arg" => "foo"]],
            ),
            attr(
                "Autoconfigure",
                php_array!["bind" => php_array!["$arg" => "bar"]],
            ),
        ],
    ));
    r.add_class(class(
        "AutoconfigureRepeatedProperties",
        vec![
            attr(
                "Autoconfigure",
                php_array!["properties" => php_array![
                    "$replaced" => "to be replaced",
                    "$bar" => "existing to be replaced",
                ]],
            ),
            attr(
                "Autoconfigure",
                php_array!["properties" => php_array!["$foo" => "bar", "$bar" => "baz"]],
            ),
        ],
    ));
    r.add_class(class("ParentNotExists", vec![]).with_parent("NotExists"));
    r.add_class(class(
        "StaticConstructorAutoconfigure",
        vec![attr(
            "Autoconfigure",
            php_array!["bind" => php_array!["$foo" => "foo"], "constructor" => "create"],
        )],
    ));
    r.add_class(class("LazyLoaded", vec![attr("Lazy", PhpArray::new())]));
    r.add_class(class(
        "LazyAutoconfigured",
        vec![
            attr("Lazy", PhpArray::new()),
            attr("Autoconfigure", php_array!["lazy" => true]),
        ],
    ));
    r.add_class(class(
        "MultipleAutoconfigureAttributed",
        vec![
            attr("Autoconfigure", php_array!["tags" => php_array!["foo"]]),
            attr("Autoconfigure", php_array!["tags" => php_array!["bar"]]),
        ],
    ));
    r.add_class(class(
        "AutoconfigureResourceTagsAttributed",
        vec![
            attr(
                "AutoconfigureResourceTag",
                php_array!["my.tag", php_array!["foo" => "bar"]],
            ),
            attr("AutoconfigureResourceTag", php_array!["another.tag"]),
        ],
    ));
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

fn bound(value: Value, short: &str) -> Value {
    Value::from(BoundArgument::new(
        value,
        false,
        INSTANCEOF_BINDING,
        Some(file(short)),
    ))
}

#[track_caller]
fn assert_instanceof(expected: &[(String, Definition)], container: &ContainerBuilder) {
    let mut e = PhpArray::new();
    for (class, definition) in expected {
        e.insert(class.as_str(), definition.to_value());
    }
    let mut a = PhpArray::new();
    for (class, definition) in container.get_autoconfigured_instanceof().iter() {
        a.insert(class, definition.to_value());
    }
    assert!(
        phpunit_equals(&Value::Array(e.clone()), &Value::Array(a.clone())),
        "expected {e:#?}\nactual {a:#?}"
    );
}

fn run(container: &mut ContainerBuilder) {
    RegisterAutoconfigureAttributesPass
        .process(container)
        .unwrap();
}

#[test]
fn testProcess() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureAttributed")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let argument = bound(Value::Int(1), "AutoconfigureAttributed");

    let expected = Definition::child("");
    expected
        .set_lazy(true)
        .set_public(true)
        .set_autowired(true)
        .set_shared(true)
        .set_properties(php_array!["bar" => "baz"])
        .set_configurator(Reference::new("bla"))
        .add_tag("a_tag", PhpArray::new())
        .add_tag("another_tag", php_array!["attr" => 234_i64]);
    expected
        .add_method_call("setBar", php_array![2_i64, 3_i64], false)
        .unwrap()
        .set_bindings(&php_array!["$bar" => argument])
        .set_factory(php_array![Value::Null, "create"]);
    assert_instanceof(&[(f("AutoconfigureAttributed"), expected)], &container);
}

#[test]
fn testIgnoreAttribute() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureAttributed")))
        .unwrap()
        .add_tag("container.ignore_attributes", PhpArray::new())
        .set_autoconfigured(true);

    run(&mut container);

    assert!(container.get_autoconfigured_instanceof().is_empty());
}

#[test]
fn testAutoconfiguredTag() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfiguredInterface")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.add_tag(&f("AutoconfiguredInterface"), php_array!["foo" => 123_i64]);
    assert_instanceof(&[(f("AutoconfiguredInterface"), expected)], &container);
}

#[test]
fn testAutoconfiguredRepeated() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeated")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.set_lazy(true).set_public(true).set_shared(false);

    assert_instanceof(&[(f("AutoconfigureRepeated"), expected)], &container);
}

#[test]
fn testAutoconfiguredRepeatedOverwrite() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeatedOverwrite")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.set_lazy(true).set_public(false).set_shared(true);

    assert_instanceof(
        &[(f("AutoconfigureRepeatedOverwrite"), expected)],
        &container,
    );
}

#[test]
fn testAutoconfiguredRepeatedTag() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeatedTag")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected
        .add_tag("foo", php_array!["priority" => 2_i64])
        .add_tag("bar", PhpArray::new());

    assert_instanceof(&[(f("AutoconfigureRepeatedTag"), expected)], &container);
}

#[test]
fn testAutoconfiguredRepeatedCalls() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeatedCalls")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected
        .add_method_call("setBar", php_array!["arg2"], false)
        .unwrap()
        .add_method_call("setFoo", php_array!["arg1"], false)
        .unwrap();

    assert_instanceof(&[(f("AutoconfigureRepeatedCalls"), expected)], &container);
}

#[test]
fn testAutoconfiguredRepeatedBindingsOverwrite() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeatedBindings")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.set_bindings(&php_array![
        "$arg" => bound(Value::from("bar"), "AutoconfigureRepeatedBindings")
    ]);

    assert_instanceof(
        &[(f("AutoconfigureRepeatedBindings"), expected)],
        &container,
    );
}

#[test]
fn testAutoconfiguredRepeatedPropertiesOverwrite() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureRepeatedProperties")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.set_properties(php_array!["$foo" => "bar", "$bar" => "baz"]);

    assert_instanceof(
        &[(f("AutoconfigureRepeatedProperties"), expected)],
        &container,
    );
}

#[test]
fn testMissingParent() {
    let mut container = container();

    container
        .register(&f("ParentNotExists"), Some(&f("ParentNotExists")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);
}

#[test]
fn testStaticConstructor() {
    let mut container = container();
    container
        .register("foo", Some(&f("StaticConstructorAutoconfigure")))
        .unwrap()
        .set_autoconfigured(true);

    let argument = bound(Value::from("foo"), "StaticConstructorAutoconfigure");

    run(&mut container);

    let expected = Definition::child("");
    expected
        .set_factory(php_array![Value::Null, "create"])
        .set_bindings(&php_array!["$foo" => argument]);
    assert_instanceof(
        &[(f("StaticConstructorAutoconfigure"), expected)],
        &container,
    );
}

#[test]
fn testLazyServiceAttribute() {
    let mut container = container();
    container
        .register("foo", Some(&f("LazyLoaded")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected.set_lazy(true);
    assert_instanceof(&[(f("LazyLoaded"), expected)], &container);
}

#[test]
fn testLazyNotCompatibleWithAutoconfigureAttribute() {
    let mut container = container();
    container
        .register("foo", Some(&f("LazyAutoconfigured")))
        .unwrap()
        .set_autoconfigured(true);

    if let Err(e) = RegisterAutoconfigureAttributesPass.process(&mut container) {
        assert_eq!(&ErrorKind::AutoconfigureFailed, e.kind());
        assert_eq!(
            "Using both attributes #[Lazy] and #[Autoconfigure] on an argument is not allowed; use the \"lazy\" parameter of #[Autoconfigure] instead.",
            e.message()
        );
    }
}

#[test]
fn testMultipleAutoconfigureAllowed() {
    let mut container = container();
    container
        .register("foo", Some(&f("MultipleAutoconfigureAttributed")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected
        .add_tag("foo", PhpArray::new())
        .add_tag("bar", PhpArray::new());
    assert_instanceof(
        &[(f("MultipleAutoconfigureAttributed"), expected)],
        &container,
    );
}

#[test]
fn testAutoconfiguredResourceTags() {
    let mut container = container();
    container
        .register("foo", Some(&f("AutoconfigureResourceTagsAttributed")))
        .unwrap()
        .set_autoconfigured(true);

    run(&mut container);

    let expected = Definition::child("");
    expected
        .add_resource_tag("my.tag", php_array!["foo" => "bar"])
        .add_resource_tag("another.tag", PhpArray::new());

    assert_instanceof(
        &[(f("AutoconfigureResourceTagsAttributed"), expected)],
        &container,
    );
}

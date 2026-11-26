//! Tests/Compiler/AutowirePassTest.php at symfony/dependency-injection
//! 9b34cad8.
//!
//! `AutowirePass` is not ported yet, so its 93 upstream tests are pending.
//! The `AutowireAsDecoratorPass` tests of this file
//! (testAsDecoratorAttribute, testMultipleAsDecoratorAttribute,
//! testAsTagDecoratorAttribute, testNestedAttributes) also need
//! `DecoratorServicePass`, which another port owns. The tests below are
//! extras, not upstream's: they check what `AutowireAsDecoratorPass`
//! alone leaves on the fixtures of those four tests.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AutowirePassTest.php
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry,
};
use symfony_dependency_injection::compiler::{AutowireAsDecoratorPass, CompilerPassInterface};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const AS_DECORATOR: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsDecorator";
const AS_TAG_DECORATOR: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsTagDecorator";

fn c(short: &str) -> String {
    format!("{NS}{short}")
}

fn class(short: &str, attributes: Vec<AttributeMetadata>) -> ClassMetadata {
    let mut class = ClassMetadata::class(&c(short));
    class.attributes = attributes;
    class
}

fn decorator(arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: AS_DECORATOR.to_string(),
        arguments,
    }
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    for short in [
        "AsDecoratorFoo",
        "AsDecoratorMultipleFoo",
        "AsDecoratorMultipleBar",
        "AsTagDecoratorFoo",
        "AsTagDecoratorBar",
    ] {
        r.add_class(class(short, vec![]));
    }
    r.add_class(class(
        "AsDecoratorBar10",
        vec![decorator(
            php_array!["decorates" => c("AsDecoratorFoo"), "priority" => 10_i64],
        )],
    ));
    r.add_class(class(
        "AsDecoratorBaz",
        vec![decorator(
            php_array!["decorates" => "NonExistent", "onInvalid" => 2_i64],
        )],
    ));
    r.add_class(class(
        "AsDecoratorMultiple",
        vec![
            decorator(php_array!["decorates" => c("AsDecoratorMultipleFoo")]),
            decorator(php_array!["decorates" => c("AsDecoratorMultipleBar")]),
        ],
    ));
    r.add_class(class(
        "AsTagDecoratorService",
        vec![AttributeMetadata {
            class: AS_TAG_DECORATOR.to_string(),
            arguments: php_array!["test.tag"],
        }],
    ));
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

#[test]
fn as_decorator_sets_the_decorated_service() {
    let mut container = container();
    container.register(&c("AsDecoratorFoo"), None).unwrap();
    container
        .register(&c("AsDecoratorBar10"), Some(&c("AsDecoratorBar10")))
        .unwrap()
        .set_autowired(true);
    container
        .register(&c("AsDecoratorBaz"), Some(&c("AsDecoratorBaz")))
        .unwrap()
        .set_autowired(true);

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    let bar = container.get_definition(&c("AsDecoratorBar10")).unwrap();
    let decorated = bar.decorated_service().unwrap();
    assert_eq!(c("AsDecoratorFoo"), decorated.id);
    assert_eq!(None, decorated.renamed_id);
    assert_eq!(10, decorated.priority);
    assert_eq!(None, decorated.invalid_behavior);

    let baz = container.get_definition(&c("AsDecoratorBaz")).unwrap();
    let decorated = baz.decorated_service().unwrap();
    assert_eq!("NonExistent", decorated.id);
    assert_eq!(Some(InvalidBehavior::Null), decorated.invalid_behavior);
}

#[test]
fn multiple_as_decorator_clones_the_definition() {
    let mut container = container();
    container
        .register(&c("AsDecoratorMultiple"), Some(&c("AsDecoratorMultiple")))
        .unwrap()
        .set_autowired(true)
        .set_argument(0, Value::from("arg1"));

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(!container.has_definition(&c("AsDecoratorMultiple")));
    for decorated in ["AsDecoratorMultipleFoo", "AsDecoratorMultipleBar"] {
        let id = format!(".decorator.{}.{}", c(decorated), c("AsDecoratorMultiple"));
        let definition = container.get_definition(&id).unwrap();
        assert_eq!(c(decorated), definition.decorated_service().unwrap().id);
        assert_eq!(Some(c("AsDecoratorMultiple")), definition.class());
    }
}

#[test]
fn as_tag_decorator_registers_a_tag_decorator_template() {
    let mut container = container();
    container
        .register(
            &c("AsTagDecoratorService"),
            Some(&c("AsTagDecoratorService")),
        )
        .unwrap()
        .set_autowired(true);

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(!container.has_definition(&c("AsTagDecoratorService")));
    let template = container
        .get_definition(&format!(
            ".tag_decorator.test.tag.{}",
            c("AsTagDecoratorService")
        ))
        .unwrap();
    let tags = template.tags();
    let tag = tags.get(&Key::from("container.tag_decorator")).unwrap();
    let expected = Value::Array(PhpArray::list([Value::Array(
        php_array!["decorates_tag" => "test.tag", "priority" => 0_i64],
    )]));
    assert!(expected.identical(tag), "{tag:?}");
    assert!(template.has_tag("container.excluded"));
}

#[test]
fn ignore_attributes_tag_and_non_autowired_definitions_are_skipped() {
    let mut container = container();
    container
        .register("a", Some(&c("AsDecoratorBar10")))
        .unwrap()
        .set_autowired(true)
        .add_tag("container.ignore_attributes", PhpArray::new());
    container
        .register("b", Some(&c("AsDecoratorBar10")))
        .unwrap();

    AutowireAsDecoratorPass.process(&mut container).unwrap();

    assert!(
        container
            .get_definition("a")
            .unwrap()
            .decorated_service()
            .is_none()
    );
    assert!(
        container
            .get_definition("b")
            .unwrap()
            .decorated_service()
            .is_none()
    );
}

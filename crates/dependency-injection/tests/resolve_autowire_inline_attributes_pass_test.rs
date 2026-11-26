//! Tests/Compiler/ResolveAutowireInlineAttributesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture classes of `autowiring_classes_80.php` are given
//! as class metadata; an object written in an attribute's arguments is the
//! object `new_autowire_attribute()` builds (`#[AutowireInline]`), or a
//! plain object of the class (`new Foo()`).
//!
//! Pending:
//! - testAttribute creates services (`$container->get()`), which the
//!   build-time port refuses.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveAutowireInlineAttributesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::compare::loose_equals;
use php_value::object::GenericObject;
use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ResolveAutowireInlineAttributesPass, new_autowire_attribute,
};
use symfony_dependency_injection::definition::Definition;
use symfony_dependency_injection::{ContainerBuilder, Reference};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const AUTOWIRE_INLINE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireInline";

fn c(short: &str) -> String {
    format!("{NS}{short}")
}

fn autowire_inline(arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: AUTOWIRE_INLINE.to_string(),
        arguments,
    }
}

fn inlined(type_: &str, attribute: AttributeMetadata) -> ParameterMetadata {
    let mut parameter = ParameterMetadata::new("inlined", Some(&c(type_)));
    parameter.attributes.push(attribute);
    parameter
}

fn ctor(parameters: Vec<ParameterMetadata>) -> MethodMetadata {
    MethodMetadata::public("__construct").with_parameters(parameters)
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    r.add_class(ClassMetadata::class(&c("Foo")));
    r.add_class(
        ClassMetadata::class(&c("AutowireInlineAttributesBar")).with_method(ctor(vec![
            ParameterMetadata::new("foo", Some(&c("Foo"))),
            ParameterMetadata::new("someString", Some("string")),
        ])),
    );
    let new_foo = Value::Object(Arc::new(GenericObject::new(c("Foo"), PhpArray::new())));
    r.add_class(
        ClassMetadata::class(&c("AutowireInlineAttributes1")).with_method(ctor(vec![inlined(
            "AutowireInlineAttributesBar",
            autowire_inline(php_array![
                0 => c("AutowireInlineAttributesBar"),
                1 => php_array!["$someString" => "testString", "$foo" => new_foo],
            ]),
        )])),
    );
    let nested = new_autowire_attribute(&autowire_inline(php_array![0 => c("Foo")])).unwrap();
    r.add_class(
        ClassMetadata::class(&c("NestedAutowireInlineAttribute")).with_method(ctor(vec![inlined(
            "AutowireInlineAttributesBar",
            autowire_inline(php_array![
                0 => c("AutowireInlineAttributesBar"),
                "arguments" => php_array![0 => nested, 1 => "testString"],
            ]),
        )])),
    );
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

#[test]
fn testChildDefinition() {
    let mut container = container();

    let definition = Definition::child("parent");
    definition.set_class(Some(&c("AutowireInlineAttributes1")));
    container
        .set_definition("autowire_inline1", definition)
        .unwrap()
        .set_autowired(true);

    ResolveAutowireInlineAttributesPass::new()
        .process(&mut container)
        .unwrap();

    let keys: Vec<Key> = container
        .get_definition("autowire_inline1")
        .unwrap()
        .arguments()
        .keys()
        .cloned()
        .collect();
    assert_eq!(vec![Key::from("$inlined")], keys);
}

#[test]
fn testNestedAttribute() {
    let mut container = container();

    container
        .register(
            "nested_autowire_inline",
            Some(&c("NestedAutowireInlineAttribute")),
        )
        .unwrap()
        .set_autowired(true);

    ResolveAutowireInlineAttributesPass::new()
        .process(&mut container)
        .unwrap();

    let arguments = |id: &str| Value::Array(container.get_definition(id).unwrap().arguments());
    let class = |id: &str| container.get_definition(id).unwrap().class();

    assert!(loose_equals(
        &Value::Array(php_array![
            0 => Reference::new(".autowire_inline.nested_autowire_inline.1")
        ]),
        &arguments("nested_autowire_inline"),
    ));
    assert_eq!(
        Some(c("AutowireInlineAttributesBar")),
        class(".autowire_inline.nested_autowire_inline.1")
    );

    assert!(loose_equals(
        &Value::Array(php_array![
            0 => Reference::new(".autowire_inline.nested_autowire_inline.2"),
            1 => "testString",
        ]),
        &arguments(".autowire_inline.nested_autowire_inline.1"),
    ));
    assert_eq!(
        Some(c("Foo")),
        class(".autowire_inline.nested_autowire_inline.2")
    );
}

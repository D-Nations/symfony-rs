//! Tests/Compiler/AddBehaviorDescribingTagsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AddBehaviorDescribingTagsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::compare::loose_equals;
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::compiler::{
    AddBehaviorDescribingTagsPass, CompilerPassInterface, DecoratorServicePass,
};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior};

fn list(items: &[&str]) -> Value {
    Value::Array(PhpArray::list(items.iter().map(|t| Value::from(*t))))
}

fn tags(items: &[&str]) -> Vec<String> {
    items.iter().map(|t| (*t).to_string()).collect()
}

#[test]
fn testProcessSetsParameter() {
    let mut container = ContainerBuilder::new();

    AddBehaviorDescribingTagsPass::new(tags(&["kernel.event_subscriber", "kernel.reset"]))
        .process(&mut container)
        .unwrap();

    assert!(
        list(&[
            "proxy",
            "container.do_not_inline",
            "container.service_locator",
            "container.service_subscriber",
            "container.service_subscriber.locator",
            "kernel.event_subscriber",
            "kernel.reset",
        ])
        .identical(
            &container
                .get_parameter("container.behavior_describing_tags")
                .unwrap()
        )
    );
}

#[test]
fn testMultiplePassesMerge() {
    let mut container = ContainerBuilder::new();

    AddBehaviorDescribingTagsPass::new(tags(&["kernel.event_subscriber", "kernel.reset"]))
        .process(&mut container)
        .unwrap();
    AddBehaviorDescribingTagsPass::new(tags(&["kernel.locale_aware"]))
        .process(&mut container)
        .unwrap();

    assert!(
        list(&[
            "proxy",
            "container.do_not_inline",
            "container.service_locator",
            "container.service_subscriber",
            "container.service_subscriber.locator",
            "kernel.event_subscriber",
            "kernel.reset",
            "kernel.locale_aware",
        ])
        .identical(
            &container
                .get_parameter("container.behavior_describing_tags")
                .unwrap()
        )
    );
}

#[test]
fn testProcessWithExistingParameter() {
    let mut container = ContainerBuilder::new();
    container
        .set_parameter(
            "container.behavior_describing_tags",
            list(&["existing.tag"]),
        )
        .unwrap();

    AddBehaviorDescribingTagsPass::new(tags(&["new.tag"]))
        .process(&mut container)
        .unwrap();

    assert!(
        list(&["existing.tag", "new.tag"]).identical(
            &container
                .get_parameter("container.behavior_describing_tags")
                .unwrap()
        )
    );
}

#[test]
fn testDefaultTagsCoverServiceSubscriberLocatorAndProxyTagsKeptByDecoratorServicePass() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().set_tags(php_array![
        "proxy" => "foo",
        "container.service_subscriber" => php_array![],
        "container.service_subscriber.locator" => php_array![],
        "bar" => php_array!["attr" => "baz"],
    ]);
    container
        .register("baz", None)
        .unwrap()
        .set_tags(php_array!["foobar" => php_array!["attr" => "bar"]])
        .set_decorated_service(Some("foo"), None, 0, InvalidBehavior::Exception)
        .unwrap();

    AddBehaviorDescribingTagsPass::new(Vec::new())
        .process(&mut container)
        .unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();

    let expected = php_array![
        "proxy" => "foo",
        "container.service_subscriber" => php_array![],
        "container.service_subscriber.locator" => php_array![],
    ];
    let actual = container.get_definition("baz.inner").unwrap().tags();
    assert!(
        loose_equals(&Value::Array(expected), &Value::Array(actual.clone())),
        "{actual:?}"
    );
    let expected = php_array![
        "bar" => php_array!["attr" => "baz"],
        "foobar" => php_array!["attr" => "bar"],
        "container.decorator" => php_array![php_array!["id" => "foo", "inner" => "baz.inner"]],
    ];
    let actual = container.get_definition("baz").unwrap().tags();
    assert!(
        loose_equals(&Value::Array(expected), &Value::Array(actual.clone())),
        "{actual:?}"
    );
}

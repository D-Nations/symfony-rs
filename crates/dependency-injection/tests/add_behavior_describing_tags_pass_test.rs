//! Tests/Compiler/AddBehaviorDescribingTagsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending: testDefaultTagsCoverServiceSubscriberLocatorAndProxyTagsKeptByDecoratorServicePass
//! (it needs `DecoratorServicePass`, not ported yet).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AddBehaviorDescribingTagsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::compiler::{
    AddBehaviorDescribingTagsPass, CompilerPassInterface,
};

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

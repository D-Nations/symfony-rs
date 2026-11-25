//! Tests/Compiler/DecoratorServicePassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order.
//! `assertEquals()` on an alias compares its id, on arrays it is PHP's
//! `==` (`loose_equals`).
//!
//! Not portable: testProcessWithInvalidDecoratedAndWrongBehavior passes
//! the invalid behaviour `12`, which the port's closed `InvalidBehavior`
//! cannot hold.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/DecoratorServicePassTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::disallowed_names
)]

use php_value::compare::loose_equals;
use php_value::{PhpArray, Value};
use symfony_dependency_injection::compiler::{CompilerPassInterface, DecoratorServicePass};
use symfony_dependency_injection::{
    Alias, ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference, Result,
};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    DecoratorServicePass::new().process(container)
}

fn decorate(definition: &Definition, id: &str, renamed: Option<&str>, priority: i64) {
    definition
        .set_decorated_service(Some(id), renamed, priority, InvalidBehavior::Exception)
        .unwrap();
}

fn decorate_with(definition: &Definition, id: &str, behavior: InvalidBehavior) {
    definition
        .set_decorated_service(Some(id), None, 0, behavior)
        .unwrap();
}

fn alias_id(container: &ContainerBuilder, id: &str) -> String {
    container.get_alias(id).unwrap().id()
}

/// A PHP array from `(key, value)` pairs.
fn array(entries: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in entries {
        array.insert(k, v);
    }
    array
}

fn arr(entries: Vec<(&str, Value)>) -> Value {
    Value::Array(array(entries))
}

fn list(values: impl IntoIterator<Item = Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

fn s(text: &str) -> Value {
    Value::from(text)
}

fn tags(container: &ContainerBuilder, id: &str) -> Value {
    Value::Array(container.get_definition(id).unwrap().tags())
}

/// `['id' => 'foo', 'inner' => $inner]` in a list.
fn decorator_tag(inner: &str) -> Value {
    list([arr(vec![("id", s("foo")), ("inner", s(inner))])])
}

#[test]
fn testProcessWithoutAlias() {
    let mut container = ContainerBuilder::new();
    let foo_definition = container.register("foo", None).unwrap();
    let foo_extended_definition = container.register("foo.extended", None).unwrap();
    foo_extended_definition.set_public(true);
    decorate(&foo_extended_definition, "foo", None, 0);
    let bar_definition = container.register("bar", None).unwrap();
    bar_definition.set_public(true);
    let bar_extended_definition = container.register("bar.extended", None).unwrap();
    bar_extended_definition.set_public(true);
    decorate(&bar_extended_definition, "bar", Some("bar.yoo"), 0);

    process(&mut container).unwrap();

    assert_eq!("foo.extended", alias_id(&container, "foo"));
    assert!(!container.get_alias("foo").unwrap().is_public());

    assert_eq!("bar.extended", alias_id(&container, "bar"));
    assert!(container.get_alias("bar").unwrap().is_public());

    assert!(foo_definition.ptr_eq(&container.get_definition("foo.extended.inner").unwrap()));
    assert!(
        !container
            .get_definition("foo.extended.inner")
            .unwrap()
            .is_public()
    );

    assert!(bar_definition.ptr_eq(&container.get_definition("bar.yoo").unwrap()));
    assert!(!container.get_definition("bar.yoo").unwrap().is_public());

    assert_eq!(None, foo_extended_definition.decorated_service());
    assert_eq!(None, bar_extended_definition.decorated_service());
}

#[test]
fn testProcessWithAlias() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().set_public(true);
    container
        .set_alias("foo.alias", Alias::new("foo", false))
        .unwrap();
    let foo_extended_definition = container.register("foo.extended", None).unwrap();
    foo_extended_definition.set_public(true);
    decorate(&foo_extended_definition, "foo.alias", None, 0);

    process(&mut container).unwrap();

    assert_eq!("foo.extended", alias_id(&container, "foo.alias"));
    assert!(!container.get_alias("foo.alias").unwrap().is_public());

    assert_eq!("foo", alias_id(&container, "foo.extended.inner"));
    assert!(
        !container
            .get_alias("foo.extended.inner")
            .unwrap()
            .is_public()
    );

    assert_eq!(None, foo_extended_definition.decorated_service());
}

#[test]
fn testProcessWithPriority() {
    let mut container = ContainerBuilder::new();
    let foo_definition = container.register("foo", None).unwrap();
    let bar_definition = container.register("bar", None).unwrap();
    bar_definition.set_public(true);
    decorate(&bar_definition, "foo", None, 0);
    let baz_definition = container.register("baz", None).unwrap();
    baz_definition.set_public(true);
    decorate(&baz_definition, "foo", None, 5);
    let qux_definition = container.register("qux", None).unwrap();
    qux_definition.set_public(true);
    decorate(&qux_definition, "foo", None, 3);

    process(&mut container).unwrap();

    assert_eq!("bar", alias_id(&container, "foo"));
    assert!(!container.get_alias("foo").unwrap().is_public());

    assert!(foo_definition.ptr_eq(&container.get_definition("baz.inner").unwrap()));
    assert!(!container.get_definition("baz.inner").unwrap().is_public());

    assert_eq!("qux", alias_id(&container, "bar.inner"));
    assert!(!container.get_alias("bar.inner").unwrap().is_public());

    assert_eq!("baz", alias_id(&container, "qux.inner"));
    assert!(!container.get_alias("qux.inner").unwrap().is_public());

    assert_eq!(None, bar_definition.decorated_service());
    assert_eq!(None, baz_definition.decorated_service());
    assert_eq!(None, qux_definition.decorated_service());
}

#[test]
fn testProcessWithInvalidDecorated() {
    let mut container = ContainerBuilder::new();
    let decorator_definition = container.register("decorator", None).unwrap();
    decorate_with(
        &decorator_definition,
        "unknown_decorated",
        InvalidBehavior::Ignore,
    );

    process(&mut container).unwrap();
    assert!(!container.has("decorator"));

    let mut container = ContainerBuilder::new();
    let decorator_definition = container.register("decorator", None).unwrap();
    decorate_with(
        &decorator_definition,
        "unknown_decorated",
        InvalidBehavior::Null,
    );

    process(&mut container).unwrap();
    assert!(container.has("decorator"));
    assert_eq!(
        Some(InvalidBehavior::Null),
        decorator_definition.decoration_on_invalid()
    );

    let mut container = ContainerBuilder::new();
    let decorator_definition = container.register("decorator", None).unwrap();
    decorate(&decorator_definition, "unknown_service", None, 0);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
}

#[test]
fn testProcessNoInnerAliasWithInvalidDecorated() {
    let mut container = ContainerBuilder::new();
    let decorator = container.register("decorator", None).unwrap();
    decorate_with(&decorator, "unknown_decorated", InvalidBehavior::Null);

    process(&mut container).unwrap();
    assert!(!container.has_alias("decorator.inner"));
}

#[test]
fn testProcessMovesTagsFromDecoratedDefinitionToDecoratingDefinition() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_tags(array(vec![("bar", arr(vec![("attr", s("baz"))]))]));
    let baz = container.register("baz", None).unwrap();
    baz.set_tags(array(vec![("foobar", arr(vec![("attr", s("bar"))]))]));
    decorate(&baz, "foo", None, 0);

    process(&mut container).unwrap();

    assert!(tags(&container, "baz.inner").identical(&Value::Array(PhpArray::new())));
    assert!(loose_equals(
        &arr(vec![
            ("bar", arr(vec![("attr", s("baz"))])),
            ("foobar", arr(vec![("attr", s("bar"))])),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

#[test]
fn testProcessMovesTagsFromDecoratedDefinitionToDecoratingDefinitionMultipleTimes() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .set_tags(array(vec![("bar", arr(vec![("attr", s("baz"))]))]));
    let deco1 = container.register("deco1", None).unwrap();
    decorate(&deco1, "foo", None, 50);
    let deco2 = container.register("deco2", None).unwrap();
    decorate(&deco2, "foo", None, 2);

    process(&mut container).unwrap();

    assert!(tags(&container, "deco1").identical(&Value::Array(PhpArray::new())));
    assert!(loose_equals(
        &arr(vec![
            ("bar", arr(vec![("attr", s("baz"))])),
            ("container.decorator", decorator_tag("deco1.inner")),
        ]),
        &tags(&container, "deco2")
    ));
}

/// A decorator id, the id it decorates and its priority.
type Decorator = (&'static str, &'static str, i64);

#[test]
fn testProcessMovesTagsToTheOutermostDecoratorOfTheService() {
    let cases: [(&[Decorator], &str); 3] = [
        // decorator of a decorator processed first
        (&[("deco1", "foo", 0), ("deco2", "deco1", 5)], "deco2"),
        // decorator of a decorator processed in between
        (
            &[
                ("deco1", "foo", 5),
                ("deco2", "deco1", 3),
                ("deco3", "foo", 0),
            ],
            "deco3",
        ),
        // decorator of an inner decorator
        (
            &[
                ("deco1", "foo", 0),
                ("deco2", "foo", -1),
                ("deco3", "deco1", 5),
            ],
            "deco2",
        ),
    ];
    for (decorators, expected_outermost_decorator) in cases {
        let mut container = ContainerBuilder::new();
        container
            .register("foo", None)
            .unwrap()
            .set_tags(array(vec![("bar", arr(vec![("attr", s("baz"))]))]));
        for (id, decorated_id, priority) in decorators {
            let definition = container.register(id, None).unwrap();
            decorate(&definition, decorated_id, None, *priority);
        }

        process(&mut container).unwrap();

        let tagged: Vec<String> = container
            .find_tagged_service_ids("bar", false)
            .unwrap()
            .keys()
            .map(str::to_string)
            .collect();
        assert_eq!(vec![expected_outermost_decorator.to_string()], tagged);
    }
}

#[test]
fn testProcessKeepsTheOrderOfTheDecoratorsOfADecoratorOfAMissingService() {
    let mut container = ContainerBuilder::new();
    let deco1 = container.register("deco1", None).unwrap();
    decorate_with(&deco1, "foo", InvalidBehavior::Ignore);
    let deco2 = container.register("deco2", None).unwrap();
    decorate(&deco2, "deco1", None, 5);

    process(&mut container).unwrap();

    assert_eq!("deco2", alias_id(&container, "deco1"));
}

/// `foo` tagged with `$tags`, decorated by `baz` tagged `foobar`.
fn decorated_with_tags(tags: PhpArray, baz_tags: Option<PhpArray>) -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().set_tags(tags);
    let baz = container.register("baz", None).unwrap();
    if let Some(baz_tags) = baz_tags {
        baz.set_tags(baz_tags);
    }
    decorate(&baz, "foo", None, 0);
    container
}

fn foobar_tags() -> PhpArray {
    array(vec![("foobar", arr(vec![("attr", s("bar"))]))])
}

fn locator_tag() -> Value {
    let mut tag = PhpArray::new();
    tag.insert(0, Value::Array(PhpArray::new()));
    Value::Array(tag)
}

#[test]
fn testProcessLeavesServiceLocatorTagOnOriginalDefinition() {
    let mut container = decorated_with_tags(
        array(vec![
            ("container.service_locator", locator_tag()),
            ("bar", arr(vec![("attr", s("baz"))])),
        ]),
        Some(foobar_tags()),
    );

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![("container.service_locator", locator_tag())]),
        &tags(&container, "baz.inner")
    ));
    assert!(loose_equals(
        &arr(vec![
            ("bar", arr(vec![("attr", s("baz"))])),
            ("foobar", arr(vec![("attr", s("bar"))])),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

#[test]
fn testProcessLeavesServiceSubscriberTagOnOriginalDefinition() {
    let empty = || Value::Array(PhpArray::new());
    let mut container = decorated_with_tags(
        array(vec![
            ("container.service_subscriber", empty()),
            ("container.service_subscriber.locator", empty()),
            ("bar", arr(vec![("attr", s("baz"))])),
        ]),
        Some(foobar_tags()),
    );

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![
            ("container.service_subscriber", empty()),
            ("container.service_subscriber.locator", empty()),
        ]),
        &tags(&container, "baz.inner")
    ));
    assert!(loose_equals(
        &arr(vec![
            ("bar", arr(vec![("attr", s("baz"))])),
            ("foobar", arr(vec![("attr", s("bar"))])),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

#[test]
fn testProcessLeavesProxyTagOnOriginalDefinition() {
    let mut container = decorated_with_tags(
        array(vec![
            ("proxy", s("foo")),
            ("bar", arr(vec![("attr", s("baz"))])),
        ]),
        Some(foobar_tags()),
    );

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![("proxy", s("foo"))]),
        &tags(&container, "baz.inner")
    ));
    assert!(loose_equals(
        &arr(vec![
            ("bar", arr(vec![("attr", s("baz"))])),
            ("foobar", arr(vec![("attr", s("bar"))])),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

fn listener_tag() -> Value {
    list([arr(vec![("event", s("foo"))])])
}

#[test]
fn testProcessMovesRoleDescribingTagsWithoutBehaviorDescribingTagsParameter() {
    let mut container = decorated_with_tags(
        array(vec![
            ("container.service_locator", locator_tag()),
            ("kernel.event_listener", listener_tag()),
        ]),
        None,
    );

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![("container.service_locator", locator_tag())]),
        &tags(&container, "baz.inner")
    ));
    assert!(loose_equals(
        &arr(vec![
            ("kernel.event_listener", listener_tag()),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

#[test]
fn testProcessLeavesBehaviorDescribingTagsFromParameterOnOriginalDefinition() {
    let logger_aware = || list([Value::Array(PhpArray::new())]);
    let mut container = decorated_with_tags(
        array(vec![
            ("container.service_locator", locator_tag()),
            ("logger_aware", logger_aware()),
            ("kernel.event_listener", listener_tag()),
        ]),
        None,
    );
    container
        .set_parameter(
            "container.behavior_describing_tags",
            list([s("container.service_locator"), s("logger_aware")]),
        )
        .unwrap();

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![
            ("container.service_locator", locator_tag()),
            ("logger_aware", logger_aware()),
        ]),
        &tags(&container, "baz.inner")
    ));
    assert!(loose_equals(
        &arr(vec![
            ("kernel.event_listener", listener_tag()),
            ("container.decorator", decorator_tag("baz.inner")),
        ]),
        &tags(&container, "baz")
    ));
}

#[test]
fn testCannotDecorateSyntheticService() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().set_synthetic(true);
    let baz = container.register("baz", None).unwrap();
    decorate(&baz, "foo", None, 0);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "A synthetic service cannot be decorated: service \"baz\" cannot decorate \"foo\".",
        e.message()
    );
}

#[test]
fn testGenericInnerReference() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();

    let bar = container.register("bar", None).unwrap();
    decorate(&bar, "foo", None, 0);
    bar.set_property("prop", Reference::new(".inner"));

    process(&mut container).unwrap();

    assert!(loose_equals(
        &arr(vec![("prop", Reference::new("bar.inner").into())]),
        &Value::Array(container.get_definition("bar").unwrap().properties())
    ));
}

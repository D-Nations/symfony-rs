//! Tests/Compiler/ResolveInstanceofConditionalsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `self::class` is the test class (a `TestCase`),
//! `parent::class` is `PHPUnit\Framework\TestCase`; these, `Reference`,
//! `TypedReference`, `\Stringable`, `DecoratorWithBehavior` and the
//! interfaces it implements are class metadata. `assertEquals()` on
//! `BoundArgument`s compares their values; on tag arrays it ignores the
//! order of the keys, as PHPUnit's does.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveInstanceofConditionalsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::BoundArgument;
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ResolveChildDefinitionsPass, ResolveInstanceofConditionalsPass,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference,
};

const SELF: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\ResolveInstanceofConditionalsPassTest";
const PARENT: &str = "PHPUnit\\Framework\\TestCase";
const REFERENCE: &str = "Symfony\\Component\\DependencyInjection\\Reference";
const TYPED_REFERENCE: &str = "Symfony\\Component\\DependencyInjection\\TypedReference";
const DECORATOR_WITH_BEHAVIOR: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\DecoratorWithBehavior";
const RESET_INTERFACE: &str = "Symfony\\Contracts\\Service\\ResetInterface";
const RESOURCE_CHECKER_INTERFACE: &str = "Symfony\\Component\\Config\\ResourceCheckerInterface";
const SERVICE_SUBSCRIBER_INTERFACE: &str =
    "Symfony\\Contracts\\Service\\ServiceSubscriberInterface";

fn container() -> ContainerBuilder {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class("stdClass"));
    registry.add_class(ClassMetadata::interface("PHPUnit\\Framework\\Test"));
    registry.add_class(ClassMetadata::interface(
        "PHPUnit\\Framework\\SelfDescribing",
    ));
    registry.add_class(
        ClassMetadata::class(PARENT)
            .with_abstract(true)
            .with_interfaces(&[
                "PHPUnit\\Framework\\Test",
                "PHPUnit\\Framework\\SelfDescribing",
            ]),
    );
    registry.add_class(ClassMetadata::class(SELF).with_parent(PARENT));
    registry.add_class(ClassMetadata::interface("Stringable"));
    registry.add_class(ClassMetadata::class(REFERENCE).with_interfaces(&["Stringable"]));
    registry.add_class(ClassMetadata::class(TYPED_REFERENCE).with_parent(REFERENCE));
    registry.add_class(ClassMetadata::interface(RESET_INTERFACE));
    registry.add_class(ClassMetadata::interface(RESOURCE_CHECKER_INTERFACE));
    registry.add_class(ClassMetadata::interface(SERVICE_SUBSCRIBER_INTERFACE));
    registry.add_class(
        ClassMetadata::class(DECORATOR_WITH_BEHAVIOR).with_interfaces(&[
            RESET_INTERFACE,
            RESOURCE_CHECKER_INTERFACE,
            SERVICE_SUBSCRIBER_INTERFACE,
        ]),
    );
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

fn conditionals(entries: Vec<(&str, Definition)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (interface, definition) in entries {
        array.insert(interface, definition.to_value());
    }
    array
}

fn instanceof() -> ResolveInstanceofConditionalsPass {
    ResolveInstanceofConditionalsPass::new()
}

fn child_definitions() -> ResolveChildDefinitionsPass {
    ResolveChildDefinitionsPass::new()
}

fn call(method: &str, arguments: PhpArray) -> Value {
    Value::from(php_array![Value::from(method), Value::from(arguments)])
}

#[test]
fn testProcess() {
    let mut container = container();
    let def = container.register("foo", Some(SELF)).unwrap();
    def.add_tag("tag", PhpArray::new())
        .set_autowired(true)
        .set_changes(Vec::new());
    let instanceof_def = Definition::child("");
    instanceof_def
        .set_property("foo", "bar")
        .add_tag("baz", php_array!["attr" => 123]);
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, instanceof_def)]));

    instanceof().process(&mut container).unwrap();

    let parent = format!(".instanceof.{PARENT}.0.foo");
    let def = container.get_definition("foo").unwrap();
    assert_eq!(PhpArray::new(), def.instanceof_conditionals());
    assert!(def.is_child(), "assertInstanceOf(ChildDefinition::class)");
    assert!(def.is_autowired());
    assert_eq!(Some(parent.clone()), def.parent());
    assert_eq!(
        php_array!["tag" => php_array![PhpArray::new()], "baz" => php_array![php_array!["attr" => 123]]],
        def.tags()
    );

    let parent = container.get_definition(&parent).unwrap();
    assert_eq!(php_array!["foo" => "bar"], parent.properties());
    assert_eq!(PhpArray::new(), parent.tags());
}

#[test]
fn testProcessInheritance() {
    let mut container = container();

    let def = container.register("parent", Some(PARENT)).unwrap();
    def.add_method_call("foo", php_array!["foo"], false)
        .unwrap();
    let instanceof_def = Definition::child("");
    instanceof_def
        .add_method_call("foo", php_array!["bar"], false)
        .unwrap();
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, instanceof_def)]));

    let def = Definition::child("parent");
    def.set_class(Some(SELF));
    container.set_definition("child", def).unwrap();

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    let expected = php_array![
        call("foo", php_array!["bar"]),
        call("foo", php_array!["foo"]),
    ];

    assert_eq!(
        expected,
        container.get_definition("parent").unwrap().method_calls()
    );
    assert_eq!(
        expected,
        container.get_definition("child").unwrap().method_calls()
    );
}

#[test]
fn testProcessDoesReplaceShared() {
    let mut container = container();

    let def = container.register("foo", Some("stdClass")).unwrap();
    let instanceof_def = Definition::child("");
    instanceof_def.set_shared(false);
    def.set_instanceof_conditionals(conditionals(vec![("stdClass", instanceof_def)]));

    instanceof().process(&mut container).unwrap();

    let def = container.get_definition("foo").unwrap();
    assert!(!def.is_shared());
}

#[test]
fn testProcessHandlesMultipleInheritance() {
    let mut container = container();

    let def = container.register("foo", Some(SELF)).unwrap();
    def.set_shared(true);

    let lazy = Definition::child("");
    lazy.set_lazy(true).set_shared(false);
    let autowired = Definition::child("");
    autowired.set_autowired(true);
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, lazy), (SELF, autowired)]));

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    let def = container.get_definition("foo").unwrap();
    assert!(def.is_autowired());
    assert!(def.is_lazy());
    assert!(def.is_shared());
}

#[test]
fn testProcessUsesAutoconfiguredInstanceof() {
    let mut container = container();
    let def = container.register("normal_service", Some(SELF)).unwrap();
    let local = Definition::child("");
    local
        .add_tag("local_instanceof_tag", PhpArray::new())
        .set_factory("locally_set_factory");
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, local)]));
    def.set_autoconfigured(true);
    container
        .register_for_autoconfiguration(PARENT)
        .add_tag("autoconfigured_tag", PhpArray::new())
        .set_autowired(true)
        .set_factory("autoconfigured_factory");

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    let def = container.get_definition("normal_service").unwrap();
    // autowired thanks to the autoconfigured instanceof
    assert!(def.is_autowired());
    // factory from the specific instanceof overrides global one
    assert_eq!(Value::from("locally_set_factory"), def.factory());
    // tags are merged, the locally set one is first
    assert_eq!(
        php_array![
            "local_instanceof_tag" => php_array![PhpArray::new()],
            "autoconfigured_tag" => php_array![PhpArray::new()]
        ],
        def.tags()
    );
}

#[test]
fn testAutoconfigurationOfTheClassWinsOverTheInheritedOne() {
    let mut container = container();
    container
        .register("normal_service", Some(SELF))
        .unwrap()
        .set_autoconfigured(true);
    container
        .register_for_autoconfiguration(SELF)
        .add_tag("some_tag", php_array!["priority" => 100]);
    container
        .register_for_autoconfiguration(PARENT)
        .add_tag("some_tag", PhpArray::new());

    instanceof().process(&mut container).unwrap();

    assert_eq!(
        php_array![php_array!["priority" => 100], PhpArray::new()],
        container
            .get_definition("normal_service")
            .unwrap()
            .tag("some_tag")
    );
}

#[test]
fn testAutoconfiguredTagsAreOrderedFromTheMostSpecificType() {
    let mut container = container();
    container
        .register("normal_service", Some(TYPED_REFERENCE))
        .unwrap()
        .set_autoconfigured(true);
    container
        .register_for_autoconfiguration(REFERENCE)
        .add_tag("some_tag", php_array!["from" => "parent"]);
    container
        .register_for_autoconfiguration("Stringable")
        .add_tag("some_tag", php_array!["from" => "interface"]);
    container
        .register_for_autoconfiguration(TYPED_REFERENCE)
        .add_tag("some_tag", php_array!["from" => "class"]);

    instanceof().process(&mut container).unwrap();

    let expected = php_array![
        php_array!["from" => "class"],
        php_array!["from" => "parent"],
        php_array!["from" => "interface"]
    ];
    assert_eq!(
        expected,
        container
            .get_definition("normal_service")
            .unwrap()
            .tag("some_tag")
    );
}

#[test]
fn testAutoconfigureInstanceofDoesNotDuplicateTags() {
    let mut container = container();
    let def = container.register("normal_service", Some(SELF)).unwrap();
    def.add_tag("duplicated_tag", PhpArray::new())
        .add_tag("duplicated_tag", php_array!["and_attributes" => 1]);
    let local = Definition::child("");
    local.add_tag("duplicated_tag", PhpArray::new());
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, local)]));
    def.set_autoconfigured(true);
    container
        .register_for_autoconfiguration(PARENT)
        .add_tag("duplicated_tag", php_array!["and_attributes" => 1]);

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    let def = container.get_definition("normal_service").unwrap();
    assert_eq!(
        php_array!["duplicated_tag" => php_array![PhpArray::new(), php_array!["and_attributes" => 1]]],
        def.tags()
    );
}

#[test]
fn testProcessDoesNotUseAutoconfiguredInstanceofIfNotEnabled() {
    let mut container = container();
    let def = container.register("normal_service", Some(SELF)).unwrap();
    let local = Definition::child("");
    local.add_tag("foo_tag", PhpArray::new());
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, local)]));
    container
        .register_for_autoconfiguration(PARENT)
        .set_autowired(true);

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    let def = container.get_definition("normal_service").unwrap();
    assert!(!def.is_autowired());
}

#[test]
fn testBadInterfaceThrowsException() {
    let mut container = container();
    let def = container.register("normal_service", Some(SELF)).unwrap();
    let local = Definition::child("");
    local.add_tag("foo_tag", PhpArray::new());
    def.set_instanceof_conditionals(conditionals(vec![("App\\FakeInterface", local)]));

    let e = instanceof().process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "\"App\\FakeInterface\" is set as an \"instanceof\" conditional, but it does not exist.",
        e.message()
    );
}

#[test]
fn testBadInterfaceForAutomaticInstanceofIsOk() {
    let mut container = container();
    container
        .register("normal_service", Some(SELF))
        .unwrap()
        .set_autoconfigured(true);
    container
        .register_for_autoconfiguration("App\\FakeInterface")
        .set_autowired(true);

    instanceof().process(&mut container).unwrap();
    assert!(container.has_definition("normal_service"));
}

/// Test that autoconfigured calls are handled gracefully.
#[test]
fn testProcessForAutoconfiguredCalls() {
    let mut container = container();

    let expected = php_array![
        call("setFoo", php_array!["plain_value", "%some_parameter%"]),
        call("callBar", PhpArray::new()),
        call("isBaz", PhpArray::new()),
    ];

    container
        .register_for_autoconfiguration(PARENT)
        .add_method_call(
            "setFoo",
            php_array!["plain_value", "%some_parameter%"],
            false,
        )
        .unwrap();
    container
        .register_for_autoconfiguration(SELF)
        .add_method_call("callBar", PhpArray::new(), false)
        .unwrap();

    let def = container.register("foo", Some(SELF)).unwrap();
    def.set_autoconfigured(true)
        .add_method_call("isBaz", PhpArray::new(), false)
        .unwrap();
    assert_eq!(
        php_array![call("isBaz", PhpArray::new())],
        def.method_calls(),
        "Definition shouldn't have only one method call."
    );

    instanceof().process(&mut container).unwrap();

    assert_eq!(
        expected,
        container.find_definition("foo").unwrap().method_calls()
    );
}

#[test]
fn testProcessThrowsExceptionForArguments() {
    let mut container = container();
    container
        .register_for_autoconfiguration(PARENT)
        .add_argument("bar");

    let e = instanceof().process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Autoconfigured instanceof for type \"PHPUnit\\Framework\\TestCase\" defines arguments but these are not supported and should be removed.",
        e.message()
    );
}

#[test]
fn testMergeReset() {
    let mut container = container();

    let local = Definition::child("");
    local.add_tag("bar", PhpArray::new());
    container
        .register("bar", Some(SELF))
        .unwrap()
        .add_argument("a")
        .add_method_call("setB", PhpArray::new(), false)
        .unwrap()
        .set_decorated_service(Some("foo"), None, 0, InvalidBehavior::Exception)
        .unwrap()
        .add_tag("t", PhpArray::new())
        .set_instanceof_conditionals(conditionals(vec![(PARENT, local)]));

    instanceof().process(&mut container).unwrap();

    let abstract_ = container
        .get_definition(".abstract.instanceof.bar")
        .unwrap();

    assert_eq!(PhpArray::new(), abstract_.arguments());
    assert_eq!(PhpArray::new(), abstract_.method_calls());
    assert_eq!(None, abstract_.decorated_service());
    assert_eq!(PhpArray::new(), abstract_.tags());
    assert!(abstract_.is_abstract());
}

fn bound_values(bindings: &PhpArray) -> Vec<(Key, PhpArray)> {
    bindings
        .iter()
        .map(|(k, v)| (k.clone(), BoundArgument::from_value(v).unwrap().values()))
        .collect()
}

#[test]
fn testProcessForAutoconfiguredBindings() {
    let mut container = container();

    let mut bindings = PhpArray::new();
    bindings.insert(
        "$foo",
        BoundArgument::new(Value::Int(234), false, 0, None).to_value(),
    );
    bindings.insert(
        PARENT,
        BoundArgument::new(Reference::new("foo").into(), false, 0, None).to_value(),
    );
    container
        .register_for_autoconfiguration(SELF)
        .set_bindings(&bindings);

    container
        .register("foo", Some(SELF))
        .unwrap()
        .set_autoconfigured(true)
        .set_bindings(&php_array![
            "$foo" => BoundArgument::new(Value::Int(123), false, 0, None).to_value()
        ]);

    instanceof().process(&mut container).unwrap();

    let foo_reference = Value::from(Reference::new("foo"));
    let actual = bound_values(&container.find_definition("foo").unwrap().bindings());
    assert_eq!(2, actual.len());
    assert_eq!(Key::from("$foo"), actual[0].0);
    assert_eq!(
        BoundArgument::new(Value::Int(123), false, 0, None).values(),
        actual[0].1
    );
    assert_eq!(Key::from(PARENT), actual[1].0);
    // assertEquals(): an equal Reference.
    assert_eq!(
        Reference::from_value(&foo_reference),
        Reference::from_value(actual[1].1.get(&Key::Int(0)).unwrap())
    );
}

#[test]
fn testBindingsOnInstanceofConditionals() {
    let mut container = container();
    let def = container.register("foo", Some(SELF)).unwrap();
    def.set_bindings(&php_array!["$toto" => 123]);
    def.set_instanceof_conditionals(conditionals(vec![(PARENT, Definition::child(""))]));

    instanceof().process(&mut container).unwrap();

    let bindings = container.get_definition("foo").unwrap().bindings();
    let keys: Vec<Key> = bindings.keys().cloned().collect();
    assert_eq!(vec![Key::from("$toto")], keys);
    let binding = BoundArgument::from_value(bindings.get(&Key::from("$toto")).unwrap())
        .expect("assertInstanceOf(BoundArgument::class)");
    assert_eq!(Some(&Value::Int(123)), binding.values().get(&Key::Int(0)));
}

#[test]
fn testDecoratorsAreNotAutomaticallyTagged() {
    let mut container = container();

    let decorator = container.register("decorator", Some(SELF)).unwrap();
    decorator
        .set_decorated_service(Some("decorated"), None, 0, InvalidBehavior::Exception)
        .unwrap();
    let tag = Definition::child("");
    tag.add_tag("tag", PhpArray::new());
    let other_tag = Definition::child("");
    other_tag.add_tag("other-tag", PhpArray::new());
    decorator.set_instanceof_conditionals(conditionals(vec![(PARENT, tag), (SELF, other_tag)]));
    decorator.set_autoconfigured(true);
    decorator.add_tag("manual", PhpArray::new());

    container
        .register_for_autoconfiguration(PARENT)
        .add_tag("tag", PhpArray::new());
    container
        .register_for_autoconfiguration(SELF)
        .add_tag("last-tag", PhpArray::new());

    instanceof().process(&mut container).unwrap();
    child_definitions().process(&mut container).unwrap();

    assert_eq!(
        php_array![
            "manual" => php_array![PhpArray::new()],
            "other-tag" => php_array![PhpArray::new()],
            "last-tag" => php_array![PhpArray::new()]
        ],
        container.get_definition("decorator").unwrap().tags()
    );
}

#[test]
fn testDecoratorsKeepBehaviorDescribingTags() {
    let mut container = container();

    container
        .set_parameter(
            "container.behavior_describing_tags",
            php_array!["container.service_subscriber", "kernel.reset"],
        )
        .unwrap();

    container
        .register("decorator", Some(DECORATOR_WITH_BEHAVIOR))
        .unwrap()
        .set_autoconfigured(true)
        .set_decorated_service(Some("decorated"), None, 0, InvalidBehavior::Exception)
        .unwrap();

    container
        .register_for_autoconfiguration(RESOURCE_CHECKER_INTERFACE)
        .add_tag("config_cache.resource_checker", PhpArray::new());
    container
        .register_for_autoconfiguration(SERVICE_SUBSCRIBER_INTERFACE)
        .add_tag("container.service_subscriber", PhpArray::new());
    container
        .register_for_autoconfiguration(RESET_INTERFACE)
        .add_tag("kernel.reset", php_array!["method" => "reset"]);

    instanceof().process(&mut container).unwrap();

    let tags = container.get_definition("decorator").unwrap().tags();
    assert_eq!(2, tags.len());
    assert_eq!(
        Some(&Value::from(php_array![0 => PhpArray::new()])),
        tags.get(&Key::from("container.service_subscriber"))
    );
    assert_eq!(
        Some(&Value::from(php_array![php_array!["method" => "reset"]])),
        tags.get(&Key::from("kernel.reset"))
    );
    assert!(!container.has_parameter("container.behavior_describing_tags"));
}

#[test]
fn testSyntheticService() {
    let mut container = container();
    let excluded = Definition::child("");
    excluded.add_tag("container.excluded", PhpArray::new());
    container
        .register("kernel", Some("stdClass"))
        .unwrap()
        .set_instanceof_conditionals(conditionals(vec![("stdClass", excluded)]))
        .set_synthetic(true);

    instanceof().process(&mut container).unwrap();

    assert_eq!(
        PhpArray::new(),
        container.get_definition("kernel").unwrap().tags()
    );
}

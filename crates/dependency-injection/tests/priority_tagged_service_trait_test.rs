//! Tests/Compiler/PriorityTaggedServiceTraitTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; the data provider's rows run in a loop. The fixture classes
//! (the test file's and `Tests/Fixtures/*TagClass.php`) are class
//! metadata; a static default method's result is its `returns`, and the
//! `#[AsTaggedItem]` attributes are class attributes. `assertEquals()` on
//! references compares their ids, types and names.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/PriorityTaggedServiceTraitTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::disallowed_names
)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, MethodMetadata, Visibility,
};
use symfony_dependency_injection::compiler::{
    AS_TAGGED_ITEM, CompilerPassInterface, ResolveInstanceofConditionalsPass, TagName,
    find_and_sort_tagged_services,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference,
};

const TESTS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";

fn t(short: &str) -> String {
    format!("{TESTS}{short}")
}

fn f(short: &str) -> String {
    format!("{FIXTURES}{short}")
}

fn static_method(name: &str, returns: Value) -> MethodMetadata {
    MethodMetadata::public(name)
        .with_static(true)
        .with_returns(returns)
}

fn as_tagged_item(index: Option<&str>, priority: Option<i64>) -> AttributeMetadata {
    let mut arguments = PhpArray::new();
    if let Some(index) = index {
        arguments.insert("index", Value::from(index));
    }
    if let Some(priority) = priority {
        arguments.insert("priority", Value::Int(priority));
    }
    AttributeMetadata {
        class: AS_TAGGED_ITEM.to_string(),
        arguments,
    }
}

fn with_attributes(mut class: ClassMetadata, attributes: Vec<AttributeMetadata>) -> ClassMetadata {
    class.attributes = attributes;
    class
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    r.add_class(ClassMetadata::class("stdClass"));
    r.add_class(
        ClassMetadata::class(&f("BarTagClass"))
            .with_method(static_method(
                "getDefaultFooName",
                Value::from("bar_tag_class"),
            ))
            .with_method(static_method(
                "getFooBar",
                Value::from("bar_tab_class_with_defaultmethod"),
            ))
            .with_method(static_method("getPriority", Value::Int(0)).with_return_type("int")),
    );
    r.add_class(
        ClassMetadata::class(&f("FooTagClass"))
            .with_method(static_method(
                "getDefaultFooName",
                Value::from("foo_tag_class"),
            ))
            .with_method(static_method("getPriority", Value::Int(20)).with_return_type("int")),
    );
    r.add_class(
        ClassMetadata::class(&f("IntTagClass"))
            .with_method(static_method("getFooBar", Value::Int(10)))
            .with_method(static_method("getPriority", Value::Int(30)).with_return_type("int")),
    );
    r.add_class(
        ClassMetadata::class(&f("FooTaggedForInvalidDefaultMethodClass"))
            .with_method(
                MethodMetadata::public("getMethodShouldBeStatic")
                    .with_returns(Value::from("anonymous_foo_class_with_default_method")),
            )
            .with_method(
                static_method(
                    "getMethodShouldBePublicInsteadProtected",
                    Value::from("anonymous_foo_class_with_default_method"),
                )
                .with_visibility(Visibility::Protected),
            )
            .with_method(
                static_method(
                    "getMethodShouldBePublicInsteadPrivate",
                    Value::from("anonymous_foo_class_with_default_method"),
                )
                .with_visibility(Visibility::Private),
            ),
    );
    r.add_class(with_attributes(
        ClassMetadata::class(&t("HelloNamedService")).with_parent("stdClass"),
        vec![as_tagged_item(Some("hello"), Some(1))],
    ));
    r.add_class(with_attributes(
        ClassMetadata::class(&t("HelloNamedService2")),
        vec![as_tagged_item(None, Some(2))],
    ));
    r.add_class(with_attributes(
        ClassMetadata::class(&t("MultiTagHelloNamedService")),
        vec![
            as_tagged_item(Some("multi_hello_0"), Some(0)),
            as_tagged_item(Some("multi_hello_1"), Some(1)),
            as_tagged_item(Some("multi_hello_2"), Some(2)),
        ],
    ));
    r.add_class(
        ClassMetadata::interface(&t("HelloInterface")).with_method(
            MethodMetadata::public("getFooBar")
                .with_static(true)
                .with_return_type("string"),
        ),
    );
    r.add_class(
        ClassMetadata::class(&t("ClassWithDefaultNameMethod"))
            .with_method(MethodMetadata::public("getDefaultName").with_returns(Value::from("foo"))),
    );
    r.add_class(
        ClassMetadata::class(&t("ServiceWithStaticGetType"))
            .with_method(static_method("getType", Value::from("from_static_method"))),
    );
    r.add_class(
        ClassMetadata::class(&t("ServiceWithStaticPriority"))
            .with_method(static_method("getPriority", Value::Int(10))),
    );
    r.add_class(
        ClassMetadata::class(&t("NonStaticDefaultIndexClass"))
            .with_method(MethodMetadata::public("getType").with_returns(Value::from("foo"))),
    );
    r.add_class(with_attributes(
        ClassMetadata::class(&t("AsTaggedItemClassWithBusinessMethod")).with_method(
            MethodMetadata::public("getDefaultName").with_returns(Value::from("ignored")),
        ),
        vec![as_tagged_item(Some("bar"), None)],
    ));
    r.add_class(
        ClassMetadata::class(&t("StaticPriorityClass"))
            .with_method(static_method("getDefaultPriority", Value::Int(10))),
    );
    r.add_class(
        ClassMetadata::class(&t("MultiTagNonStaticClass"))
            .with_method(static_method("getType", Value::from("default"))),
    );
    r.add_class(with_attributes(
        ClassMetadata::class(&t("DecoratedAsTaggedItemService")),
        vec![as_tagged_item(Some("custom_key"), Some(1))],
    ));
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

/// `(new ResolveInstanceofConditionalsPass())->process($container)`.
fn resolve_instanceof(container: &mut ContainerBuilder) {
    ResolveInstanceofConditionalsPass::new()
        .process(container)
        .unwrap();
}

/// `PriorityTaggedServiceTraitImplementation::test()`.
fn test(tag: impl Into<TagName>, container: &mut ContainerBuilder) -> PhpArray {
    find_and_sort_tagged_services(tag, container, &[]).unwrap()
}

fn reference(id: &str) -> Value {
    Value::from(Reference::new(id))
}

fn typed(id: &str, class: &str) -> Value {
    Value::from(Reference::typed(
        id,
        class,
        InvalidBehavior::Exception,
        None,
        PhpArray::new(),
    ))
}

fn tagged(
    tag: &str,
    index_attribute: Option<&str>,
    default_index_method: Option<&str>,
) -> IteratorArgument {
    IteratorArgument::tagged_legacy(
        tag,
        index_attribute,
        default_index_method,
        false,
        None,
        Vec::new(),
        true,
    )
}

fn keys(array: &PhpArray) -> Vec<String> {
    array
        .keys()
        .map(|k| String::from_utf8_lossy(&k.bytes()).into_owned())
        .collect()
}

/// `assertEquals()` on reference arrays (keys, ids, types, names).
#[track_caller]
fn assert_refs(expected: &[(&str, Value)], actual: &PhpArray) {
    let expected_keys: Vec<&str> = expected.iter().map(|(k, _)| *k).collect();
    assert_eq!(expected_keys, keys(actual));
    for ((_, e), a) in expected.iter().zip(actual.values()) {
        assert_eq!(Reference::from_value(e), Reference::from_value(a));
    }
}

#[track_caller]
fn assert_list(expected: &[Value], actual: &PhpArray) {
    let rows: Vec<(String, Value)> = expected
        .iter()
        .enumerate()
        .map(|(i, v)| (i.to_string(), v.clone()))
        .collect();
    let rows: Vec<(&str, Value)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    assert_refs(&rows, actual);
}

#[test]
fn testThatCacheWarmersAreProcessedInPriorityOrder() {
    let services: [(&str, Option<i64>); 19] = [
        ("my_service1", Some(100)),
        ("my_service2", Some(200)),
        ("my_service3", Some(-501)),
        ("my_service4", None),
        ("my_service5", Some(-1)),
        ("my_service6", Some(-500)),
        ("my_service7", Some(-499)),
        ("my_service8", Some(1)),
        ("my_service9", Some(-2)),
        ("my_service10", Some(-1000)),
        ("my_service11", Some(-1001)),
        ("my_service12", Some(-1002)),
        ("my_service13", Some(-1003)),
        ("my_service14", Some(-1000)),
        ("my_service15", Some(1)),
        ("my_service16", Some(-1)),
        ("my_service17", Some(200)),
        ("my_service18", Some(100)),
        ("my_service19", None),
    ];

    let mut container = ContainerBuilder::new();
    for (id, priority) in services {
        let definition = container.register(id, None).unwrap();
        let attributes = match priority {
            Some(p) => php_array!["priority" => p],
            None => PhpArray::new(),
        };
        definition.add_tag("my_custom_tag", attributes);
    }

    let expected: Vec<Value> = [
        "my_service2",
        "my_service17",
        "my_service1",
        "my_service18",
        "my_service8",
        "my_service15",
        "my_service4",
        "my_service19",
        "my_service5",
        "my_service16",
        "my_service9",
        "my_service7",
        "my_service6",
        "my_service3",
        "my_service10",
        "my_service14",
        "my_service11",
        "my_service12",
        "my_service13",
    ]
    .iter()
    .map(|id| reference(id))
    .collect();

    assert_list(&expected, &test("my_custom_tag", &mut container));
}

#[test]
fn testWithEmptyArray() {
    let mut container = ContainerBuilder::new();
    assert!(test("my_custom_tag", &mut container).is_empty());
}

#[test]
fn testOnlyTheFirstNonIndexedTagIsListed() {
    let mut container = container();
    container
        .register("service1", None)
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let definition = container
        .register("service2", Some(&f("BarTagClass")))
        .unwrap();
    definition.add_tag("my_custom_tag", php_array!["priority" => 100]);
    definition.add_tag("my_custom_tag", PhpArray::new());

    assert_list(
        &[reference("service2"), reference("service1")],
        &test("my_custom_tag", &mut container),
    );
}

#[test]
fn testTagAddedByAutoconfigurationDoesNotOverrideThePriority() {
    let mut container = container();
    container
        .register_for_autoconfiguration("stdClass")
        .add_tag("my_custom_tag", PhpArray::new());
    container
        .register("service1", Some("stdClass"))
        .unwrap()
        .set_autoconfigured(true)
        .add_tag("my_custom_tag", php_array!["priority" => -70]);
    container
        .register("service2", Some("stdClass"))
        .unwrap()
        .set_autoconfigured(true);

    resolve_instanceof(&mut container);

    let expected = [reference("service2"), reference("service1")];
    assert_list(&expected, &test("my_custom_tag", &mut container));

    let expected = [
        ("service2", typed("service2", "stdClass")),
        ("service1", typed("service1", "stdClass")),
    ];
    let services = test(
        &IteratorArgument::tagged("my_custom_tag", None, true, Vec::new(), true),
        &mut container,
    );
    assert_refs(&expected, &services);
}

#[test]
fn testOnlyTheIndexedTagsAreListed() {
    let mut container = container();
    container
        .register("service1", None)
        .unwrap()
        .add_tag("my_custom_tag", php_array!["foo" => "bar"]);

    let definition = container
        .register("service2", Some(&f("BarTagClass")))
        .unwrap();
    definition.add_tag("my_custom_tag", php_array!["priority" => 100]);
    definition.add_tag("my_custom_tag", php_array!["foo" => "a"]);
    definition.add_tag("my_custom_tag", php_array!["foo" => "b", "priority" => 100]);
    definition.add_tag("my_custom_tag", php_array!["foo" => "b"]);
    definition.add_tag("my_custom_tag", PhpArray::new());

    let tag = IteratorArgument::tagged("my_custom_tag", Some("foo"), false, Vec::new(), true);
    let bar = f("BarTagClass");
    let expected = [
        ("bar_tag_class", typed("service2", &bar)),
        ("b", typed("service2", &bar)),
        ("bar", reference("service1")),
        ("a", typed("service2", &bar)),
    ];
    let services = test(&tag, &mut container);
    assert_refs(&expected, &services);
    assert_refs(&expected, &test(&tag, &mut container));
}

#[test]
fn testTheIndexedTagsByDefaultIndexMethod() {
    let mut container = container();
    container
        .register("service1", Some(&f("FooTagClass")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let definition = container
        .register("service2", Some(&f("BarTagClass")))
        .unwrap();
    definition.add_tag("my_custom_tag", php_array!["priority" => 100]);
    definition.add_tag("my_custom_tag", PhpArray::new());

    container
        .register("service3", Some(&f("IntTagClass")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    container
        .register("service4", Some(&t("HelloInterface")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let definition = container
        .register("debug.service5", Some("stdClass"))
        .unwrap();
    definition.add_tag("my_custom_tag", PhpArray::new());
    definition.add_tag("container.decorator", php_array!["id" => "service5"]);

    let tag = tagged("my_custom_tag", Some("foo"), Some("getFooBar"));
    let expected = [
        (
            "bar_tab_class_with_defaultmethod",
            typed("service2", &f("BarTagClass")),
        ),
        ("service1", typed("service1", &f("FooTagClass"))),
        ("10", typed("service3", &f("IntTagClass"))),
        ("service4", typed("service4", &t("HelloInterface"))),
        ("service5", typed("debug.service5", "stdClass")),
    ];
    let services = test(&tag, &mut container);
    assert_refs(&expected, &services);
    assert_refs(&expected, &test(&tag, &mut container));
}

#[test]
fn testTheIndexedTagsByDefaultIndexMethodFailure() {
    let class = f("FooTaggedForInvalidDefaultMethodClass");
    let rows = [
        (
            "getMethodShouldBeStatic",
            None,
            format!("Method \"{class}::getMethodShouldBeStatic()\" should be static."),
        ),
        (
            "getMethodShouldBeStatic",
            Some("foo"),
            format!(
                "Either method \"{class}::getMethodShouldBeStatic()\" should be static or tag \"my_custom_tag\" on service \"service1\" is missing attribute \"foo\"."
            ),
        ),
        (
            "getMethodShouldBePublicInsteadProtected",
            None,
            format!(
                "Method \"{class}::getMethodShouldBePublicInsteadProtected()\" should be public."
            ),
        ),
        (
            "getMethodShouldBePublicInsteadProtected",
            Some("foo"),
            format!(
                "Either method \"{class}::getMethodShouldBePublicInsteadProtected()\" should be public or tag \"my_custom_tag\" on service \"service1\" is missing attribute \"foo\"."
            ),
        ),
        (
            "getMethodShouldBePublicInsteadPrivate",
            None,
            format!(
                "Method \"{class}::getMethodShouldBePublicInsteadPrivate()\" should be public."
            ),
        ),
        (
            "getMethodShouldBePublicInsteadPrivate",
            Some("foo"),
            format!(
                "Either method \"{class}::getMethodShouldBePublicInsteadPrivate()\" should be public or tag \"my_custom_tag\" on service \"service1\" is missing attribute \"foo\"."
            ),
        ),
    ];
    for (default_index_method, index_attribute, expected_exception_message) in rows {
        let mut container = container();
        container
            .register("service1", Some(&class))
            .unwrap()
            .add_tag("my_custom_tag", PhpArray::new());

        let tag = tagged("my_custom_tag", index_attribute, Some(default_index_method));
        let e = find_and_sort_tagged_services(&tag, &mut container, &[]).unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument));
        assert_eq!(expected_exception_message, e.message());
    }
}

#[test]
fn testTaggedItemAttributes() {
    let mut container = container();
    let (hello, hello2, multi) = (
        t("HelloNamedService"),
        t("HelloNamedService2"),
        t("MultiTagHelloNamedService"),
    );
    container
        .register("service1", Some(&f("FooTagClass")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());
    let conditional = Definition::child("");
    conditional.add_tag("my_custom_tag", PhpArray::new());
    let conditional2 = Definition::child("");
    conditional2.add_tag("my_custom_tag2", PhpArray::new());
    let mut instanceof = PhpArray::new();
    instanceof.insert(hello.as_str(), conditional.to_value());
    instanceof.insert("stdClass", conditional2.to_value());
    container
        .register("service2", Some(&hello))
        .unwrap()
        .set_autoconfigured(true)
        .set_instanceof_conditionals(instanceof);
    for id in ["service3", "service4", "service5"] {
        container
            .register(id, Some(&hello2))
            .unwrap()
            .set_autoconfigured(true)
            .add_tag("my_custom_tag", PhpArray::new());
    }
    container
        .register("service6", Some(&multi))
        .unwrap()
        .set_autoconfigured(true)
        .add_tag("my_custom_tag", PhpArray::new());

    resolve_instanceof(&mut container);

    let tag = IteratorArgument::tagged_legacy(
        "my_custom_tag",
        Some("foo"),
        Some("getFooBar"),
        false,
        None,
        vec!["service4".to_string(), "service5".to_string()],
        true,
    );
    let expected = [
        ("service3", typed("service3", &hello2)),
        ("multi_hello_2", typed("service6", &multi)),
        ("hello", typed("service2", &hello)),
        ("multi_hello_1", typed("service6", &multi)),
        ("service1", typed("service1", &f("FooTagClass"))),
        ("multi_hello_0", typed("service6", &multi)),
    ];

    let services = test(&tag, &mut container);
    assert_refs(&expected, &services);
    assert_refs(&expected, &test(&tag, &mut container));
}

#[test]
fn testResolveIndexedTags() {
    let mut container = container();
    container
        .set_parameter("custom_param_service1", "bar")
        .unwrap();
    container
        .set_parameter("custom_param_service2", "baz")
        .unwrap();
    container
        .set_parameter("custom_param_service2_empty", "")
        .unwrap();
    container
        .set_parameter("custom_param_service2_null", Value::Null)
        .unwrap();
    container.register("service1", None).unwrap().add_tag(
        "my_custom_tag",
        php_array!["foo" => "%custom_param_service1%"],
    );

    let definition = container
        .register("service2", Some(&f("BarTagClass")))
        .unwrap();
    definition.add_tag(
        "my_custom_tag",
        php_array!["foo" => "%custom_param_service2%", "priority" => 100],
    );
    definition.add_tag(
        "my_custom_tag",
        php_array!["foo" => "%custom_param_service2_empty%"],
    );
    definition.add_tag(
        "my_custom_tag",
        php_array!["foo" => "%custom_param_service2_null%"],
    );

    let tag = tagged("my_custom_tag", Some("foo"), Some("getFooBar"));
    let bar = f("BarTagClass");
    let expected = [
        ("baz", typed("service2", &bar)),
        ("bar", reference("service1")),
        ("", typed("service2", &bar)),
        ("bar_tab_class_with_defaultmethod", typed("service2", &bar)),
    ];
    let services = test(&tag, &mut container);
    assert_refs(&expected, &services);
    assert_refs(&expected, &test(&tag, &mut container));
}

#[test]
fn testAttributesAreMergedWithTags() {
    let mut container = container();
    let class = t("MultiTagHelloNamedService");
    let definition = container
        .register("service_attr_first", Some(&class))
        .unwrap();
    definition.set_autoconfigured(true);
    definition.add_tag("my_custom_tag", php_array!["foo" => "z"]);
    definition.add_tag("my_custom_tag", PhpArray::new());

    resolve_instanceof(&mut container);

    let tag = tagged("my_custom_tag", Some("foo"), Some("getFooBar"));
    let services = test(&tag, &mut container);

    let expected = [
        ("multi_hello_2", typed("service_attr_first", &class)),
        ("multi_hello_1", typed("service_attr_first", &class)),
        ("z", typed("service_attr_first", &class)),
        ("multi_hello_0", typed("service_attr_first", &class)),
    ];
    assert_refs(&expected, &services);
}

#[test]
fn testAttributesAreFallbacks() {
    let mut container = container();
    let class = t("MultiTagHelloNamedService");
    let definition = container
        .register("service_attr_first", Some(&class))
        .unwrap();
    definition.set_autoconfigured(true);
    definition.add_tag("my_custom_tag", php_array!["foo" => "z"]);

    resolve_instanceof(&mut container);

    let tag = tagged("my_custom_tag", Some("foo"), Some("getFooBar"));
    let services = test(&tag, &mut container);

    assert_refs(&[("z", typed("service_attr_first", &class))], &services);
}

#[test]
fn testTaggedIteratorWithDefaultNameMethod() {
    let mut container = container();
    container
        .register("service", Some(&t("ClassWithDefaultNameMethod")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let tag = IteratorArgument::tagged("my_custom_tag", None, false, Vec::new(), true);
    let services = test(&tag, &mut container);
    assert_list(&[reference("service")], &services);
}

#[test]
fn testIndexedIteratorUsesTagAttributeOverDefaultMethod() {
    let mut container = container();
    container
        .register("service.a", Some(&t("ServiceWithStaticGetType")))
        .unwrap()
        .add_tag("my_tag", php_array!["type" => "from_tag"]);

    let tag = tagged("my_tag", Some("type"), Some("getType"));
    let services = test(&tag, &mut container);

    assert!(services.contains_key(&Key::from("from_tag")));
    assert!(!services.contains_key(&Key::from("from_static_method")));
    let reference = Reference::from_value(services.get(&Key::from("from_tag")).unwrap()).unwrap();
    assert!(reference.is_typed());
    assert_eq!("service.a", reference.id());
}

#[test]
fn testIndexedIteratorUsesDefaultMethodAsFallback() {
    let mut container = container();
    container
        .register("service.a", Some(&t("ServiceWithStaticGetType")))
        .unwrap()
        .add_tag("my_tag", PhpArray::new());

    let tag = tagged("my_tag", Some("type"), Some("getType"));
    let services = test(&tag, &mut container);

    assert!(services.contains_key(&Key::from("from_static_method")));
    assert!(!services.contains_key(&Key::from("from_tag")));
    let reference =
        Reference::from_value(services.get(&Key::from("from_static_method")).unwrap()).unwrap();
    assert!(reference.is_typed());
}

#[test]
fn testIndexedIteratorUsesTagIndexAndDefaultPriorityMethod() {
    let mut container = container();

    container
        .register("service.a", Some(&t("ServiceWithStaticPriority")))
        .unwrap()
        .add_tag("my_tag", php_array!["type" => "tag_index"]);

    container
        .register("service.b", Some("stdClass"))
        .unwrap()
        .add_tag("my_tag", php_array!["type" => "another_index"]);

    let tag = IteratorArgument::tagged_legacy(
        "my_tag",
        Some("type"),
        None,
        false,
        Some("getPriority"),
        Vec::new(),
        true,
    );
    let services = test(&tag, &mut container);

    let reference = Reference::from_value(services.get(&Key::from("tag_index")).unwrap()).unwrap();
    assert_eq!("service.a", reference.id());

    assert_eq!(vec!["tag_index", "another_index"], keys(&services));
}

#[test]
fn testTaggedLocatorWithProvidedIndexAttributeAndNonStaticDefaultIndexMethod() {
    let mut container = container();
    let class = t("NonStaticDefaultIndexClass");
    container
        .register("service", Some(&class))
        .unwrap()
        .add_tag("my_custom_tag", php_array!["type" => "foo"]);

    let tag = tagged("my_custom_tag", Some("type"), Some("getType"));

    let services = test(&tag, &mut container);
    assert_refs(&[("foo", typed("service", &class))], &services);
}

#[test]
fn testTaggedLocatorWithoutIndexAttributeAndNonStaticDefaultIndexMethod() {
    let mut container = container();
    let class = t("NonStaticDefaultIndexClass");
    container
        .register("service", Some(&class))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let tag = tagged("my_custom_tag", Some("type"), Some("getType"));

    let e = find_and_sort_tagged_services(&tag, &mut container, &[]).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        format!(
            "Either method \"{class}::getType()\" should be static or tag \"my_custom_tag\" on service \"service\" is missing attribute \"type\"."
        ),
        e.message()
    );
}

#[test]
fn testMergingAsTaggedItemWithEmptyTagAndNonStaticBusinessMethod() {
    let mut container = container();
    let class = t("AsTaggedItemClassWithBusinessMethod");
    container
        .register("service", Some(&class))
        .unwrap()
        .set_autoconfigured(true)
        .add_tag("my_custom_tag", PhpArray::new());

    resolve_instanceof(&mut container);

    let tag = IteratorArgument::tagged("my_custom_tag", Some("index"), false, Vec::new(), true);

    let services = test(&tag, &mut container);
    assert_refs(&[("bar", typed("service", &class))], &services);
}

#[test]
fn testPriorityFallbackWithoutIndexAndStaticPriorityMethod() {
    let mut container = container();
    container
        .register("service", Some(&t("StaticPriorityClass")))
        .unwrap()
        .add_tag("my_custom_tag", PhpArray::new());

    let tag = IteratorArgument::tagged_legacy(
        "my_custom_tag",
        None,
        None,
        false,
        Some("getDefaultPriority"),
        Vec::new(),
        true,
    );

    let services = test(&tag, &mut container);
    assert_list(&[reference("service")], &services);
}

#[test]
fn testMultiTagsWithMixedAttributesAndNonStaticDefault() {
    let mut container = container();
    container
        .register("service", Some(&t("MultiTagNonStaticClass")))
        .unwrap()
        .add_tag("my_custom_tag", php_array!["type" => "foo"])
        .add_tag("my_custom_tag", PhpArray::new());

    let tag = tagged("my_custom_tag", Some("type"), Some("getType"));

    let services = test(&tag, &mut container);
    assert_eq!(2, services.len());
    assert!(services.contains_key(&Key::from("foo")));
    assert!(services.contains_key(&Key::from("default")));
}

#[test]
fn testTagAttributesAreNotAList() {
    let mut container = ContainerBuilder::new();
    let mut tag = PhpArray::new();
    tag.insert(1, Value::Array(php_array!["attributes" => "not_a_list"]));
    container
        .register("service1", None)
        .unwrap()
        .set_tags(php_array!["my_custom_tag" => Value::Array(tag)]);

    assert_list(
        &[reference("service1")],
        &test("my_custom_tag", &mut container),
    );
}

#[test]
fn testDecoratedServiceAsTaggedItemIndex() {
    let mut container = container();
    let class = t("DecoratedAsTaggedItemService");

    // Register the inner service with AsTaggedItem
    container
        .register("inner.tagged_service", Some(&class))
        .unwrap()
        .set_autoconfigured(true)
        .add_tag("my_custom_tag", PhpArray::new());

    // Register a decorator that wraps the inner service
    let decorator = container
        .register("decorator.tagged_service", Some("stdClass"))
        .unwrap();
    decorator.add_tag("my_custom_tag", PhpArray::new());
    decorator.add_tag(
        "container.decorator",
        php_array!["id" => class.as_str(), "inner" => "inner.tagged_service"],
    );

    resolve_instanceof(&mut container);

    let tag = IteratorArgument::tagged("my_custom_tag", Some("foo"), false, Vec::new(), true);
    let services = test(&tag, &mut container);

    let reference = Reference::from_value(services.get(&Key::from("custom_key")).unwrap()).unwrap();
    assert_eq!("decorator.tagged_service", reference.id());
}

#[test]
fn testMultiLevelDecoratedServiceAsTaggedItemIndex() {
    let mut container = container();
    let class = t("DecoratedAsTaggedItemService");

    // Register the innermost service with AsTaggedItem
    container
        .register("inner.tagged_service", Some(&class))
        .unwrap()
        .set_autoconfigured(true)
        .add_tag("my_custom_tag", PhpArray::new());

    // First decorator wraps the inner service
    let decorator1 = container
        .register("decorator1.tagged_service", Some("stdClass"))
        .unwrap();
    decorator1.add_tag("my_custom_tag", PhpArray::new());
    decorator1.add_tag(
        "container.decorator",
        php_array!["id" => class.as_str(), "inner" => "inner.tagged_service"],
    );

    // Second decorator wraps the first decorator
    let decorator2 = container
        .register("decorator2.tagged_service", Some("stdClass"))
        .unwrap();
    decorator2.add_tag("my_custom_tag", PhpArray::new());
    decorator2.add_tag(
        "container.decorator",
        php_array!["id" => class.as_str(), "inner" => "decorator1.tagged_service"],
    );

    resolve_instanceof(&mut container);

    let tag = IteratorArgument::tagged("my_custom_tag", Some("foo"), false, Vec::new(), true);
    let services = test(&tag, &mut container);

    let reference = Reference::from_value(services.get(&Key::from("custom_key")).unwrap()).unwrap();
    assert_eq!("decorator2.tagged_service", reference.id());
}
